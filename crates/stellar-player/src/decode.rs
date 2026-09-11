use anyhow::Context;
use audioadapter_buffers::direct::InterleavedSlice;
use ringbuf::{
    CachingCons, CachingProd, HeapRb,
    traits::{Observer, Producer, Split},
};
use rubato::Resampler;
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::Thread,
};
use symphonia::core::{
    codecs::audio::AudioDecoderOptions,
    formats::{FormatOptions, TrackType, probe::Hint},
    io::{MediaSourceStream, MediaSourceStreamOptions},
    meta::MetadataOptions,
};
use tracing::{debug, error, warn};

pub fn spawn_decode(
    path: PathBuf,
    target_sample_rate: u32,
    target_channels: u16,
) -> Result<(HeapF32Consumer, Arc<AtomicBool>, Thread), anyhow::Error> {
    let buffer =
        HeapRb::new(BUFFER_SECONDS * target_sample_rate as usize * target_channels as usize);
    let (producer, consumer) = buffer.split();

    let done = Arc::new(AtomicBool::new(false));

    let join_handle = std::thread::spawn({
        let done = done.clone();
        move || {
            if let Err(e) = decode(
                &path,
                target_sample_rate,
                target_channels as usize,
                producer,
            ) {
                error!(?e, "Decode thread errored");
            } else {
                debug!("Decode thread finished");
            }

            done.store(true, Ordering::SeqCst);
        }
    });
    let thread = join_handle.thread().clone();

    Ok((consumer, done, thread))
}

pub fn decode(
    path: &Path,
    target_sample_rate: u32,
    target_channels: usize,
    mut producer: HeapF32Producer,
) -> Result<(), anyhow::Error> {
    let source = std::fs::File::open(path)?;

    let media_source_stream_options = MediaSourceStreamOptions::default();
    let media_source_stream = MediaSourceStream::new(Box::new(source), media_source_stream_options);

    let hint = Hint::new();
    // hint.with_extension(path.extension());

    let format_options = FormatOptions::default();
    let metadata_options = MetadataOptions::default();
    let mut format = symphonia::default::get_probe()
        .probe(&hint, media_source_stream, format_options, metadata_options)
        .context("Unsupported format")?;

    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| anyhow::anyhow!("Missing audio track"))?;
    let track_id = track.id;

    let audio_codec_params = track
        .codec_params
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing codec parameters"))?
        .audio()
        .ok_or_else(|| anyhow::anyhow!("Missing audio codec parameters"))?;
    let audio_decoder_options = AudioDecoderOptions::default();
    let mut audio_decoder = symphonia::default::get_codecs()
        .make_audio_decoder(audio_codec_params, &audio_decoder_options)
        .context("Failed to make audio decoder")?;

    let source_channels = match &audio_codec_params.channels {
        Some(channels) => channels.count(),
        None => {
            error!(
                "AudioCodecParameters channels missing for {}, defaulting to 2",
                path.display()
            );
            2
        }
    };

    let mut resampler = match audio_codec_params.sample_rate {
        Some(source_sample_rate) if source_sample_rate == target_sample_rate => {
            // Already in target sample rate, skip resampling
            None
        }
        Some(source_sample_rate) => {
            let resampler = rubato::Fft::<f32>::new(
                source_sample_rate as usize,
                target_sample_rate as usize,
                1024,
                source_channels,
                rubato::FixedSync::Both,
            )
            .context("Failed to initialize resampler")?;

            Some(resampler)
        }
        None => {
            warn!(
                "AudioCodecParameters sample_rate missing for {}, skipping resampling",
                path.display()
            );
            None
        }
    };

    let mut decoded_buffer = Vec::<f32>::new();
    let mut resampled_buffer = Vec::<f32>::new();
    let mut remixed_buffer = Vec::<f32>::new();

    loop {
        // Get the next packet from the media format.
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => {
                // Reached the end of the stream.
                break;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => {
                // The track list has been changed. Re-examine it and create a new set of decoders,
                // then restart the decode loop. This is an advanced feature and it is not
                // unreasonable to consider this "the end." As of Symphonia v0.5.0, the only usage
                // of this is for chained OGG physical streams.
                anyhow::bail!("Received ResetRequired, not implemented");
            }
            Err(e) => return Err(e).context("Format error"),
        };

        if packet.track_id != track_id {
            continue;
        }

        // Decode the packet into audio samples.
        match audio_decoder.decode(&packet) {
            Ok(decoded) => {
                // Append decoded samples to buffer, interleaved, and convert to f32
                let prev_len = decoded_buffer.len();
                decoded_buffer.resize(prev_len + decoded.samples_interleaved(), 0.0);
                decoded.copy_to_slice_interleaved(&mut decoded_buffer[prev_len..]);

                if let Some(resampler) = &mut resampler {
                    let needed_input_frames_per_channel = resampler.input_frames_next();
                    let needed_input_frames = needed_input_frames_per_channel * source_channels;

                    if decoded_buffer.len() < needed_input_frames {
                        continue;
                    }

                    let needed_output_frames_per_channel = resampler.output_frames_next();
                    let needed_output_frames = needed_output_frames_per_channel * source_channels;

                    resampled_buffer.resize(needed_output_frames, 0.0);

                    let (used_input_frames_per_channel, used_output_frames_per_channel) = resampler
                        .process_into_buffer(
                            &InterleavedSlice::new(
                                &decoded_buffer,
                                source_channels,
                                needed_input_frames_per_channel,
                            )
                            .context("Failed to create InterleavedSlice input adapter")?,
                            &mut InterleavedSlice::new_mut(
                                &mut resampled_buffer,
                                source_channels,
                                needed_output_frames_per_channel,
                            )
                            .context("Failed to create InterleavedSlice output adapter")?,
                            None,
                        )
                        .context("Resampler error")?;

                    let used_input_frames = used_input_frames_per_channel * source_channels;
                    let used_output_frames = used_output_frames_per_channel * source_channels;

                    // Remove used decoded samples
                    decoded_buffer.drain(0..used_input_frames);

                    let output_slice = if source_channels == target_channels {
                        // Use resampled samples directly
                        &resampled_buffer[0..used_output_frames]
                    } else {
                        let remixed_frames = used_output_frames_per_channel * target_channels;
                        remixed_buffer.clear();
                        remixed_buffer.reserve_exact(remixed_frames);

                        // Upmix or downmix by averaging
                        for frame in
                            resampled_buffer[0..used_output_frames].chunks_exact(source_channels)
                        {
                            let average = frame.iter().sum::<f32>() / frame.iter().len() as f32;

                            for _ in 0..target_channels {
                                remixed_buffer.push(average);
                            }
                        }

                        &remixed_buffer[0..remixed_frames]
                    };

                    // Wait for space
                    while producer.vacant_len() < output_slice.len() {
                        std::thread::park();
                    }

                    producer.push_slice(&output_slice);
                } else {
                    let output_slice = if source_channels == target_channels {
                        // Use decoded samples directly
                        &decoded_buffer
                    } else {
                        let remixed_frames = decoded.frames() * target_channels;
                        remixed_buffer.clear();
                        remixed_buffer.reserve_exact(remixed_frames);

                        // Upmix or downmix by averaging
                        for frame in decoded_buffer.chunks_exact(source_channels) {
                            let average = frame.iter().sum::<f32>() / frame.iter().len() as f32;

                            for _ in 0..target_channels {
                                remixed_buffer.push(average);
                            }
                        }

                        &remixed_buffer[0..remixed_frames]
                    };

                    // Wait for space
                    while producer.vacant_len() < output_slice.len() {
                        std::thread::park();
                    }

                    producer.push_slice(&output_slice);

                    // Clear decoded samples
                    decoded_buffer.clear();
                }
            }
            Err(symphonia::core::errors::Error::IoError(e)) => {
                // The packet failed to decode due to an IO error, skip the packet.
                error!(?e, "IO error during decoding, skipping packet");
                continue;
            }
            Err(symphonia::core::errors::Error::DecodeError(e)) => {
                // The packet failed to decode due to invalid data, skip the packet.
                error!(?e, "Decode error during decoding, skipping packet");
                continue;
            }
            Err(e) => return Err(e).context("Decoder error"),
        }
    }

    Ok(())
}

type HeapF32Producer = CachingProd<Arc<HeapRb<f32>>>;
type HeapF32Consumer = CachingCons<Arc<HeapRb<f32>>>;

const BUFFER_SECONDS: usize = 1;
