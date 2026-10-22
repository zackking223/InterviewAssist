use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    SampleFormat, Stream, StreamConfig,
};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy)]
pub enum SourceKind {
    SystemAudio,
    Microphone,
}

pub enum AudioMessage {
    Samples(Vec<i16>),
    Flush,
    Stop,
}

pub struct AudioSession {
    stream: Stream,
    sender: mpsc::UnboundedSender<AudioMessage>,
}

impl AudioSession {
    pub fn pause(&self) -> Result<(), String> {
        self.stream.pause().map_err(|error| error.to_string())?;
        self.sender
            .send(AudioMessage::Flush)
            .map_err(|_| "transcription worker is no longer available".to_string())
    }

    pub fn resume(&self) -> Result<(), String> {
        self.stream.play().map_err(|error| error.to_string())
    }

    pub fn stop(self) {
        let _ = self.sender.send(AudioMessage::Stop);
        drop(self.stream);
    }
}

pub fn start(
    kind: SourceKind,
    sender: mpsc::UnboundedSender<AudioMessage>,
) -> Result<AudioSession, String> {
    let host = cpal::default_host();
    let device = match kind {
        SourceKind::SystemAudio => host.default_output_device(),
        SourceKind::Microphone => host.default_input_device(),
    }
    .ok_or_else(|| "no default audio device is available".to_string())?;
    let supported = match kind {
        SourceKind::SystemAudio => device.default_output_config(),
        SourceKind::Microphone => device.default_input_config(),
    }
    .map_err(|error| error.to_string())?;

    let sample_format = supported.sample_format();
    let config: StreamConfig = supported.into();
    let channels = config.channels as usize;
    let source_rate = config.sample_rate.0;
    let error_callback = |_error| {
        // Errors are surfaced by the control path. Never log audio data.
    };

    let stream = match sample_format {
        SampleFormat::F32 => {
            let mut converter = PcmConverter::new(source_rate, channels);
            let tx = sender.clone();
            device.build_input_stream(
                &config,
                move |data: &[f32], _| {
                    let pcm = converter.convert(
                        data.iter()
                            .copied()
                            .map(|v| (v.clamp(-1.0, 1.0) * i16::MAX as f32) as i16),
                    );
                    if !pcm.is_empty() {
                        let _ = tx.send(AudioMessage::Samples(pcm));
                    }
                },
                error_callback,
                None,
            )
        }
        SampleFormat::I16 => {
            let mut converter = PcmConverter::new(source_rate, channels);
            let tx = sender.clone();
            device.build_input_stream(
                &config,
                move |data: &[i16], _| {
                    let pcm = converter.convert(data.iter().copied());
                    if !pcm.is_empty() {
                        let _ = tx.send(AudioMessage::Samples(pcm));
                    }
                },
                error_callback,
                None,
            )
        }
        SampleFormat::U16 => {
            let mut converter = PcmConverter::new(source_rate, channels);
            let tx = sender.clone();
            device.build_input_stream(
                &config,
                move |data: &[u16], _| {
                    let pcm = converter.convert(data.iter().copied().map(|v| {
                        (v as i32 - 32_768).clamp(i16::MIN as i32, i16::MAX as i32) as i16
                    }));
                    if !pcm.is_empty() {
                        let _ = tx.send(AudioMessage::Samples(pcm));
                    }
                },
                error_callback,
                None,
            )
        }
        format => return Err(format!("unsupported audio sample format: {format:?}")),
    }
    .map_err(|error| error.to_string())?;

    stream.play().map_err(|error| error.to_string())?;
    Ok(AudioSession { stream, sender })
}

struct PcmConverter {
    channels: usize,
    step: f64,
    phase: f64,
}

impl PcmConverter {
    fn new(source_rate: u32, channels: usize) -> Self {
        Self {
            channels: channels.max(1),
            step: source_rate as f64 / 24_000.0,
            phase: 0.0,
        }
    }

    fn convert<I>(&mut self, input: I) -> Vec<i16>
    where
        I: IntoIterator<Item = i16>,
    {
        let raw: Vec<i16> = input.into_iter().collect();
        if raw.is_empty() {
            return Vec::new();
        }
        let mono: Vec<i16> = raw
            .chunks(self.channels)
            .map(|frame| {
                let sum: i64 = frame.iter().map(|sample| *sample as i64).sum();
                (sum / frame.len() as i64) as i16
            })
            .collect();

        let mut output = Vec::with_capacity((mono.len() as f64 / self.step + 1.0) as usize);
        while self.phase < mono.len() as f64 {
            output.push(mono[self.phase.floor() as usize]);
            self.phase += self.step;
        }
        self.phase -= mono.len() as f64;
        output
    }
}

#[cfg(test)]
mod tests {
    use super::PcmConverter;

    #[test]
    fn downmixes_stereo_and_resamples() {
        let mut converter = PcmConverter::new(48_000, 2);
        let input = vec![1_000, 3_000, 2_000, 4_000, 3_000, 5_000, 4_000, 6_000];
        assert_eq!(converter.convert(input), vec![2_000, 4_000]);
    }
}
