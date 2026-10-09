# 单 EXE 发行说明 / Single-EXE distribution

正式发行只提供多格式版：下载一个 EXE 即可运行，无需额外安装 Python 或携带运行时文件夹。

Only the multiformat edition is released. Download one EXE; no separate Python installation or accompanying runtime folder is required.

转换后端按需释放到 `%LOCALAPPDATA%\NCAEAudioConverter\runtime`，不是完全不落地执行。关闭程序后可删除缓存，下次自动生成。不要删除正在使用的缓存。

The backend extracts on demand to that local cache, so execution is not entirely in-memory. Remove it only while the application is closed; it is regenerated when needed.

默认生成会备份并替换目标；首次 `.bak` 保存在原音效旁。正常替换不额外导出。显式导出默认到 Downloads；仅生成到 EXE 旁的 generated。预览只读，不会切换真实模块。

Generate defaults to backup/replacement, retaining the first adjacent `.bak`. Normal replacement makes no extra export. Explicit exports default to Downloads; generate-only output goes beside the EXE under generated. Previews are read-only.

完整中英文文档见仓库 README / README.en.md 及 docs，解密来源为 AllenHeartcore/audioeffect-ncm，按 GPL v3.0 保留许可。对应源码随版本提供。

See the repository README / README.en.md and docs for full bilingual guidance. Decryption originates from AllenHeartcore/audioeffect-ncm; GPL v3.0 and third-party notices are retained. Corresponding source accompanies each release.
