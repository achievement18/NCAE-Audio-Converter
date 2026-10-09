# Sources, port history and acknowledgements

[简体中文](ATTRIBUTION.md) · [Home](../README.en.md)

## 1. NCAE decryption upstream: AllenHeartcore/audioeffect-ncm

- Repository: https://github.com/AllenHeartcore/audioeffect-ncm
- Description: Reverse engineered audio effect module for NetEase Cloud Music.
- Relevant implementation: `read_encfile` and `NCAEDecryptor` in `ncae/decrypt.py`.
- Research checkout revision: `b709cee76c6b4b0569a31c4345c3069aab831151`.
- [Decryption source at that revision](https://github.com/AllenHeartcore/audioeffect-ncm/blob/b709cee76c6b4b0569a31c4345c3069aab831151/ncae/decrypt.py).
- License: GNU GPL v3.0, retained in [the upstream license copy](../licenses/audioeffect-ncm-GPL-3.0.txt).

Provenance evidence: early local validation scripts imported `read_encfile` and `NCAEDecryptor` directly from upstream `ncae.decrypt`. A subsequent `ncae_core.py` organized the container, key and stream transformations into a Python core; the original Rust implementation notes explicitly described an equivalent Rust port. Those personal validation scripts contain real file paths and are not distributed; this document records their provenance rather than publishing private samples.

Inherited concepts include NCAE header/key reconstruction, permutation-table generation, XOR keystream transformation and raw Deflate decompression. **This project must not claim independent discovery of that reverse-engineering work.**

## 2. Subsequent modifications

The version prepared on October 9, 2026 includes:

- A Rust port of the Python core with integer/bounds handling, avoiding NumPy uint8 arithmetic issues.
- Little-endian payload lengths based on locally verified files, plus truncation, length and bounded-decompression checks; not an unchanged line-for-line copy.
- Reverse raw Deflate/XOR packaging using a template's keys/header fields and updated payload type.
- WAV processing, format adaptation, backup/restore, native GUI, background work and previews.
- An isolated multiformat backend, cache validation and single-EXE release tooling.

These additions do not imply endorsement by the upstream author or upstream support for reverse packaging and all client versions.

## 3. Preview references

IR/JSON previews were based on owner-supplied `ir_viewer_core.js`, `json_effect_preview_core.js` and `json_effect_viewer_core.js`. Original references/notices remain under `apps/multiformat/reference/previews`. The native Rust implementation renders desktop previews without a browser or Node runtime.

No verifiable public repository URL accompanied those reference files, so no author profile or upstream URL is fabricated. Their MIT notices are retained. Standard PEQ model comments reference the W3C Audio EQ Cookbook; private numeric types are not guessed.

## 4. Conversion reference

`backend/ir_converter.py` comes from the owner's supplied conversion reference project; `backend/bridge.py` adapts it for isolated execution. Supplied notes remain under `apps/multiformat/reference`. No separate verified public upstream URL was provided. These capabilities must not be attributed to audioeffect-ncm.

NumPy, SoundFile, libsndfile and CFFI provide sample-data and codec functionality under their respective licenses.

## 5. Licensing and release scope

The repository is distributed under GPL v3.0 to retain the decryption port's upstream licensing. Existing third-party notices are not removed or relicensed. Corresponding source, build scripts, lockfiles and references accompany the source archive. See [third-party notices](../THIRD_PARTY_NOTICES.md).

Only the multiformat application is built for release. Historical stable source is not a second distributed application. No upstream effect-resource collection, personal audio/test files or Cloud Music client files are included.
