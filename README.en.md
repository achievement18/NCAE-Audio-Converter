# NCAE Audio Converter · Multiformat Edition

[简体中文](README.md) · [Latest release](https://github.com/achievement18/NCAE-Audio-Converter/releases/latest) · [User guide](docs/USAGE.en.md) · [Formats](docs/FORMATS.en.md) · [Attribution](docs/ATTRIBUTION.en.md)

A native **Windows x64** desktop utility built with Rust / egui / eframe. Browse local NetEase Cloud Music `.ncae` effects, inspect IR and JSON previews, decrypt/export payloads, and generate, back up, replace or restore effects using your own impulse responses or parameter files.

**Only the multiformat edition is distributed, as one EXE.** No separately installed Python, Node.js or local web server is required. Documentation is bilingual; the application UI is currently primarily Chinese, not a fully localized English UI.

> **Back up before replacing.** By default, Generate backs up and directly replaces the selected effect. Check the target and only process files you are authorized to use. This is not an official NetEase product; modified effects are not guaranteed to work with every client version.

## Origin of the decryption core

The NCAE decryption research and algorithm originate from **[AllenHeartcore/audioeffect-ncm](https://github.com/AllenHeartcore/audioeffect-ncm)**, particularly [`ncae/decrypt.py`](https://github.com/AllenHeartcore/audioeffect-ncm/blob/b709cee76c6b4b0569a31c4345c3069aab831151/ncae/decrypt.py). Credit belongs to the original author for that reverse-engineering work.

This project does not claim to have independently discovered the algorithm. Early Python validation called the upstream decryptor; a local Python core was subsequently ported to Rust. Reverse packaging, input adapters, backup/restore, the desktop GUI, visualizations and bundled-runtime distribution were added afterwards. The upstream application's entire effect system or a GUI was not copied into this project. See [attribution and modification history](docs/ATTRIBUTION.en.md).

The project is distributed under **GNU GPL v3.0**, retaining the upstream license. Separate third-party MIT, Python, NumPy and other notices still apply. See [LICENSE](LICENSE) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Download and launch

1. Download `NCAE-Audio-Converter-Multiformat-<version>-windows-x64.exe` from [Releases](https://github.com/achievement18/NCAE-Audio-Converter/releases).
2. Place it in a writable directory and run it. First use does not need to download dependencies.
3. The application detects the Cloud Music installation and effect-data directory. Select a directory manually if detection fails.
4. Select an effect in the library to inspect its type and preview. Import a source file only when you want to generate/replace an effect.

### What “single EXE” means

Only one EXE needs to be downloaded or carried around; no adjacent `runtime` directory is required. The conversion backend and dependencies are compressed inside it.

On the first operation that needs the multiformat backend, it is extracted in a background task to:

```text
%LOCALAPPDATA%\NCAEAudioConverter\runtime\<version-content-id>\ready-…
```

Later processes validate and reuse the cache. Modified or unexpected files cause a fresh clean cache generation. Runtime cache and conversion temporary files are still written to disk: this is not an entirely in-memory application. You can remove the cache while the application is closed; it will be regenerated. Allow roughly 300 MB for cache, temporary files and updates, plus additional space for large inputs.

The application statically links its C/C++ runtime. Python's required DLLs are embedded and extracted with its backend. Windows system DLLs, graphics drivers and fonts remain operating-system dependencies. No native macOS, Linux or Windows ARM64 distribution is provided.

## Features

| Feature | Purpose and scope |
|---|---|
| Effect-directory detection | Checks installation, user-data and known candidate locations; supports manual selection and refresh. The install directory is not necessarily the effect-data directory. |
| Effect library | Search by name, compact IR / JSON / unknown badges, IR-first or name sorting, and adjustable library width. |
| Target suggestions | Identifies confirmed IR targets suitable for WAV replacement from actual file detection, rather than guessing by name. |
| File import | File picker, path entry and drag-and-drop; displays available format, channel and sample-rate information. |
| Multiformat input | Converts WAV, compatible IRS, FLAC, AIFF, AU, CAF, W64, RF64 and sample JSON / NPZ into NCAE audio payloads. |
| Parameter effects | Imports applicable parameter JSON separately from `ir.samples.v1` sample JSON. |
| Length processing | Full response, maximum-energy window matching the template length, or beginning-only truncation; shorter inputs are zero-padded in fixed-length modes. |
| Channel processing | Keep input channels, convert to mono or stereo; this is not a general surround downmixer. |
| Peak matching | Matches an available IR template's peak; not perceptual loudness normalization or an anti-clipping limiter. |
| Backup and replacement | Keeps the first original `.bak`, replaces the target and refreshes library/preview. |
| Restore | Restores a confirmed valid backup; missing, corrupted or ambiguous originals are not reconstructed. |
| Decrypted export | Exports WAV, JSON or unknown binary payloads from the selected NCAE; does not render or play the effect. |
| Source to WAV | Converts the imported source; can fall back to a selected IR effect when no source is imported. Parameter JSON is not directly converted into audio. |
| IR preview | Full-response FFT, channel selection, smoothing and display-only normalization for the selected effect. |
| JSON preview | Read-only plots/details for EQ, PEQ, reverb, rotation, tone, spatial, compressor and limiter modules. |
| Appearance | System-following or manual light/dark theme, transitions and hover feedback; collapsible, resizable, scrollable log. |
| Background work | Scanning, conversion and writes run off the UI thread, with phase/elapsed-time feedback and conflicting-operation protection. |

## Common workflows

### Replace an effect with your own IR

1. Download and enable an effect in Cloud Music, then refresh this application's library.
2. Choose a confirmed **IR** target and check its name and preview.
3. Import your WAV / IRS / FLAC or another supported source.
4. Start with Full response and Keep input channels. Enable peak matching only when wanted.
5. Keep direct replacement and original backup enabled, then generate.
6. In Cloud Music, switch to a different effect and back to reload it.

### Generate without changing the original

Disable direct replacement. The new NCAE is saved under `generated` beside the EXE, and the selected effect remains unchanged. The EXE directory must be writable.

### Export or convert to WAV

Use More operations → decrypted export or source to WAV. A completion dialog shows the actual output path. Existing filenames are avoided using numbered names rather than silently overwritten.

### Restore the original

Select the same target and choose Restore original effect in More operations. A valid backup is required; restoration cannot be guaranteed if no backup was made. See the [user guide](docs/USAGE.en.md).

## File locations

| Operation | Location | Naming / behavior |
|---|---|---|
| Normal replacement | Original effect directory | Updates the target; no extra Downloads export |
| First backup | Beside the original effect | `original-name.original-<identifier>.bak`; retained on later replacements |
| Generate only | `generated` beside the EXE | New NCAE; original unchanged |
| Decrypted export | Windows Downloads known folder | Library display name; extension follows payload type |
| Source to WAV | Windows Downloads known folder | Actual source stem; collisions receive numbered suffixes |
| Conversion runtime | Per-user local cache | Extracted on demand; not a personal-effect backup |

If the Downloads known folder cannot be located, the application logs a fallback output location. Always check the path in the completion dialog.

## Format support and limitations

- **Audio input:** `.wav` / `.wave`, `.irs`, `.flac`, `.aif` / `.aiff`, `.au` / `.snd`, `.caf`, `.w64`, `.rf64`.
- **Sample data:** `.json` / `.npz` using the `ir.samples.v1` schema.
- **Parameter data:** JSON compatible with the target client; distinct from sampled audio.
- **GUI output:** generated NCAE, decrypted payloads and source-to-WAV. Other audio-to-audio conversions are available through the source backend CLI, not a GUI output-format selector.
- IRS support covers readable audio files, particularly RIFF/WAVE containers, not every proprietary format sharing that extension.
- No automatic sample-rate conversion. Generated NCAE audio uses float32 WAV; higher-precision floating-point or integer data may lose precision.
- Container/subtype restrictions still apply, including FLAC's integer encodings. Arbitrary conversions are not advertised as universally lossless.
- Arbitrary EQ presets, ViPER presets or black-box DSP parameters cannot be reconstructed into genuine impulse responses.

See [format details](docs/FORMATS.en.md) for schemas, channel semantics and precision.

## Previews are read-only

Previews follow the selected library effect, **not the imported source**. Clicking a module changes the detail view, not the effect's actual enabled state. JSON EQ / PEQ plots show settings or confirmed models, not a measured response of the entire reverb/rotation/effect chain. Unknown private PEQ type numbers are not guessed. See the UI information tooltip and [user guide](docs/USAGE.en.md).

## Safety, privacy and compatibility

- Distribution assets and tests contain no personal effects, original backups, account details or replacement records. Test fixtures are synthetic.
- Detection, conversion and preview do not require a Cloud Music login or uploading effects.
- Only local files are modified. Client signatures, integrity checks and server-side verification are not bypassed; the client may reject, update or overwrite changed resources.
- Treat imported files as untrusted. Size, sample-count and decompression limits are defensive measures, not a complete security sandbox.
- Do not use multiple instances to replace the same target concurrently. Resolve file locks before retrying.
- Executables are not commercially code-signed and Windows may show a reputation warning. Download from this repository and verify the checksum; do not disable system-wide protection.

## Development, validation and source

- [Single-EXE build guide](docs/BUILD.en.md) / [中文构建说明](docs/BUILD.md)
- [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) · [Changelog](CHANGELOG.md)
- `apps/multiformat` is the only distribution target. `apps/stable` is retained as historical development reference and is not released as a second application.
- Cargo.lock pins Rust dependencies; build metadata records actual runtime versions.
- Release validation includes Rust tests, formatting, PE import inspection and isolated EXE-only smoke tests: fresh extraction, cache reuse, altered-cache recovery and actual WAV conversion.
- Backend/package self-tests do not establish GUI compatibility with every GPU, Windows release or Cloud Music client.

Each release also provides corresponding source and SHA-256 checksums. The source archive is not a second application required to run the EXE.
