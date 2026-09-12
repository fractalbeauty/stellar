use anyhow::Context;
use cpal::{
    FromSample, OutputCallbackInfo, Sample, SampleFormat, Stream,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use ringbuf::{
    CachingCons, HeapRb,
    traits::{Consumer, Observer},
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::Thread,
};
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

use crate::decode;

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
            SampleFormat::I8 => device.build_output_stream(
                output_config.config(),
                make_data_callback::<i8>(rx),
                error_callback,
                None,
            )?,
            SampleFormat::I16 => device.build_output_stream(
                output_config.config(),
                make_data_callback::<i16>(rx),
                error_callback,
                None,
            )?,
            SampleFormat::I32 => device.build_output_stream(
                output_config.config(),
                make_data_callback::<i32>(rx),
                error_callback,
                None,
            )?,
            SampleFormat::I64 => device.build_output_stream(
                output_config.config(),
                make_data_callback::<i64>(rx),
                error_callback,
                None,
            )?,
            SampleFormat::U8 => device.build_output_stream(
                output_config.config(),
                make_data_callback::<u8>(rx),
                error_callback,
                None,
            )?,
            SampleFormat::U16 => device.build_output_stream(
                output_config.config(),
                make_data_callback::<u16>(rx),
                error_callback,
                None,
            )?,
            SampleFormat::U32 => device.build_output_stream(
                output_config.config(),
                make_data_callback::<u32>(rx),
                error_callback,
                None,
            )?,
            SampleFormat::U64 => device.build_output_stream(
                output_config.config(),
                make_data_callback::<u64>(rx),
                error_callback,
                None,
            )?,
            SampleFormat::F32 => device.build_output_stream(
                output_config.config(),
                make_data_callback::<f32>(rx),
                error_callback,
                None,
            )?,
            SampleFormat::F64 => device.build_output_stream(
                output_config.config(),
                make_data_callback::<f64>(rx),
                error_callback,
                None,
            )?,
            other => anyhow::bail!("Unsupported sample format: {other:?}"),
        };

        tx.send(DataCallbackMessage::PlayNow {
            buffer: {
                let file = PathBuf::from(todo!());
                let (consumer, done, thread) = decode::spawn_decode(
                    file,
                    output_config.sample_rate() as usize,
                    output_config.channels() as usize,
                )?;

                DataCallbackBuffer {
                    buffer: consumer,
                    done,
                    thread,
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

            // Wake decoder thread
            current.thread.unpark();
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
    buffer: CachingCons<Arc<HeapRb<f32>>>,
    done: Arc<AtomicBool>,
    thread: Thread,
}
