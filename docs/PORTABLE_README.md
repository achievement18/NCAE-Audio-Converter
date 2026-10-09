# NCAE 音效转换 · 便携版

直接双击本目录的 **NCAE音效转换.exe**。
需要更多格式时，运行 **NCAE音效转换-多格式支线.exe**。

整个文件夹可一起移动。不要只复制一个 EXE，也不要移除 runtime、converter 或运行库 DLL。两种版本保持独立，不要同时替换同一个音效。

## 文件与操作
- 默认备份后替换，只写目标和首次 .bak，不额外导出副本。
- 更多操作中的导出保存到系统 Downloads。
- 源文件转 WAV 沿用源文件名；解密导出按音效库名称；重名自动编号。
- 取消直接替换后，仅生成的文件存放在本目录 generated。
- 预览页签只读，不改变模块开关；JSON 曲线不等同于整套音效实测响应。

## 本机数据
公开发行包不包含音效、备份或替换记录。如果此文件夹另有 legacy-backups 和 replacement_records.json，它们是本机迁移数据，不要上传到 GitHub。

命令行工具位于 tools。许可及依赖声明位于 licenses，以及 runtime/converter 的组件许可文件中。
