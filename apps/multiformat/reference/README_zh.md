# IR 双向格式转换器

## 支持范围

音频容器：WAV、IRS（仅可解码音频，输出为 WAV 内容）、FLAC、AIFF、AU、CAF、W64、RF64。
数据容器：JSON、NPZ，限定本工具定义的 `ir.samples.v1` 格式。
所有这些格式都支持输入和输出，因此既能与 WAV 双向转换，也可相互转换。

重要：不支持把任意 JSON/XML 软件预设直接转为 WAV，也不支持把 WAV 自动拟合成特定播放器 EQ 预设。SOFA、私有/加密 IRS、厂商专用动态混响格式不支持。

## 安装与运行

安装 Python 3.10 或更新版本，然后安装依赖：

```bash
pip install numpy soundfile
```

在 ir_converter.py 所在目录运行：

```bash
python ir_converter.py input.irs output.wav
python ir_converter.py input.wav output.irs
python ir_converter.py input.flac output.wav
python ir_converter.py input.wav output.flac
python ir_converter.py input.aiff output.wav
python ir_converter.py input.wav output.aiff
python ir_converter.py input.wav samples.json
python ir_converter.py samples.json restored.wav
python ir_converter.py input.wav samples.npz
python ir_converter.py samples.npz restored.wav
python ir_converter.py input.wav pcm16.wav --subtype PCM_16
```

程序根据输出扩展名确定目标格式。输入、输出不能为同一文件；输出已存在时拒绝覆盖。输出父目录必须已存在。整个 IR 会读入内存。

## API

```python
from ir_converter import convert
result = convert("input.irs", "output.wav")
print(result)
```

转换核心：`read_ir()` 解码到 IR 数据结构，`write_ir()` 编码到目标容器，`convert()` 串联两者。可以通过扩展两个适配函数增加真正有格式定义的文件类型。

## 保留什么

- 采样率、帧数和声道顺序。
- 不归一化、不重采样、不裁剪、不删除起始静音。
- 目标支持时保留原 PCM/FLOAT/DOUBLE 编码；否则 FLAC 默认 PCM_24，其余音频容器默认 DOUBLE。
- `--subtype` 可显式选择编码；目标是否支持由 libsndfile 检查。
- 超出目标整数 PCM 幅度范围时拒绝转换，不暗中削波或缩放。

## 不保证什么

- 不保留全部音频容器元数据，例如 WAV 自定义块、声道掩码和布局标签。因此多声道文件仍需双方采用一致的路径/声道约定。
- 更改采样编码可能产生量化误差；返回值 encoding_changed 会标记编码是否变化，不代表精度损失的完整判定。
- WAV 浮点数据转整数 FLAC 不一定完全无损。
- 输出 IRS 只是 WAV 容器使用 .irs 后缀；不能保证所有名为 IRS 的私有格式兼容。某些播放器还要求特定采样编码和声道数。
- JSON/NPZ 只存采样数据，不是软件效果链预设。

## JSON 示例

```json
{
  "schema": "ir.samples.v1",
  "sample_rate": 48000,
  "source_subtype": "DOUBLE",
  "samples": [[1.0], [0.5], [0.25]]
}
```

samples 为二维数组：[帧][声道]。不匹配此 schema 的 JSON 会被拒绝。

## 验证

已在本地使用 numpy 2.5.3、soundfile 0.14.0 运行 8 个测试方法，全部通过。
其中覆盖 IRS、FLAC、AIFF、AU、CAF、W64、RF64、JSON、NPZ 九种格式在 1/2/4 声道下与 WAV 往返的 27 组组合，并验证采样率和可精确表示的测试样本完全一致。
另验证 float64 JSON 往返、拒绝陌生 JSON、拒绝削波、拒绝覆盖、非法数据、非法编码，以及写出失败清理。
这些测试不代表兼容所有第三方播放器或私有 IRS 文件。
