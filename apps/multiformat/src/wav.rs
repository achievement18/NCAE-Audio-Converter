//! 简化的 WAV 读写和变换工具。
//! 支持 32-bit float PCM、16/24/32-bit PCM。

use anyhow::{bail, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthMode {
    Full,
    Length,
    Start,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelMode {
    Keep,
    Mono,
    Stereo,
}

#[derive(Debug, Clone)]
pub struct WavData {
    pub samples: Vec<f32>, // interleaved
    pub sample_rate: u32,
    pub channels: u16,
    pub bits: u16,
    pub format_tag: u16,
    pub frames: usize,
    pub peak: f32,
}

impl WavData {
    pub fn duration_ms(&self) -> f32 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.frames as f32 / self.sample_rate as f32 * 1000.0
        }
    }
}

pub fn parse_wav(data: &[u8]) -> Result<WavData> {
    if data.len() < 44 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        bail!("不是有效的 RIFF/WAVE 文件");
    }

    let mut pos = 12usize;
    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    let mut raw: Option<&[u8]> = None;

    while pos + 8 <= data.len() {
        let cid = &data[pos..pos + 4];
        let size = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let end = pos + 8 + size;
        if end > data.len() {
            break;
        }
        let chunk = &data[pos + 8..end];

        if cid == b"fmt " && chunk.len() >= 16 {
            let mut af = u16::from_le_bytes(chunk[0..2].try_into().unwrap());
            let channels = u16::from_le_bytes(chunk[2..4].try_into().unwrap());
            let sr = u32::from_le_bytes(chunk[4..8].try_into().unwrap());
            let bits = u16::from_le_bytes(chunk[14..16].try_into().unwrap());

            if af == 0xFFFE && chunk.len() >= 26 {
                af = u16::from_le_bytes(chunk[24..26].try_into().unwrap());
            }
            fmt = Some((af, channels, sr, bits));
        } else if cid == b"data" {
            raw = Some(chunk);
        }

        pos = end + (size % 2);
    }

    let (format_tag, channels, sample_rate, bits) =
        fmt.ok_or_else(|| anyhow::anyhow!("WAV 缺少 fmt 块"))?;
    let raw = raw.ok_or_else(|| anyhow::anyhow!("WAV 缺少 data 块"))?;

    if channels == 0 || channels > 8 {
        bail!("不支持的声道数: {}", channels);
    }

    if sample_rate == 0 {
        bail!("WAV 采样率不能为 0");
    }

    let samples: Vec<f32> = match (format_tag, bits) {
        (3, 32) => {
            if raw.len() % 4 != 0 {
                bail!("32-bit float data 长度不是 4 的倍数");
            }
            raw.chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                .collect()
        }
        (1, 16) => {
            if raw.len() % 2 != 0 {
                bail!("16-bit PCM data 长度不是 2 的倍数");
            }
            raw.chunks_exact(2)
                .map(|b| i16::from_le_bytes(b.try_into().unwrap()) as f32 / 32768.0)
                .collect()
        }
        (1, 24) => {
            if raw.len() % 3 != 0 {
                bail!("24-bit PCM data 长度不是 3 的倍数");
            }
            raw.chunks_exact(3)
                .map(|bytes| {
                    let value = (i32::from(bytes[0])
                        | (i32::from(bytes[1]) << 8)
                        | (i32::from(bytes[2]) << 16))
                        << 8
                        >> 8;
                    value as f32 / 8_388_608.0
                })
                .collect()
        }
        (1, 32) => {
            if raw.len() % 4 != 0 {
                bail!("32-bit PCM data 长度不是 4 的倍数");
            }
            raw.chunks_exact(4)
                .map(|b| i32::from_le_bytes(b.try_into().unwrap()) as f32 / 2147483648.0)
                .collect()
        }
        _ => bail!("暂不支持 WAV: format={} bits={}", format_tag, bits),
    };

    if samples.is_empty() {
        bail!("WAV 没有音频采样数据");
    }
    if samples.len() % channels as usize != 0 {
        bail!("WAV 采样数据不足以构成完整声道帧");
    }
    if samples.iter().any(|sample| !sample.is_finite()) {
        bail!("WAV 包含 NaN 或无穷大采样值，无法安全转换");
    }
    let frames = samples.len() / channels as usize;
    let peak = samples.iter().fold(0.0f32, |m, v| m.max(v.abs()));

    Ok(WavData {
        samples,
        sample_rate,
        channels,
        bits,
        format_tag,
        frames,
        peak,
    })
}

pub fn write_wav_float32(samples: &[f32], sample_rate: u32, channels: u16) -> Vec<u8> {
    let mut data = Vec::with_capacity(samples.len() * 4);
    for &s in samples {
        data.extend_from_slice(&s.to_le_bytes());
    }

    let block_align = channels * 4;
    let byte_rate = sample_rate * block_align as u32;
    let fmt = {
        let mut v = Vec::with_capacity(16);
        v.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
        v.extend_from_slice(&channels.to_le_bytes());
        v.extend_from_slice(&sample_rate.to_le_bytes());
        v.extend_from_slice(&byte_rate.to_le_bytes());
        v.extend_from_slice(&block_align.to_le_bytes());
        v.extend_from_slice(&32u16.to_le_bytes());
        v
    };

    let riff_size = 4 + (8 + fmt.len()) + (8 + data.len());
    let mut out = Vec::with_capacity(8 + riff_size);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(riff_size as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
    out.extend_from_slice(&fmt);
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    out
}

pub fn convert_channels(samples: &[f32], channels: u16, target: ChannelMode) -> (Vec<f32>, u16) {
    match target {
        ChannelMode::Keep => (samples.to_vec(), channels),
        ChannelMode::Mono => {
            if channels == 1 {
                return (samples.to_vec(), 1);
            }
            let ch = channels as usize;
            let mut out = Vec::with_capacity(samples.len() / ch);
            for frame in samples.chunks_exact(ch) {
                out.push(frame.iter().sum::<f32>() / ch as f32);
            }
            (out, 1)
        }
        ChannelMode::Stereo => {
            if channels == 2 {
                return (samples.to_vec(), 2);
            }
            if channels == 1 {
                let mut out = Vec::with_capacity(samples.len() * 2);
                for &x in samples {
                    out.push(x);
                    out.push(x);
                }
                return (out, 2);
            }
            let ch = channels as usize;
            let mut out = Vec::with_capacity((samples.len() / ch) * 2);
            for frame in samples.chunks_exact(ch) {
                out.push(frame[0]);
                out.push(frame.get(1).copied().unwrap_or(frame[0]));
            }
            (out, 2)
        }
    }
}

pub fn resample_length(
    samples: &[f32],
    channels: u16,
    target_frames: usize,
    mode: LengthMode,
) -> Vec<f32> {
    if mode == LengthMode::Full || target_frames == 0 {
        return samples.to_vec();
    }

    let ch = channels as usize;
    let frames = samples.len() / ch;

    let take = if mode == LengthMode::Start {
        target_frames.min(frames)
    } else if target_frames >= frames {
        frames
    } else {
        // 找能量最大的窗口
        let mut energy = Vec::with_capacity(frames);
        for i in 0..frames {
            let base = i * ch;
            let e: f32 = (0..ch).map(|c| samples[base + c] * samples[base + c]).sum();
            energy.push(e);
        }
        let mut window: f32 = energy[..target_frames].iter().sum();
        let mut best = window;
        let mut start = 0usize;
        for i in 1..=(frames - target_frames) {
            window += energy[i + target_frames - 1] - energy[i - 1];
            if window > best {
                best = window;
                start = i;
            }
        }
        let _ = best;
        let out = &samples[start * ch..(start + target_frames) * ch];
        return out.to_vec();
    };

    let mut out = samples[..take * ch].to_vec();
    if take < target_frames {
        out.extend(std::iter::repeat(0.0f32).take((target_frames - take) * ch));
    }
    out
}

pub fn peak_match(samples: &[f32], target_peak: f32) -> Vec<f32> {
    let peak = samples.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    if peak <= 0.0 || target_peak <= 0.0 {
        return samples.to_vec();
    }
    let scale = target_peak / peak;
    samples.iter().map(|v| v * scale).collect()
}

pub fn build_variant(
    input: &WavData,
    template: Option<&WavData>,
    mode: LengthMode,
    channel_mode: ChannelMode,
    do_peak_match: bool,
) -> Vec<u8> {
    let (mut samples, channels) = convert_channels(&input.samples, input.channels, channel_mode);

    if let Some(t) = template {
        samples = resample_length(&samples, channels, t.frames, mode);
        if do_peak_match {
            samples = peak_match(&samples, t.peak);
        }
    } else {
        if do_peak_match {
            samples = peak_match(&samples, input.peak);
        }
    }

    write_wav_float32(&samples, input.sample_rate, channels)
}
