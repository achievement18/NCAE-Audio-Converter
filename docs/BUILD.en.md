# Build and release: multiformat single EXE

[简体中文](BUILD.md) · [Home](../README.en.md)

## Requirements

- Windows x64, PowerShell 7 and Git.
- Rust 1.98.1 (validated version), with `x86_64-pc-windows-msvc`.
- MSVC C++ Build Tools and Windows SDK, including linker and `rc.exe`.
- Python 3.12 x64 to prepare the private embedded runtime; end users do not install it.
- Initial dependency acquisition needs network access. Offline builds need prepared Cargo caches and backend.

## Build

From the repository root:

```powershell
pwsh -File scripts/Build-Release.ps1 -Version 0.2.1 -Python "C:\path\to\python.exe"
```

Only `apps/multiformat` is released: prepare backend → pack embedded resources → check formatting/tests → build GUI with static CRT → inspect PE imports → isolated EXE-only smoke test → generate SHA-256/build metadata.

Outputs:

```text
release/NCAE-Audio-Converter-Multiformat-0.2.1-windows-x64.exe
release/NCAE-Audio-Converter-Multiformat-0.2.1-windows-x64.exe.sha256
release/NCAE-Audio-Converter-Multiformat-0.2.1-windows-x64.exe.build-info.json
```

No stable-release executable or adjacent Python folder is distributed. Existing output files are not overwritten; explicitly archive an old artifact or use a new version.

## Backend and offline builds

```powershell
pwsh -File scripts/Prepare-Backend.ps1 -Python "C:\path\to\python.exe"
pwsh -File scripts/Build-Release.ps1 -Version 0.2.1 -Offline -Python "C:\path\to\python.exe"
```

Use `-ExistingBackend "C:\path\to\backend"` for a verified directory containing `runtime` and `converter`. Dependencies are installed locally, not into global site-packages. `-Offline` controls Cargo only; pip may still need network if the backend is unprepared.

`-SkipTests` is for local troubleshooting, not validated releases. Python dependency ranges are in `backend/requirements-runtime.txt`; actual versions are recorded in build-info. Bit-for-bit reproducibility across dates is not claimed.

## Single-file implementation

- `scripts/Pack-Embedded.py` creates a bounded, path-restricted zlib archive containing backend, licenses and documentation, never personal data.
- `build.rs` reads `NCAE_EMBEDDED_BACKEND` for `include_bytes!` embedding.
- `embedded_runtime.rs` extracts on first backend use in a worker. First use per process compares every file byte-for-byte and rejects unexpected files/linked paths. This is not complete isolation from a local attacker with equal write access.
- Explicit Windows target plus `-C target-feature=+crt-static` removes external application VC-runtime requirements; backend DLLs are embedded.
- The cache-directory content identifier distinguishes versions and is not a signature; complete content comparison validates the cache.

Ordinary development builds without the embedding variable still look for adjacent or `dist` runtime/converter directories. Those builds are not the official single-EXE mode.

## Tests

```powershell
cargo fmt --manifest-path apps/multiformat/Cargo.toml -- --check
cargo test --locked --manifest-path apps/multiformat/Cargo.toml
pwsh -File scripts/Test-SingleExe.ps1 -Executable "release\NCAE-Audio-Converter-Multiformat-0.2.1-windows-x64.exe"
```

Format tests need a prepared or embedded backend. Smoke tests copy the EXE into an isolated path containing spaces/non-ASCII characters, redirect test LOCALAPPDATA, and validate conversion/cache handling without modifying real Cloud Music effects. `--self-test <absolute-report-path>` is a noninteractive validation entry point, not an interactive GUI session.

## Source CLI (not another distributed executable)

```powershell
cargo run --release --manifest-path apps/multiformat/Cargo.toml --bin ncae-tool-multiformat -- info "effect.ncae"
cargo run --release --manifest-path apps/multiformat/Cargo.toml --bin ncae-tool-multiformat -- roundtrip "effect.ncae"
cargo run --release --manifest-path apps/multiformat/Cargo.toml --bin ncae-tool-multiformat -- decrypt "effect.ncae" "output"
```

The source CLI also has encrypt / replace / restore with separate arguments and historical backup-record semantics. GUI collision protection and locations are not universal CLI guarantees. Read its no-argument help and test synthetic files first.

General backend conversion:

```powershell
& .\apps\multiformat\dist\runtime\python.exe -I -B -X utf8 .\apps\multiformat\dist\converter\bridge.py "input.flac" "output.wav" --subtype FLOAT
```

These are developer commands; the GUI does not expose an arbitrary output-format selector.

## CI and publication

Pushes, PRs and manual runs build the multiformat target. `v*` tags must match VERSION and create draft releases without replacing existing releases. `git archive` provides corresponding source for that commit. Publish only the multiformat EXE, checksum, build info and source—not the old dual-variant ZIP.

Historical `apps/stable` is excluded from release builds. Any fixes backported there require independent tests; it must not be described as updated by this release.
