# 格式与预览范围

原版支持 RIFF/WAVE、可解析的 IRS，以及 NCAE 参数 JSON。多格式支线另支持 FLAC、AIFF、AU、CAF、W64、RF64 和 ir.samples.v1 JSON/NPZ。

- 输出 IRS 是 WAV 容器使用 .irs 后缀，不是厂商私有加密算法。
- 采样 JSON/NPZ 与播放器 EQ 预设不是同一种数据；任意预设不能直接还原为 WAV。
- NCAE 音频生成沿用 float32 WAV。float64 或整数数据转换可能量化。
- 不自动重采样；声道、长度和峰值处理由生成设置决定。
- IR 图由所选音效完整脉冲响应计算，不读取导入源文件来画图。
- JSON 图仅显示设置或可确认模型；未知数字 PEQ 类型不映射，不代表整套音效真实频响。
- 预览具有大小、帧数、JSON 深度/节点和解压输出限制。
