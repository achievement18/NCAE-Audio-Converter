# 第三方来源与许可 / Third-party sources and licenses

## NCAE 解密 / NCAE decryption

**AllenHeartcore/audioeffect-ncm** — https://github.com/AllenHeartcore/audioeffect-ncm

GNU GPL v3.0. The decryption core is a port/adaptation of work originating in this project, not an independent discovery. / 解密核心来自该项目的研究与后续移植，不是本项目独立发现。

Research revision: `b709cee76c6b4b0569a31c4345c3069aab831151`. See `licenses/audioeffect-ncm-GPL-3.0.txt`, root `LICENSE`, and `docs/ATTRIBUTION.md` / `docs/ATTRIBUTION.en.md` for modifications and provenance.

## 预览 / Preview references

Owner-supplied IR/JSON JavaScript reference cores and their existing notices are retained under `apps/multiformat/reference/previews`. MIT notices remain intact. The distribution embeds `licenses/preview-cores.txt`. No unverified public upstream URL is asserted.

保留原交付的 MIT 声明，不将其版权冒认为本项目作者所有；原生适配与外部参考的范围见来源说明。

## 多格式后端 / Conversion backend

`backend/ir_converter.py` is the owner-supplied reference converter; `bridge.py` is the isolated invocation adapter. Supplied reference notes remain under `apps/multiformat/reference`. Separate public provenance was not provided; the NCAE decryption upstream is not credited for this independent conversion component.

## Rust 依赖 / Rust dependencies

Versions and dependency sources are recorded in Cargo.lock. License records/texts collected from the locally resolved crates are retained under `licenses/rust`. Crates keep their original notices; GPL coverage of this application does not replace each dependency's own license.

## 内嵌运行时 / Embedded runtime

After the backend is extracted, inspect these paths inside its cache generation:

- Python: `runtime/LICENSE.txt`.
- NumPy / SoundFile / CFFI / pycparser: license and metadata files inside `converter/deps`, including `.dist-info` directories.
- libsndfile: `converter/deps/_soundfile_data/COPYING` and related bundled notices. Its separate license still applies.
- Microsoft Visual C++ runtime DLLs used by embedded Python: Microsoft components subject to their applicable redistribution terms, not relicensed by this project.

单 EXE 不删除这些许可：它们和相应依赖一起压缩内嵌，运行时释放后可读取。根许可证、来源说明与开发文档也随可执行文件嵌入。系统 DLL、字体与驱动不打包。

The single-EXE format preserves rather than removes component licenses. Backend files can be inspected after extraction; corresponding application source/build scripts are provided separately with the release. Windows system DLLs, fonts and drivers are not bundled.
