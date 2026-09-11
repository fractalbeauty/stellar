use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Context;
use cpal::{
    Device, FromSample, OutputCallbackInfo, Sample, SampleFormat, SampleRate, Stream,
    SupportedOutputConfigs, SupportedStreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use ringbuf::{
    HeapRb, SharedRb,
    traits::{Consumer, Observer, Producer},
};
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

pub mod queue;

pub struct CpalBackend {
    stream: Stream,
}

impl CpalBackend {
    pub fn new() -> Result<Self, anyhow::Error> {
        let host = cpal::default_host();

        let Some(device) = host.default_output_device() else {
            anyhow::bail!("No output devices available");
        };

        let supported_output_configs = device
            .supported_output_configs()
            .context("Failed to get supported output configs")?;

        let output_config_range = supported_output_configs
            .max_by(|a, b| a.cmp_default_heuristics(b))
            .ok_or_else(|| anyhow::anyhow!("Failed to choose output config"))?;

        let output_config = output_config_range
            .try_with_standard_sample_rate()
            .unwrap_or_else(|| output_config_range.with_max_sample_rate());

        debug!(
            channels = output_config.channels(),
            sample_rate = output_config.sample_rate(),
            sample_format = ?output_config.sample_format(),
            buffer_size = ?output_config.buffer_size(),
            "Configured output"
        );

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        let error_callback = |err| error!(?err, "cpal error");

        let stream = match output_config.sample_format() {
            SampleFormat::I8 => todo!(),
            SampleFormat::I16 => todo!(),
            SampleFormat::I24 => todo!(),
            SampleFormat::I32 => todo!(),
            SampleFormat::I64 => todo!(),
            SampleFormat::U8 => todo!(),
            SampleFormat::U16 => todo!(),
            SampleFormat::U24 => todo!(),
            SampleFormat::U32 => todo!(),
            SampleFormat::U64 => todo!(),
            SampleFormat::F32 => device.build_output_stream(
                output_config.config(),
                make_data_callback::<f32>(rx),
                error_callback,
                None,
            )?,
            SampleFormat::F64 => todo!(),
            SampleFormat::DsdU8 => todo!(),
            SampleFormat::DsdU16 => todo!(),
            SampleFormat::DsdU32 => todo!(),
            _ => todo!(),
        };

        tx.send(DataCallbackMessage::PlayNow {
            buffer: {
                let d = 3;
                let frames = (output_config.sample_rate() as usize) * d;
                let mut buffer = HeapRb::new(frames * (output_config.channels() as usize));
                buffer.push_iter((0..frames).flat_map(|i| {
                    let clock = i as f32 / (output_config.sample_rate() as f32);

                    let a = 440.0;
                    let b = 880.0;
                    let phase = 2.0
                        * std::f32::consts::PI
                        * (a * clock + (b - a) / (2.0 * d as f32) * clock * clock);
                    let sample = phase.sin();

                    vec![sample; output_config.channels() as usize]
                }));
                dbg!(buffer.vacant_len(), buffer.occupied_len());
                DataCallbackBuffer {
                    buffer,
                    done: AtomicBool::new(true),
                }
            },
        })
        .unwrap();

        tx.send(DataCallbackMessage::PlayNext {
            buffer: {
                let d = 3;
                let frames = (output_config.sample_rate() as usize) * d;
                let mut buffer = HeapRb::new(frames * (output_config.channels() as usize));
                buffer.push_iter((0..frames).flat_map(|i| {
                    let clock = i as f32 / (output_config.sample_rate() as f32);

                    let a = 440.0;
                    let b = 880.0;
                    let phase = 2.0
                        * std::f32::consts::PI
                        * (a * clock + (b - a) / (2.0 * d as f32) * clock * clock);
                    let sample = phase.sin();

                    vec![sample; output_config.channels() as usize]
                }));
                dbg!(buffer.vacant_len(), buffer.occupied_len());
                DataCallbackBuffer {
                    buffer,
                    done: AtomicBool::new(true),
                }
            },
        })
        .unwrap();

        stream.play().unwrap();

        Ok(Self { stream })
    }
}

fn make_data_callback<T: Sample + FromSample<f32>>(
    mut rx: mpsc::UnboundedReceiver<DataCallbackMessage>,
) -> impl FnMut(&mut [T], &OutputCallbackInfo) {
    let mut state = DataCallbackState {
        current: None,
        next: None,
    };

    move |output_buffer, _| {
        // Handle messages
        while let Ok(message) = rx.try_recv() {
            match message {
                DataCallbackMessage::PlayNow { buffer } => {
                    state.current = Some(buffer);
                    debug!("Received PlayNow")
                }
                DataCallbackMessage::PlayNext { buffer } => {
                    state.next = Some(buffer);
                }
            }
        }

        // Read from current buffer
        if let Some(current) = &mut state.current {
            let mut buffer_iter = current.buffer.pop_iter();

            for output_sample in output_buffer {
                let Some(buffer_sample) = buffer_iter.next() else {
                    break;
                };
                *output_sample = Sample::from_sample(buffer_sample);
            }
        }

        if state
            .current
            .as_ref()
            .is_some_and(|current| current.buffer.is_empty() && current.done.load(Ordering::SeqCst))
        {
            debug!("Finished current buffer");
            state.current = state.next.take();
            state.next = None;
        } else if state
            .current
            .as_ref()
            .is_some_and(|current| current.buffer.is_empty())
        {
            warn!("Underrun!");
        }
    }
}

struct DataCallbackState {
    current: Option<DataCallbackBuffer>,
    next: Option<DataCallbackBuffer>,
}

enum DataCallbackMessage {
    PlayNow { buffer: DataCallbackBuffer },
    PlayNext { buffer: DataCallbackBuffer },
}

struct DataCallbackBuffer {
    buffer: HeapRb<f32>,
    done: AtomicBool,
}
