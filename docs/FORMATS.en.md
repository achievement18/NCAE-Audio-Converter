# Formats, schemas and precision

[简体中文](FORMATS.md) · [Home](../README.en.md)

## Capability matrix

| Data | Input / NCAE generation | GUI source-to-WAV | Other backend conversions |
|---|---|---|---|
| WAV / WAVE, compatible IRS | Yes | Yes | Yes |
| FLAC, AIFF / AIF, AU / SND, CAF, W64, RF64 | Backend-decodable subtypes | Yes | Container-supported encodings |
| `ir.samples.v1` JSON / NPZ | Audio/IR payload | Yes | Yes |
| Parameter JSON | Parameter payload; client semantics must match | No | Not directly treated as audio |
| NCAE | Target template or export source | IR payload export | Decrypt first, then process its payload |
| MP3, AAC, OGG, proprietary encrypted IRS, arbitrary EQ presets | Not promised import formats | Not promised | Require explicit adapters, not renamed extensions |

NCAE is a container; IR and parameter JSON are distinct payloads. Generation uses a selected template, not arbitrary template-free client resource creation. The GUI has no general FLAC/AIFF output selector; backend CLI conversion must be distinguished from GUI features.

## `ir.samples.v1` JSON example

```json
{
  "schema": "ir.samples.v1",
  "sample_rate": 48000,
  "source_subtype": "DOUBLE",
  "samples": [[0.0, 0.0], [0.25, -0.25], [0.0, 0.0]]
}
```

`samples` is a nonempty finite two-dimensional `[frames, channels]` array with a positive integer sample rate. NPZ stores the same `schema`, `sample_rate`, `source_subtype` and `samples` fields; pickle loading is disabled. Unknown `ir.samples.*` schemas are rejected rather than mistaken for parameter presets.

## IRS scope

Supported IRS files are readable RIFF/WAVE or backend-supported audio. Written IRS files are WAV containers with an `.irs` extension, not a proprietary encrypted format. Corrupt/private IRS files are not silently packaged as opaque audio.

## Precision and processing

- NCAE audio generation uses float32 WAV. Higher-precision integer or float64 inputs may quantize.
- Source-to-WAV preserves supported source encoding where possible; the backend CLI can request a specific subtype.
- Native WAV processing supports PCM16 / PCM24 / PCM32 / float32; other supported encodings use the backend.
- Integer PCM output requires representable ranges; out-of-range values are not silently rescaled. Container/subtype combinations are validated.
- No automatic resampling. Length options only trim/pad; channel conversion follows the user guide.
- General backend conversion preserves sample rate, frame count and channel order without rendering EQ, reverb or other DSP.
- Equivalent-sample tests do not promise lossless preservation of all metadata, original compressed bytes or arbitrary precision.

## Defensive limits

The conversion bridge permits 1–8 channels and at most 16,777,216 samples. JSON files are limited to 128 MiB and declared expanded NPZ data to 256 MiB. Previews have separate file/frame/JSON-depth/node/NCAE-decompression limits; bridge thresholds are not a universal contract for every parser.

Non-finite samples, invalid sample rates, damaged containers and unknown sample schemas are rejected. These limits do not constitute a complete sandbox for malicious files.

## Preview interpretation

IR previews use the complete impulse response FFT rather than a truncated beginning. Channel choice, smoothing and normalization affect display only. JSON EQ/PEQ shows settings and confirmed models, not a fabricated measured response of reverb/rotation/the entire chain. Private numeric PEQ types are not assigned unverified filter meanings.
