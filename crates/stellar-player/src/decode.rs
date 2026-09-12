use anyhow::Context;
use audioadapter_buffers::direct::InterleavedSlice;
use ringbuf::{
    CachingCons, CachingProd, HeapRb,
    traits::{Observer, Producer, Split},
};
use rubato::{Indexing, Resampler as _};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::Thread,
};
use symphonia::core::{
    codecs::audio::{AudioDecoder, AudioDecoderOptions},
    formats::{FormatOptions, FormatReader, TrackType, probe::Hint},
    io::{MediaSourceStream, MediaSourceStreamOptions},
    meta::MetadataOptions,
};
use tracing::{debug, error, warn};

pub fn spawn_decode(
    path: PathBuf,
    target_sample_rate: usize,
    target_channels: usize,
) -> Result<(HeapF32Consumer, Arc<AtomicBool>, Thread), anyhow::Error> {
    let buffer = HeapRb::new(BUFFER_SECONDS * target_sample_rate * target_channels);
    let (producer, consumer) = buffer.split();

    let done = Arc::new(AtomicBool::new(false));

    let join_handle = std::thread::spawn({
        let done = done.clone();
        move || {
            let result = Decoder::open(&path, target_sample_rate, target_channels, producer)
                .and_then(|mut decoder| decoder.run());

            if let Err(e) = result {
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

/// Decoder state.
struct Decoder {
    format: Box<dyn FormatReader>,
    track_id: u32,
    audio_decoder: Box<dyn AudioDecoder>,
    source_channels: usize,
    target_channels: usize,
    resampler: Option<Resampler>,
    decoded_buffer: Vec<f32>,
    remixed_buffer: Vec<f32>,
    producer: HeapF32Producer,
}

impl Decoder {
    /// Open a file and initialize decoder state.
    fn open(
        path: &Path,
        target_sample_rate: usize,
        target_channels: usize,
        producer: HeapF32Producer,
    ) -> anyhow::Result<Self> {
        let source = std::fs::File::open(path)?;

        let media_source_stream_options = MediaSourceStreamOptions::default();
        let media_source_stream =
            MediaSourceStream::new(Box::new(source), media_source_stream_options);

        let hint = Hint::new();
        // hint.with_extension(path.extension());

        let format_options = FormatOptions::default();
        let metadata_options = MetadataOptions::default();
        let format = symphonia::default::get_probe()
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
        let audio_decoder = symphonia::default::get_codecs()
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

        let resampler = match audio_codec_params.sample_rate {
            Some(source_sample_rate) => Resampler::new(
                source_sample_rate as usize,
                target_sample_rate,
                source_channels,
            )?,
            None => {
                warn!(
                    "AudioCodecParameters sample_rate missing for {}, skipping resampling",
                    path.display()
                );
                None
            }
        };

        Ok(Self {
            format,
            track_id,
            audio_decoder,
            source_channels,
            target_channels,
            resampler,
            decoded_buffer: Vec::new(),
            remixed_buffer: Vec::new(),
            producer,
        })
    }

    /// Decode and process packets and push to the audio buffer.
    fn run(&mut self) -> anyhow::Result<()> {
        loop {
            // Get the next packet from the media format.
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => {
                    // Reached the end of the stream.
                    break;
                }
                Err(symphonia::core::errors::Error::ResetRequired) => {
                    // The track list has been changed. Re-examine it and create a new set of
                    // decoders, then restart the decode loop. This is an advanced feature and
                    // it is not unreasonable to consider this "the end." As of Symphonia
                    // v0.5.0, the only usage of this is for chained OGG physical streams.
                    anyhow::bail!("Received ResetRequired, not implemented");
                }
                Err(e) => return Err(e).context("Format error"),
            };

            if packet.track_id != self.track_id {
                continue;
            }

            // Decode the packet into audio samples.
            match self.audio_decoder.decode(&packet) {
                Ok(decoded) => {
                    // Append decoded samples to buffer, interleaved, and convert to f32
                    let prev_len = self.decoded_buffer.len();
                    self.decoded_buffer
                        .resize(prev_len + decoded.samples_interleaved(), 0.0);
                    decoded.copy_to_slice_interleaved(&mut self.decoded_buffer[prev_len..]);

                    self.process_decoded()?;
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

        self.flush()
    }

    /// Process decoded samples in `decoded_buffer` by optionally resampling and then remixing,
    /// and push to the audio buffer.
    fn process_decoded(&mut self) -> anyhow::Result<()> {
        let Self {
            resampler,
            decoded_buffer,
            remixed_buffer,
            source_channels,
            target_channels,
            producer,
            ..
        } = self;

        if let Some(resampler) = resampler {
            // Resample decoded samples in chunks
            while decoded_buffer.len() / *source_channels >= resampler.input_frames_next() {
                // Resample chunk of decoded samples
                let (used_input_samples, resampled_slice) =
                    resampler.resample_chunk(decoded_buffer, *source_channels, None)?;

                // Remix resampled samples and push to audio buffer
                remix_and_push(
                    resampled_slice,
                    *source_channels,
                    *target_channels,
                    remixed_buffer,
                    producer,
                );

                // Remove chunk decoded samples
                decoded_buffer.drain(0..used_input_samples);
            }
        } else {
            // Remix all decoded samples and push to audio buffer
            remix_and_push(
                decoded_buffer,
                *source_channels,
                *target_channels,
                remixed_buffer,
                producer,
            );

            // Clear decoded samples
            decoded_buffer.clear();
        }

        Ok(())
    }

    /// If resampling, process a partial chunk of decoded samples at the end of the stream.
    /// If not resampling, all decoded samples were already processed by `process_decoded`
    /// since there is no chunking.
    fn flush(&mut self) -> anyhow::Result<()> {
        let Self {
            resampler,
            decoded_buffer,
            remixed_buffer,
            source_channels,
            target_channels,
            producer,
            ..
        } = self;

        let Some(resampler) = resampler else {
            return Ok(());
        };

        let remaining_frames = decoded_buffer.len() / *source_channels;
        if remaining_frames == 0 {
            return Ok(());
        }

        let needed_frames = resampler.input_frames_next();
        decoded_buffer.resize(needed_frames * *source_channels, 0.0);

        // Resample partial chunk of decoded samples
        let (used_input_samples, resampled_slice) =
            resampler.resample_chunk(decoded_buffer, *source_channels, Some(remaining_frames))?;

        // Remix resampled samples and push to audio buffer
        remix_and_push(
            resampled_slice,
            *source_channels,
            *target_channels,
            remixed_buffer,
            producer,
        );

        // Remove chunk decoded samples
        decoded_buffer.drain(0..used_input_samples);

        Ok(())
    }
}

/// Resampler state.
struct Resampler {
    inner: rubato::Fft<f32>,
    /// How many more frames to skip for the resampler output delay.
    remaining_delay_frames: usize,
    scratch: Vec<f32>,
}

impl Resampler {
    /// Builds a resampler if resampling is needed.
    fn new(
        source_sample_rate: usize,
        target_sample_rate: usize,
        source_channels: usize,
    ) -> anyhow::Result<Option<Self>> {
        if source_sample_rate == target_sample_rate {
            return Ok(None);
        }

        let inner = rubato::Fft::<f32>::new(
            source_sample_rate,
            target_sample_rate,
            1024,
            source_channels,
            rubato::FixedSync::Both,
        )
        .context("Failed to initialize resampler")?;
        let remaining_delay_frames = inner.output_delay();

        Ok(Some(Self {
            inner,
            remaining_delay_frames,
            scratch: Vec::new(),
        }))
    }

    fn input_frames_next(&self) -> usize {
        self.inner.input_frames_next()
    }

    /// Resamples a chunk of decoded samples. Handles removing output delay frames and processing
    /// a final partial chunk. Returns the number of input samples used and the resampled samples.
    fn resample_chunk(
        &mut self,
        input: &[f32],
        source_channels: usize,
        final_chunk_frames: Option<usize>,
    ) -> anyhow::Result<(usize, &[f32])> {
        let input_frames_per_channel = self.inner.input_frames_next();
        let output_frames_per_channel = self.inner.output_frames_next();

        self.scratch
            .resize(output_frames_per_channel * source_channels, 0.0);

        let indexing = final_chunk_frames.map(|len| Indexing::new().partial_len(len));

        let (used_input_frames_per_channel, mut used_output_frames_per_channel) = self
            .inner
            .process_into_buffer(
                &InterleavedSlice::new(input, source_channels, input_frames_per_channel)
                    .context("Failed to create InterleavedSlice input adapter")?,
                &mut InterleavedSlice::new_mut(
                    &mut self.scratch,
                    source_channels,
                    output_frames_per_channel,
                )
                .context("Failed to create InterleavedSlice output adapter")?,
                indexing.as_ref(),
            )
            .context("Resampler error")?;

        let skip_frames = self
            .remaining_delay_frames
            .min(used_output_frames_per_channel);
        self.remaining_delay_frames -= skip_frames;
        used_output_frames_per_channel -= skip_frames;

        let output_start = skip_frames * source_channels;
        let output_len = used_output_frames_per_channel * source_channels;
        let output_slice = &self.scratch[output_start..output_start + output_len];

        Ok((
            used_input_frames_per_channel * source_channels,
            output_slice,
        ))
    }
}

/// Remix samples to the target channel count and push to the audio buffer, waiting when full.
fn remix_and_push(
    input: &[f32],
    source_channels: usize,
    target_channels: usize,
    remixed_buffer: &mut Vec<f32>,
    producer: &mut HeapF32Producer,
) {
    let output_slice = if source_channels == target_channels {
        // Use samples directly
        input
    } else {
        // Up/downmix by averaging each frame's channels
        remixed_buffer.clear();
        remixed_buffer.reserve(input.len() / source_channels * target_channels);
        for frame in input.chunks_exact(source_channels) {
            let average = frame.iter().sum::<f32>() / source_channels as f32;
            remixed_buffer.extend(std::iter::repeat(average).take(target_channels));
        }
        remixed_buffer.as_slice()
    };

    // Wait for space
    while producer.vacant_len() < output_slice.len() {
        std::thread::park();
    }

    producer.push_slice(output_slice);
}

type HeapF32Producer = CachingProd<Arc<HeapRb<f32>>>;
type HeapF32Consumer = CachingCons<Arc<HeapRb<f32>>>;

const BUFFER_SECONDS: usize = 1;
