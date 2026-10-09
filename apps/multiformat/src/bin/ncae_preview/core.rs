/* MIT License
 * Copyright (c) 2026 JSON Effect Viewer contributors
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
 * of the Software, and to permit persons to whom the Software is furnished to do
 * so, subject to the following conditions: The above copyright notice and this
 * permission notice shall be included in all copies or substantial portions of
 * the Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
 * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO MERCHANTABILITY, FITNESS FOR
 * A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
 * COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
 * IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
 * CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 *
 * NCAE JSON adapters use actual rvb/rotate/eq.eqs/peq.f sample structures.
 * Private numeric PEQ types are NEVER assigned a guessed filter type.
 * DSP models: independent implementation of W3C Audio EQ Cookbook equations.
 */
//! Native adaptation of the supplied preview cores. Read-only analysis; not a playback engine.
use anyhow::{bail, ensure, Context, Result};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
const MAX_BYTES: usize = 128 * 1024 * 1024;
const MAX_FRAMES: usize = 4_194_304;
pub const POINTS: usize = 360;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Options {
    pub channel: Option<usize>,
    pub smoothing: u8,
    pub normalize: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Ir,
    Json,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    On,
    Off,
    Unknown,
    Missing,
}
#[derive(Clone, Debug)]
pub struct Curve {
    pub module: &'static str,
    pub label: String,
    pub points: Vec<[f64; 2]>,
    pub silent: bool,
}
#[derive(Clone, Debug)]
pub struct Model {
    pub kind: Kind,
    pub curves: Vec<Curve>,
    pub controls: Vec<[f64; 2]>,
    pub markers: Vec<[f64; 2]>,
    pub modules: Vec<Module>,
    pub peq_bands: Vec<PeqBand>,
    pub warnings: Vec<String>,
    pub summary: String,
    pub channels: usize,
    pub min_hz: f64,
    pub max_hz: f64,
}
#[derive(Clone, Debug)]
pub struct Parameter {
    pub path: String,
    pub value: String,
}
#[derive(Clone, Debug)]
pub struct Module {
    pub key: String,
    pub label: String,
    pub state: State,
    pub parameters: Vec<Parameter>,
}
#[derive(Clone, Debug)]
pub struct PeqBand {
    pub band: String,
    pub state: State,
    pub frequency: String,
    pub gain: String,
    pub q: String,
    pub type_code: String,
    pub mapping: String,
}

fn value_text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(value) => value.to_string(),
        None => "—".into(),
    }
}
fn module_parameters(value: &Value, prefix: &str, result: &mut Vec<Parameter>) {
    if let Some(object) = value.as_object() {
        for (key, value) in object {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };
            module_parameters(value, &path, result);
        }
    } else {
        result.push(Parameter {
            path: prefix.into(),
            value: value_text(Some(value)),
        });
    }
}
fn module_detail(raw: &Value, key: &str, label: &str) -> Module {
    let mut parameters = vec![];
    if let Some(value) = raw.get(key).filter(|value| value.is_object()) {
        module_parameters(value, "", &mut parameters);
    }
    Module {
        key: key.into(),
        label: label.into(),
        state: state(raw.get(key)),
        parameters,
    }
}
fn peq_details(raw: &Value) -> Vec<PeqBand> {
    let module_state = state(raw.get("peq"));
    let Some(bands) = raw["peq"]["f"].as_array() else {
        return vec![];
    };
    bands
        .iter()
        .enumerate()
        .map(|(index, band)| {
            let band_state = state(Some(band));
            let kind = band.get("type").and_then(Value::as_str).filter(|kind| {
                [
                    "peaking",
                    "lowshelf",
                    "highshelf",
                    "lowpass",
                    "highpass",
                    "notch",
                    "bandpass",
                    "allpass",
                ]
                .contains(kind)
            });
            let mapping = if module_state != State::On {
                if module_state == State::Off {
                    "模块关闭".into()
                } else {
                    "模块状态未知".into()
                }
            } else if band_state != State::On {
                if band_state == State::Off {
                    "频段关闭".into()
                } else {
                    "频段状态未知".into()
                }
            } else if let Some(kind) = kind {
                let result = (|| {
                    let gain = if ["peaking", "lowshelf", "highshelf"].contains(&kind) {
                        number(band.get("gain")).context("缺少增益")?
                    } else {
                        0.0
                    };
                    biquad(
                        kind,
                        number(band.get("freq")).context("缺少频率")?,
                        gain,
                        number(band.get("q")).context("缺少 Q")?,
                        48000.0,
                    )
                })();
                match result {
                    Ok(_) => format!("{kind} 模型"),
                    Err(error) => error.to_string(),
                }
            } else {
                "类型未映射".into()
            };
            PeqBand {
                band: band
                    .get("band")
                    .map(|value| value_text(Some(value)))
                    .unwrap_or_else(|| (index + 1).to_string()),
                state: band_state,
                frequency: value_text(band.get("freq")),
                gain: value_text(band.get("gain")),
                q: value_text(band.get("q")),
                type_code: value_text(band.get("type")),
                mapping,
            }
        })
        .collect()
}

impl Model {
    pub fn module(&self, key: &str) -> Option<&Module> {
        self.modules.iter().find(|module| module.key == key)
    }
    pub fn default_module(&self) -> Option<&str> {
        self.modules
            .iter()
            .find(|module| module.state == State::On)
            .or_else(|| {
                self.modules
                    .iter()
                    .find(|module| module.state != State::Missing)
            })
            .map(|module| module.key.as_str())
    }
}

fn cancelled(cancel: &AtomicBool) -> Result<()> {
    ensure!(!cancel.load(Ordering::Relaxed), "预览已取消");
    Ok(())
}
fn u16_at(data: &[u8], at: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        data.get(at..at + 2).context("格式字段被截断")?.try_into()?,
    ))
}
fn u32_at(data: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        data.get(at..at + 4).context("格式字段被截断")?.try_into()?,
    ))
}
fn u64_at(data: &[u8], at: usize) -> Result<usize> {
    usize::try_from(u64::from_le_bytes(
        data.get(at..at + 8)
            .context("RF64 字段被截断")?
            .try_into()?,
    ))
    .context("长度超出平台范围")
}
#[derive(Clone, Debug)]
struct Wave {
    tag: u32,
    channels: usize,
    rate: u32,
    bits: u16,
    align: usize,
    offset: usize,
    frames: usize,
}
fn wave_meta(data: &[u8]) -> Result<Wave> {
    ensure!(data.len() <= MAX_BYTES, "预览文件超过 128 MiB 上限");
    ensure!(
        data.len() >= 12 && &data[8..12] == b"WAVE",
        "需要 RIFF/WAVE 或 RF64 音频"
    );
    let rf64 = &data[..4] == b"RF64";
    ensure!(rf64 || &data[..4] == b"RIFF", "不支持的音频容器");
    let mut end = if rf64 {
        data.len()
    } else {
        u32_at(data, 4)? as usize + 8
    };
    ensure!(end >= 12 && end <= data.len(), "WAV 长度无效或文件被截断");
    let mut position = 12;
    let mut fmt = None;
    let mut payload = None;
    let mut data64 = None;
    while position < end {
        ensure!(position + 8 <= end, "WAV 块头被截断");
        let id = &data[position..position + 4];
        let start = position + 8;
        let raw = u32_at(data, position + 4)?;
        let size = if raw == u32::MAX {
            ensure!(rf64 && id == b"data", "不支持的扩展块长度");
            data64.context("RF64 缺少 ds64")?
        } else {
            raw as usize
        };
        ensure!(size <= end - start, "WAV 数据块被截断");
        if id == b"ds64" && rf64 {
            ensure!(size >= 28 && data64.is_none(), "RF64 ds64 无效");
            data64 = Some(u64_at(data, start + 8)?);
            end = u64_at(data, start)?
                .checked_add(8)
                .context("RF64 长度溢出")?;
            ensure!(end <= data.len() && end >= start + size, "RF64 长度无效");
        }
        if id == b"fmt " {
            ensure!(fmt.is_none() && size >= 16, "WAV 格式块重复或不完整");
            let mut tag = u16_at(data, start)? as u32;
            let channels = u16_at(data, start + 2)? as usize;
            let rate = u32_at(data, start + 4)?;
            let align = u16_at(data, start + 12)? as usize;
            let bits = u16_at(data, start + 14)?;
            let mut valid = bits;
            if tag == 65534 {
                ensure!(
                    size >= 40 && u16_at(data, start + 16)? >= 22,
                    "扩展 WAV 格式不完整"
                );
                valid = u16_at(data, start + 18)?;
                if valid == 0 {
                    valid = bits;
                }
                ensure!(
                    data[start + 28..start + 40] == [0, 0, 16, 0, 128, 0, 0, 170, 0, 56, 155, 113],
                    "不支持的 WAV 子格式 GUID"
                );
                tag = u32_at(data, start + 24)?;
            }
            ensure!(
                (1..=32).contains(&channels) && rate > 0 && rate <= 768000,
                "声道数或采样率超出预览范围"
            );
            ensure!(
                (tag == 1 && [8, 16, 24, 32].contains(&bits))
                    || (tag == 3 && [32, 64].contains(&bits)),
                "预览仅支持 PCM 8/16/24/32 或 float32/64"
            );
            ensure!(
                valid > 0
                    && valid <= bits
                    && (tag != 3 || valid == bits)
                    && align == channels * (bits as usize / 8),
                "WAV 位深或声道对齐无效"
            );
            fmt = Some((tag, channels, rate, bits, align));
        }
        if id == b"data" {
            ensure!(payload.is_none(), "多个 data 块无法预览");
            payload = Some((start, size));
        }
        position = start + size + (size % 2);
        ensure!(position <= end, "WAV 缺少奇数字节填充");
    }
    ensure!(!rf64 || data64.is_some(), "RF64 缺少 ds64");
    let (tag, channels, rate, bits, align) = fmt.context("缺少 WAV 格式块")?;
    let (offset, size) = payload.context("缺少 WAV 采样数据")?;
    ensure!(size > 0 && size % align == 0, "音频为空或声道帧不完整");
    let frames = size / align;
    ensure!(frames <= MAX_FRAMES, "IR 超过 4,194,304 帧，未截短分析");
    Ok(Wave {
        tag,
        channels,
        rate,
        bits,
        align,
        offset,
        frames,
    })
}
fn decode_channel(data: &[u8], meta: &Wave, channel: usize) -> Result<Vec<f64>> {
    let mut samples = Vec::with_capacity(meta.frames);
    for frame in 0..meta.frames {
        let at = meta.offset + frame * meta.align + channel * (meta.bits as usize / 8);
        let value = match (meta.tag, meta.bits) {
            (3, 32) => f32::from_le_bytes(data[at..at + 4].try_into()?) as f64,
            (3, 64) => f64::from_le_bytes(data[at..at + 8].try_into()?),
            (1, 8) => (data[at] as f64 - 128.0) / 128.0,
            (1, 16) => i16::from_le_bytes(data[at..at + 2].try_into()?) as f64 / 32768.0,
            (1, 24) => {
                let value = ((data[at] as i32)
                    | ((data[at + 1] as i32) << 8)
                    | ((data[at + 2] as i32) << 16))
                    << 8
                    >> 8;
                value as f64 / 8388608.0
            }
            (1, 32) => i32::from_le_bytes(data[at..at + 4].try_into()?) as f64 / 2147483648.0,
            _ => unreachable!(),
        };
        ensure!(value.is_finite(), "IR 含非有限采样值");
        samples.push(value);
    }
    Ok(samples)
}
fn spectrum(samples: &[f64], cancel: &AtomicBool) -> Result<(Vec<f64>, usize)> {
    let n = samples.len().next_power_of_two().max(65536);
    let mut re = vec![0.0; n];
    let mut im = vec![0.0; n];
    re[..samples.len()].copy_from_slice(samples);
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        cancelled(cancel)?;
        let angle = -2.0 * std::f64::consts::PI / len as f64;
        let wr0 = angle.cos();
        let wi0 = angle.sin();
        for start in (0..n).step_by(len) {
            let mut wr = 1.0;
            let mut wi = 0.0;
            for j in 0..len / 2 {
                let a = start + j;
                let b = a + len / 2;
                let tr = re[b] * wr - im[b] * wi;
                let ti = re[b] * wi + im[b] * wr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let next = wr * wr0 - wi * wi0;
                wi = wr * wi0 + wi * wr0;
                wr = next;
            }
        }
        len *= 2;
    }
    let power: Vec<f64> = (0..=n / 2).map(|i| re[i] * re[i] + im[i] * im[i]).collect();
    ensure!(power.iter().all(|p| p.is_finite()), "IR FFT 数值溢出");
    Ok((power, n))
}
fn to_db(power: f64) -> f64 {
    if power > 0.0 {
        10.0 * power.log10()
    } else {
        f64::NEG_INFINITY
    }
}
fn response(
    power: &[f64],
    rate: f64,
    n: usize,
    options: Options,
    points: usize,
) -> Result<(Vec<[f64; 2]>, bool)> {
    let min = 20.0;
    let max = 20000.0_f64.min(rate / 2.0);
    ensure!(max > min, "采样率过低，无法显示 20 Hz 以上响应");
    let ratio = max / min;
    let df = rate / n as f64;
    let mut xy = Vec::new();
    let mut left = 0;
    let mut right = 0;
    let mut sum = 0.0;
    for index in 0..points {
        let frequency = min * ratio.powf(index as f64 / (points - 1) as f64);
        if options.smoothing != 0 {
            let factor = 2.0_f64.powf(1.0 / (2.0 * options.smoothing as f64));
            let lo = (frequency / factor / df).ceil().max(0.0) as usize;
            let hi = ((frequency * factor / df).floor() as usize).min(power.len() - 1);
            let value = if hi < lo {
                power[(frequency / df).round() as usize]
            } else {
                while right <= hi {
                    sum += power[right];
                    right += 1;
                }
                while left < lo {
                    sum -= power[left];
                    left += 1;
                }
                (sum / (hi - lo + 1) as f64).max(0.0)
            };
            xy.push([frequency, to_db(value)]);
            continue;
        }
        let low = if index == 0 {
            min
        } else {
            min * ratio.powf((index as f64 - 0.5) / (points - 1) as f64)
        };
        let high = if index == points - 1 {
            max
        } else {
            min * ratio.powf((index as f64 + 0.5) / (points - 1) as f64)
        };
        let lo = (low / df).ceil().max(0.0) as usize;
        let hi = ((high / df).floor() as usize).min(power.len() - 1);
        if hi >= lo {
            let mut imin = lo;
            let mut imax = lo;
            for bin in lo + 1..=hi {
                if power[bin] < power[imin] {
                    imin = bin;
                }
                if power[bin] > power[imax] {
                    imax = bin;
                }
            }
            let mut bins = vec![imin];
            if imax != imin {
                bins.push(imax);
                bins.sort_unstable();
            }
            for bin in bins {
                xy.push([(bin as f64 * df).max(min), to_db(power[bin])]);
            }
        } else {
            let bin = frequency / df;
            let index = bin.floor() as usize;
            let t = bin - index as f64;
            xy.push([
                frequency,
                to_db(
                    power[index] * (1.0 - t)
                        + power.get(index + 1).copied().unwrap_or(power[index]) * t,
                ),
            ]);
        }
    }
    let peak = xy.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
    let silent = !peak.is_finite();
    let offset = if options.normalize && !silent {
        peak
    } else {
        0.0
    };
    for point in &mut xy {
        point[1] = (point[1] - offset).max(-180.0);
    }
    Ok((xy, silent))
}
pub fn ir_model(
    data: &[u8],
    options: Options,
    points: usize,
    cancel: &AtomicBool,
) -> Result<Model> {
    ensure!(
        [0, 3, 6, 12, 24, 48].contains(&options.smoothing) && points >= 32,
        "预览参数无效"
    );
    let meta = wave_meta(data)?;
    let channels = match options.channel {
        Some(channel) => vec![channel.min(meta.channels - 1)],
        None => (0..meta.channels.min(2)).collect(),
    };
    let mut curves = Vec::new();
    let mut fft_size = 0;
    for channel in channels {
        cancelled(cancel)?;
        let samples = decode_channel(data, &meta, channel)?;
        let (power, n) = spectrum(&samples, cancel)?;
        fft_size = n;
        let (points, silent) = response(&power, meta.rate as f64, n, options, points)?;
        curves.push(Curve {
            module: "ir",
            label: format!("声道 {}", channel + 1),
            points,
            silent,
        });
    }
    Ok(Model {
        kind: Kind::Ir,
        curves,
        controls: vec![],
        markers: vec![],
        modules: vec![],
        peq_bands: vec![],
        warnings: vec![
            "由当前音效的完整 IR 计算；未加窗、未截短。平滑和归一化仅影响预览显示。".into(),
        ],
        summary: format!(
            "{} Hz · {} 声道 · {} 帧 · {:.1} ms · FFT {} / {:.2} Hz",
            meta.rate,
            meta.channels,
            meta.frames,
            meta.frames as f64 / meta.rate as f64 * 1000.0,
            fft_size,
            meta.rate as f64 / fft_size as f64
        ),
        channels: meta.channels,
        min_hz: 20.0,
        max_hz: 20000.0_f64.min(meta.rate as f64 / 2.0),
    })
}
fn state(value: Option<&Value>) -> State {
    match value {
        None | Some(Value::Null) => State::Missing,
        Some(value) => match value.get("on").and_then(Value::as_bool) {
            Some(true) => State::On,
            Some(false) => State::Off,
            None => State::Unknown,
        },
    }
}
fn number(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64).filter(|v| v.is_finite())
}
#[derive(Clone, Debug)]
struct Filter {
    b: [f64; 3],
    a: [f64; 3],
}
fn biquad(kind: &str, frequency: f64, gain: f64, q: f64, rate: f64) -> Result<Filter> {
    ensure!(
        frequency > 0.0
            && frequency < rate / 2.0
            && q.is_finite()
            && (0.0001..=10000.0).contains(&q)
            && gain.is_finite()
            && gain.abs() <= 120.0,
        "频率/Q/增益超出模型范围"
    );
    let w = 2.0 * std::f64::consts::PI * frequency / rate;
    let c = w.cos();
    let alpha = w.sin() / (2.0 * q);
    let a = 10.0_f64.powf(gain / 40.0);
    let beta = 2.0 * a.sqrt() * alpha;
    let (b, d) = match kind {
        "peaking" => (
            [1.0 + alpha * a, -2.0 * c, 1.0 - alpha * a],
            [1.0 + alpha / a, -2.0 * c, 1.0 - alpha / a],
        ),
        "lowpass" => (
            [(1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0],
            [1.0 + alpha, -2.0 * c, 1.0 - alpha],
        ),
        "highpass" => (
            [(1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0],
            [1.0 + alpha, -2.0 * c, 1.0 - alpha],
        ),
        "notch" => ([1.0, -2.0 * c, 1.0], [1.0 + alpha, -2.0 * c, 1.0 - alpha]),
        "bandpass" => ([alpha, 0.0, -alpha], [1.0 + alpha, -2.0 * c, 1.0 - alpha]),
        "allpass" => (
            [1.0 - alpha, -2.0 * c, 1.0 + alpha],
            [1.0 + alpha, -2.0 * c, 1.0 - alpha],
        ),
        "lowshelf" => (
            [
                a * ((a + 1.0) - (a - 1.0) * c + beta),
                2.0 * a * ((a - 1.0) - (a + 1.0) * c),
                a * ((a + 1.0) - (a - 1.0) * c - beta),
            ],
            [
                (a + 1.0) + (a - 1.0) * c + beta,
                -2.0 * ((a - 1.0) + (a + 1.0) * c),
                (a + 1.0) + (a - 1.0) * c - beta,
            ],
        ),
        "highshelf" => (
            [
                a * ((a + 1.0) + (a - 1.0) * c + beta),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * c),
                a * ((a + 1.0) + (a - 1.0) * c - beta),
            ],
            [
                (a + 1.0) - (a - 1.0) * c + beta,
                2.0 * ((a - 1.0) - (a + 1.0) * c),
                (a + 1.0) - (a - 1.0) * c - beta,
            ],
        ),
        _ => bail!("未知滤波器类型"),
    };
    Ok(Filter {
        b: b.map(|v| v / d[0]),
        a: d.map(|v| v / d[0]),
    })
}
fn gain_at(filter: &Filter, frequency: f64, rate: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * frequency / rate;
    let mag = |v: [f64; 3]| {
        (v[0] + v[1] * w.cos() + v[2] * (2.0 * w).cos())
            .hypot(-v[1] * w.sin() - v[2] * (2.0 * w).sin())
    };
    20.0 * (mag(filter.b).max(f64::from_bits(1)) / mag(filter.a).max(f64::from_bits(1))).log10()
}
fn check_json(value: &Value, depth: usize, nodes: &mut usize) -> Result<()> {
    *nodes += 1;
    ensure!(
        *nodes <= 20000 && depth <= 30,
        "JSON 节点或嵌套超过预览上限"
    );
    match value {
        Value::Object(map) => {
            for value in map.values() {
                check_json(value, depth + 1, nodes)?
            }
        }
        Value::Array(values) => {
            for value in values {
                check_json(value, depth + 1, nodes)?
            }
        }
        _ => {}
    }
    Ok(())
}
pub fn json_model(data: &[u8], points: usize) -> Result<Model> {
    ensure!((32..=10000).contains(&points), "预览点数无效");
    ensure!(data.len() <= 1024 * 1024, "JSON 预览上限为 1 MiB");
    let text = std::str::from_utf8(data)?.trim_start_matches('\u{feff}');
    let raw: Value = serde_json::from_str(text)?;
    ensure!(raw.is_object(), "音效参数必须是 JSON 对象");
    check_json(&raw, 0, &mut 0)?;
    let keys = [
        ("eq", "EQ"),
        ("peq", "PEQ"),
        ("rvb", "混响"),
        ("rotate", "旋转"),
        ("bt", "音调"),
        ("se", "声场"),
        ("cmp", "压缩"),
        ("limiter", "限幅"),
    ];
    let modules = keys
        .iter()
        .map(|(key, label)| module_detail(&raw, key, label))
        .collect();
    let peq_bands = peq_details(&raw);
    let eq = state(raw.get("eq"));
    let peq = state(raw.get("peq"));
    let mut warnings = vec![];
    let mut controls = vec![];
    let mut markers = vec![];
    let mut curves = vec![];
    let grid: Vec<f64> = (0..points)
        .map(|i| 20.0 * 1000.0_f64.powf(i as f64 / (points - 1) as f64))
        .collect();
    if eq == State::On {
        let gains = raw["eq"]["eqs"].as_array();
        if let Some(gains) =
            gains.filter(|g| g.len() == 10 && g.iter().all(|v| number(Some(v)).is_some()))
        {
            for (f, g) in [
                31.0, 62.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
            ]
            .into_iter()
            .zip(gains)
            {
                controls.push([f, g.as_f64().unwrap()]);
            }
            let interpolate = |f: f64| {
                if f <= controls[0][0] {
                    return controls[0][1];
                }
                if f >= controls[9][0] {
                    return controls[9][1];
                }
                let i = (1..10).find(|&i| controls[i][0] >= f).unwrap();
                let [f0, g0] = controls[i - 1];
                let [f1, g1] = controls[i];
                g0 + (g1 - g0) * (f / f0).ln() / (f1 / f0).ln()
            };
            curves.push(Curve {
                module: "eq",
                label: "十段 EQ 设置参考（非真实频响）".into(),
                points: grid
                    .iter()
                    .map(|&f| [f, interpolate(f).clamp(-180.0, 180.0)])
                    .collect(),
                silent: false,
            });
            warnings.push("中心频率采用 31/62/125/250/500/1k/2k/4k/8k/16k Hz 展示假设；增益按 dB 展示，不代表网易内部滤波实现。".into());
        } else {
            warnings.push("eq.eqs 数量或数值无效，未伪造 EQ 曲线。".into());
        }
    }
    if peq == State::On {
        if let Some(bands) = raw["peq"]["f"].as_array() {
            let mut ready = true;
            let preamp = match raw["peq"].get("gain") {
                None => 0.0,
                Some(value) => match number(Some(value)) {
                    Some(value) => value,
                    None => {
                        ready = false;
                        warnings.push("PEQ 总增益无效".into());
                        0.0
                    }
                },
            };
            let mut filters = vec![];
            for (index, band) in bands.iter().enumerate() {
                let s = state(Some(band));
                if s == State::Off {
                    continue;
                }
                if s != State::On {
                    ready = false;
                    warnings.push(format!("PEQ #{} 开关状态未知", index + 1));
                    continue;
                }
                let frequency = number(band.get("freq"));
                let gain = number(band.get("gain"));
                if let (Some(f), Some(g)) = (frequency, gain) {
                    if f > 0.0 {
                        markers.push([f, g]);
                    }
                }
                let kind = band.get("type").and_then(Value::as_str).filter(|kind| {
                    [
                        "peaking",
                        "lowshelf",
                        "highshelf",
                        "lowpass",
                        "highpass",
                        "notch",
                        "bandpass",
                        "allpass",
                    ]
                    .contains(kind)
                });
                let Some(kind) = kind else {
                    ready = false;
                    warnings.push(format!(
                        "PEQ #{} 未映射类型码 {}，只保留参数，不猜测频响。",
                        index + 1,
                        band.get("type").unwrap_or(&Value::Null)
                    ));
                    continue;
                };
                let gain = if ["peaking", "lowshelf", "highshelf"].contains(&kind) {
                    gain
                } else {
                    Some(0.0)
                };
                let result = (|| {
                    biquad(
                        kind,
                        frequency.context("缺少频率")?,
                        gain.context("缺少增益")?,
                        number(band.get("q")).context("缺少 Q")?,
                        48000.0,
                    )
                })();
                match result {
                    Ok(filter) => filters.push(filter),
                    Err(error) => {
                        ready = false;
                        warnings.push(format!("PEQ #{} 无法解析：{error}", index + 1));
                    }
                }
            }
            if ready || !filters.is_empty() {
                curves.push(Curve {
                    module: "peq",
                    label: if ready {
                        "PEQ：RBJ 模型估算"
                    } else {
                        "PEQ：仅已解析部分"
                    }
                    .into(),
                    points: grid
                        .iter()
                        .map(|&f| {
                            [
                                f,
                                (preamp
                                    + filters
                                        .iter()
                                        .map(|filter| gain_at(filter, f, 48000.0))
                                        .sum::<f64>())
                                .clamp(-180.0, 180.0),
                            ]
                        })
                        .collect(),
                    silent: false,
                });
            }
            if !filters.is_empty() {
                warnings.push(
                    "PEQ 使用明示滤波类型、Hz/dB/Q 和 48 kHz 的标准模型，不代表原播放器实际响应。"
                        .into(),
                );
            }
        } else {
            warnings.push("peq.f 缺失或无效，未绘制 PEQ 曲线。".into());
        }
    }
    if eq == State::Unknown || peq == State::Unknown {
        warnings.push("存在开关状态未知的 EQ 模块，无法判断完整响应。".into());
    }
    if eq != State::On && peq != State::On && eq != State::Unknown && peq != State::Unknown {
        curves.push(Curve {
            module: "eq",
            label: "无已开启 EQ：0 dB 参考线（非整套效果）".into(),
            points: grid.iter().map(|&f| [f, 0.0]).collect(),
            silent: false,
        });
    }
    warnings.push("只描述 EQ 设置或可确认的模型，不含混响、旋转和音调等效果的实际响应。".into());
    Ok(Model {
        kind: Kind::Json,
        curves,
        controls,
        markers,
        modules,
        peq_bands,
        warnings,
        summary: "参数音效 · 设置/模型参考图，不是整套音效实测频响".into(),
        channels: 0,
        min_hz: 20.0,
        max_hz: 20000.0,
    })
}
pub fn analyze(data: &[u8], options: Options, cancel: &AtomicBool) -> Result<Model> {
    cancelled(cancel)?;
    if data.starts_with(b"RIFF") || data.starts_with(b"RF64") {
        ir_model(data, options, POINTS, cancel)
    } else {
        json_model(data, POINTS)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/preview-fixtures")
    }
    fn close(a: f64, b: f64) {
        assert!((a - b).abs() <= 1e-6 * (1.0 + b.abs()), "{a} != {b}");
    }
    #[test]
    fn ir_matches_supplied_javascript_core() {
        let expected: Value =
            serde_json::from_slice(&std::fs::read(fixture().join("reference.json")).unwrap())
                .unwrap();
        let cancel = AtomicBool::new(false);
        for case in expected["ir"].as_array().unwrap() {
            let bytes = std::fs::read(fixture().join(case["file"].as_str().unwrap())).unwrap();
            let options = Options {
                channel: Some(case["options"]["channel"].as_u64().unwrap() as usize),
                smoothing: case["options"]["smoothing"].as_u64().unwrap() as u8,
                normalize: case["options"]["normalize"].as_bool().unwrap(),
            };
            let model = ir_model(&bytes, options, 72, &cancel).unwrap();
            let actual = &model.curves[0];
            let points = case["points"].as_array().unwrap();
            assert_eq!(actual.points.len(), points.len());
            assert_eq!(actual.silent, case["silent"].as_bool().unwrap());
            for (actual, expected) in actual.points.iter().zip(points) {
                close(actual[0], expected[0].as_f64().unwrap());
                close(actual[1], expected[1].as_f64().unwrap());
            }
        }
    }
    #[test]
    fn json_matches_supplied_javascript_core() {
        let expected: Value =
            serde_json::from_slice(&std::fs::read(fixture().join("reference.json")).unwrap())
                .unwrap();
        for case in expected["json"].as_array().unwrap() {
            let model = json_model(&serde_json::to_vec(&case["input"]).unwrap(), 72).unwrap();
            let curves = case["curves"].as_array().unwrap();
            assert_eq!(model.curves.len(), curves.len(), "{}", case["input"]);
            for (curve, expected) in model.curves.iter().zip(curves) {
                let expected = expected.as_array().unwrap();
                assert_eq!(curve.points.len(), expected.len());
                for (actual, expected) in curve.points.iter().zip(expected) {
                    close(actual[0], expected[0].as_f64().unwrap());
                    close(actual[1], expected[1].as_f64().unwrap());
                }
            }
            assert_eq!(
                model.controls.len(),
                case["controls"].as_array().unwrap().len()
            );
            assert_eq!(
                model.markers.len(),
                case["markers"].as_array().unwrap().len()
            );
        }
    }
    #[test]
    fn numeric_peq_types_are_never_guessed() {
        let model = json_model(
            br#"{"peq":{"on":true,"f":[{"on":true,"type":4,"freq":1000,"gain":3,"q":1000}]}}"#,
            72,
        )
        .unwrap();
        assert!(model.curves.is_empty());
        assert_eq!(model.markers.len(), 1);
        assert!(model.warnings.iter().any(|s| s.contains("未映射类型码")));
    }
    #[test]
    fn silence_normalization_does_not_create_a_fake_zero_db_curve() {
        let wave = ncae_tool::wav::write_wav_float32(&[0.0; 8], 48000, 1);
        let model = ir_model(
            &wave,
            Options {
                normalize: true,
                ..Default::default()
            },
            72,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(model.curves[0].silent);
        assert!(model.curves[0]
            .points
            .iter()
            .all(|point| point[1] == -180.0));
    }
    #[test]
    fn invalid_and_cancelled_preview_inputs_fail_safely() {
        assert!(json_model(b"[]", 72).is_err());
        assert!(ir_model(b"RIFF", Options::default(), 72, &AtomicBool::new(false)).is_err());
        let wave = ncae_tool::wav::write_wav_float32(&[1.0], 48000, 1);
        assert!(ir_model(&wave, Options::default(), 72, &AtomicBool::new(true)).is_err());
    }

    #[test]
    fn module_details_match_the_supplied_viewer_core() {
        let reference: Value = serde_json::from_slice(
            &std::fs::read(fixture().join("module-reference.json")).unwrap(),
        )
        .unwrap();
        let model = json_model(&serde_json::to_vec(&reference["input"]).unwrap(), 72).unwrap();
        for module in &model.modules {
            let expected = &reference["modules"][&module.key];
            assert_eq!(
                module.state,
                match expected["state"].as_str().unwrap() {
                    "on" => State::On,
                    "off" => State::Off,
                    "missing" => State::Missing,
                    _ => State::Unknown,
                }
            );
            let params = expected["parameters"].as_array().unwrap();
            assert_eq!(module.parameters.len(), params.len());
            for param in params {
                let actual = module
                    .parameters
                    .iter()
                    .find(|value| value.path == param["path"].as_str().unwrap())
                    .unwrap();
                assert_eq!(actual.value, value_text(param.get("value")));
            }
        }
        assert!(model.curves.iter().any(|curve| curve.module == "eq"));
        assert!(model.curves.iter().any(|curve| curve.module == "peq"));
        assert!(model.peq_bands[0].mapping.contains("peaking"));
        assert!(model.peq_bands[1].mapping.contains("未映射"));
        assert_eq!(model.peq_bands[2].state, State::Off);
    }
    #[test]
    fn disabled_modules_preserve_their_parameters_without_building_responses() {
        let model=json_model(br#"{"eq":{"on":false,"eqs":[1,2,3]},"peq":{"on":false,"f":[{"on":true,"type":4,"freq":1000,"gain":3,"q":1000}]},"rvb":{"on":false,"room":35}}"#,72).unwrap();
        assert_eq!(model.module("rvb").unwrap().state, State::Off);
        assert!(model
            .module("rvb")
            .unwrap()
            .parameters
            .iter()
            .any(|parameter| parameter.path == "room" && parameter.value == "35"));
        assert_eq!(model.peq_bands.len(), 1);
        assert_eq!(model.peq_bands[0].mapping, "模块关闭");
        assert!(!model.curves.iter().any(|curve| curve.module == "peq"));
    }
}
