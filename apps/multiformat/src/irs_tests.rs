use super::*;
use crate::wav::{parse_wav, write_wav_float32};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ncae-irs-test-{}-{nonce}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn pcm_wave(bits: u16, channels: u16, payload: &[u8]) -> Vec<u8> {
    let mut wav = write_wav_float32(&[0.0], 44100, channels);
    wav.truncate(44);
    wav[4..8].copy_from_slice(&(36u32 + payload.len() as u32).to_le_bytes());
    wav[20..22].copy_from_slice(&1u16.to_le_bytes());
    let align = channels * (bits / 8);
    wav[28..32].copy_from_slice(&(44100u32 * u32::from(align)).to_le_bytes());
    wav[32..34].copy_from_slice(&align.to_le_bytes());
    wav[34..36].copy_from_slice(&bits.to_le_bytes());
    wav[40..44].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    wav.extend_from_slice(payload);
    wav
}

fn template(root: &Path) -> NcaeFile {
    let path = root.join("template.ncae");
    let wav = write_wav_float32(&[0.1, -0.1, 0.2, -0.2], 44100, 2);
    write_ncae(
        &path,
        &[1, 2, 3, 4],
        &[1, 2, 3, 4, 0],
        &[0, 0, 0, 0, 1, 0, 2, 0],
        &wav,
    )
    .unwrap();
    read_ncae(&path).unwrap()
}

#[test]
fn irs_float32_matches_wav_for_every_existing_conversion_option() {
    let fixture = Fixture::new();
    let template = template(&fixture.0);
    let bytes = write_wav_float32(&[0.0, 0.0, 0.3, -0.2, 0.1, 0.2], 44100, 2);
    let irs = fixture.0.join("input.IRS");
    let wav = fixture.0.join("input.wav");
    std::fs::write(&irs, &bytes).unwrap();
    std::fs::write(&wav, &bytes).unwrap();
    for mode in [LengthMode::Full, LengthMode::Length, LengthMode::Start] {
        for channels in [ChannelMode::Keep, ChannelMode::Mono, ChannelMode::Stereo] {
            for peak in [false, true] {
                let (irs_data, label) =
                    generate_from_template(&template, &irs, mode, channels, peak).unwrap();
                let (wav_data, _) =
                    generate_from_template(&template, &wav, mode, channels, peak).unwrap();
                assert_eq!(irs_data, wav_data);
                assert!(label.contains("IRS"));
                let (_, _, reserved, _) = parse_header(&irs_data).unwrap();
                assert_eq!(reserved[6], TYPE_WAV);
                assert!(parse_wav(&decrypt_ncae_bytes(&irs_data).unwrap()).is_ok());
            }
        }
    }
    assert_eq!(std::fs::read(&irs).unwrap(), bytes);
}

#[test]
fn pcm24_signed_extremes_and_irs_to_float32_are_correct() {
    let bytes = pcm_wave(24, 1, &[0, 0, 128, 0, 0, 0, 255, 255, 127, 255, 255, 255]);
    let parsed = parse_wav(&bytes).unwrap();
    assert_eq!(
        parsed.samples,
        vec![-1.0, 0.0, 8388607.0 / 8388608.0, -1.0 / 8388608.0]
    );
    let fixture = Fixture::new();
    let input = fixture.0.join("input.irs");
    std::fs::write(&input, &bytes).unwrap();
    let (data, _) = generate_from_template(
        &template(&fixture.0),
        &input,
        LengthMode::Full,
        ChannelMode::Keep,
        false,
    )
    .unwrap();
    let normalized = parse_wav(&decrypt_ncae_bytes(&data).unwrap()).unwrap();
    assert_eq!(normalized.format_tag, 3);
    assert_eq!(normalized.bits, 32);
    assert_eq!(normalized.samples, parsed.samples);
    assert_eq!(normalized.sample_rate, 44100);
}

#[test]
fn pcm16_and_pcm32_irs_are_supported() {
    let fixture = Fixture::new();
    let input = fixture.0.join("input.irs");
    for (bits, payload) in [
        (16, vec![0, 128, 255, 127]),
        (32, vec![0, 0, 0, 128, 255, 255, 255, 127]),
    ] {
        std::fs::write(&input, pcm_wave(bits, 1, &payload)).unwrap();
        let parsed = read_impulse_file(&input).unwrap();
        assert_eq!(parsed.frames, 2);
        assert_eq!(parsed.bits, bits);
        assert_eq!(parsed.samples[0], -1.0);
    }
}

#[test]
fn proprietary_or_truncated_irs_are_not_encrypted_as_opaque_payloads() {
    let fixture = Fixture::new();
    let input = fixture.0.join("unknown.irs");
    for data in [b"unknown proprietary IRS".as_slice(), b"RIFF".as_slice()] {
        std::fs::write(&input, data).unwrap();
        assert!(read_impulse_file(&input)
            .unwrap_err()
            .to_string()
            .contains("IRS 不是"));
        assert!(generate_from_template(
            &template(&fixture.0),
            &input,
            LengthMode::Full,
            ChannelMode::Keep,
            false
        )
        .is_err());
    }
}

#[test]
fn invalid_impulse_samples_are_rejected() {
    assert!(parse_wav(&write_wav_float32(&[f32::NAN], 44100, 1)).is_err());
    assert!(parse_wav(&write_wav_float32(&[f32::INFINITY], 44100, 1)).is_err());
    assert!(parse_wav(&write_wav_float32(&[0.1], 44100, 2)).is_err());
    assert!(parse_wav(&write_wav_float32(&[0.1], 0, 1)).is_err());
    assert!(parse_wav(&write_wav_float32(&[], 44100, 1)).is_err());
}
