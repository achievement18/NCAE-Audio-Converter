# User guide

[简体中文](USAGE.md) · [Home](../README.en.md)

## 1. Preparation

Use the released multiformat EXE in a writable directory; the source ZIP is not required beside it. First conversion automatically extracts the backend under `%LOCALAPPDATA%\NCAEAudioConverter\runtime`. Keep the application open while work is running.

The tool does not provide an effect library of audio content, play listening previews or download Cloud Music effects. Download the target effect in the client first.

## 2. Locate and select an effect

Client installation and effect-data locations are detected separately, checking user data and known candidates. `%LOCALAPPDATA%\NetEase\CloudMusic\audioeffect` is common, not universal.

Use a manual directory if detection fails. Download effects in the client and refresh if the library is empty. Search filters display names; IR-first sorting and name sorting are available. IR / JSON / unknown badges reflect file detection, not guaranteed client acceptance.

Drag the library boundary to change its width. Display names may omit `.ncae` and generated numeric suffixes without renaming actual files.

## 3. Import a source

Use the file picker, path input or drag-and-drop. See [formats](FORMATS.en.md). Inspection runs in the background. Read errors rather than renaming an unsupported file's extension.

Parameter JSON stores settings; `ir.samples.v1` JSON / NPZ stores actual samples. They are different data types. The preview remains the selected library target, not the imported source.

## 4. Generation settings

| Setting | Actual behavior |
|---|---|
| Full response | Keeps the complete input response, including its tail. |
| Equal-length energy window | Uses the original IR's frame count to select the highest-energy contiguous window; pads shorter inputs with zeros. |
| Beginning only | Keeps the beginning at the template length; pads short inputs or discards the excess tail. |
| Keep channels | Keeps input channels; does not force the template's channel layout. |
| Mono | Arithmetic average of input channels. |
| Stereo | Duplicates mono; for more than two channels, takes the first two rather than applying a surround downmix matrix. |
| Peak matching | Scales against an available IR target's maximum absolute sample, not LUFS loudness. |

Length processing is trimming/padding, not sample-rate resampling. Without a usable WAV template, template-dependent length/peak processing cannot have the same effect. Packaging audio into a parameter-type target does not establish client compatibility; prefer a confirmed IR target.

## 5. Generate and replace

Direct replacement and backup are enabled by default. Check the target name before starting. Background tasks show phase and elapsed time without inventing percentages; conflicting operations are restricted.

The first original is backed up as an adjacent `.bak`. Later replacements do not overwrite it with already modified content. The filename identifier distinguishes contents; it is not a cryptographic signature.

After replacement, library and preview refresh while selection is retained. Normal replacement does not export an extra Downloads copy. The client may cache effects: switch to another effect and back, or reload through normal client operation.

Disabling backup requires confirmation. Restoration may be impossible without another valid original. Do not run concurrent instances writing the same target.

## 6. Generate only and restore

Disable direct replacement to write a new NCAE under `generated` beside the EXE, leaving the original unchanged. The EXE directory must be writable.

To restore, select the original target and use More operations → Restore original effect. Valid adjacent backups are preferred; historical records are a compatibility fallback. Missing, corrupted or ambiguous originals are not guessed. Restoration also refreshes library and preview.

Old private installations may contain `replacement_records.json` or `legacy-backups`. These are not release assets and must not be uploaded to issues, repositories or shared packages. A missing original cannot be recreated from a record alone.

## 7. Two export operations

### Decrypted export

Exports the selected NCAE's WAV, JSON or unknown binary payload using the appropriate extension. The stem follows the library display name.

### Source to WAV

Uses the imported source first; when no source is supplied, a selected IR effect can be used. The actual source stem is retained, including meaningful dots and numbers, instead of the target template's name.

This operation does not apply generation-panel trimming, channel conversion or peak matching. It preserves sample rate, channels, frame count and supported encoding where possible; encoding compatibility and quantization limits remain.

Both GUI operations default to Windows Downloads, avoid collisions through numbering, and show the actual output path in a completion dialog. Parameter JSON is not renderable audio and is not fabricated into WAV. Manually addressed backend/CLI output does not inherit every GUI collision rule.

## 8. Previews

- IR: full-response FFT, channels, smoothing and display-only normalization; silence is not presented as a flat 0 dB response.
- JSON: EQ / PEQ / reverb / rotation / tone / spatial / compressor / limiter tabs show details and the original enabled state.
- Tabs do not edit module switches. Disabled modules may still contain inspectable parameters.
- EQ/PEQ models are not measured responses of the entire DSP chain. Private types are not guessed; see the information tooltip.
- Stale calculations do not replace a newer selection. Imported sources are not an additional preview target.

## 9. Theme and logs

The default theme follows the system. Use the icon to switch manually; right-click it to return to system-following. Themes and hover colors have short transitions.

The log defaults to roughly five small-text lines. Collapse, scroll or drag its top edge to resize it; reopening within the same run preserves the expanded height. This does not imply every UI setting is persisted across application restarts.

## 10. Troubleshooting

| Symptom | What to check |
|---|---|
| Empty library | Download effects in the client; redetect/select the correct data directory; clear search. |
| Slow first conversion | The embedded backend is being extracted; inspect task/log status and wait. |
| Cannot create cache | Check LOCALAPPDATA permissions, free space and link/reparse-point redirection; do not globally disable protection. |
| Unsupported format | Verify actual container/encoding/schema; private IRS and arbitrary EQ presets are not general audio. |
| Replacement fails | Check permissions, locks and client updates; do not retry concurrent writes. |
| No audible change | Reload the effect; check target, sample rate and type compatibility; restore if necessary. |
| Unexpected volume | Compare original input and peak matching; equal peaks do not imply equal perceived loudness. |
| Missing export | Read its completion path; Downloads may be redirected or a fallback used. |
| Restore fails | Confirm the first original backup still exists; deleted originals cannot be reconstructed. |
| Single EXE creates folders | On-demand backend caching is intentional; those folders need not be distributed in advance. |

When reporting a problem, include version, reproduction steps, redacted logs and synthetic samples, not account details, personal audio or backups.
