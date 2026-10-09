# 更新记录 / Changelog

## 0.2.1 — 2026-10-09

### 中文
- 正式发行仅保留多格式版，不再包含第二个稳定版 EXE。
- 将多格式运行时、依赖及许可内嵌至单 EXE，首次转换后台释放，后续校验并复用本地缓存。
- 静态链接应用 C/C++ 运行库，增加 EXE 导入表与隔离目录自测。
- 完整补充中英文介绍、操作、格式、构建和来源说明。
- 明确解密源于 AllenHeartcore/audioeffect-ncm，保留 GPL v3.0、第三方许可和对应源码。
- 这次主要改变分发形式；原有音效转换、备份、预览和 UI 操作继续保留。

### English
- Distribute only the multiformat edition, without a second stable executable.
- Embed backend, dependencies and notices in one EXE; extract on demand and validate/reuse the local cache.
- Statically link the application's C/C++ runtime; add PE-import and isolated EXE-only smoke checks.
- Expand Chinese/English introduction, usage, formats, build and attribution documentation.
- Credit AllenHeartcore/audioeffect-ncm for decryption; retain GPL v3.0, third-party notices and corresponding source.
- Existing conversion, backup, preview and UI functionality is retained; the main change is distribution.

## 0.2.0 — 2026-10-09（内部整理 / internal preparation）

两版本便携目录、IR/JSON 预览、源文件转 WAV、自动刷新、亮暗动效和可调日志。曾上传私有草稿，未作为本次公开发行版本保留。

Two-variant portable preparation, IR/JSON previews, source-to-WAV, automatic refresh, light/dark transitions and resizable logs. Uploaded as a private draft; not retained as this public distribution.
