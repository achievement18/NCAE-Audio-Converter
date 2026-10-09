# 来源、移植关系与致谢

[English](ATTRIBUTION.en.md) · [首页](../README.md)

## 1. NCAE 解密上游：AllenHeartcore/audioeffect-ncm

- 项目：https://github.com/AllenHeartcore/audioeffect-ncm
- 原项目描述：Reverse engineered audio effect module for NetEase Cloud Music。
- 核心参考：`ncae/decrypt.py` 中的 `read_encfile`、`NCAEDecryptor`。
- 本地研究副本对应提交：`b709cee76c6b4b0569a31c4345c3069aab831151`。
- [固定提交的解密源码](https://github.com/AllenHeartcore/audioeffect-ncm/blob/b709cee76c6b4b0569a31c4345c3069aab831151/ncae/decrypt.py)。
- 上游许可：GNU GPL v3.0；原文保留于 [许可证副本](../licenses/audioeffect-ncm-GPL-3.0.txt)。

来源证据：早期本地验证脚本直接从上游 `ncae.decrypt` 导入 `read_encfile` 和 `NCAEDecryptor`；随后 `ncae_core.py` 将容器、密钥与流变换整理为 Python 核心，早期 Rust 说明明确该核心的 Rust 对等实现。个人验证脚本包含真实文件路径，因此不随公开仓库发布；这里记录来源关系，而不是公开那些样本。

解密沿用的思路包括：NCAE 头与密钥字节还原、置换表构建、异或密钥流变换、raw Deflate 解压。**不能将这些研究描述为本项目原创发现。**

## 2. 本项目的后续修改

2026-10-09 整理的版本包括：

- Python 核心到 Rust 的移植，使用整数与边界检查，避免 NumPy uint8 运算问题。
- 依据本地已验证文件采用小端载荷长度，补充截断、长度与解压限制；不是逐行不变的上游复制品。
- 反向 raw Deflate / 异或封装，沿用模板密钥、头字段并更新载荷类型。
- WAV 处理、输入格式适配、备份恢复、原生 GUI、异步任务与预览。
- 多格式独立后端、缓存验证与单 EXE 构建发行。

这些扩展不意味着上游作者为本项目背书，也不意味着上游承诺支持反向封装或所有客户端版本。

## 3. 预览算法参考

IR 和 JSON 预览参考了项目所有者提供的 `ir_viewer_core.js`、`json_effect_preview_core.js`、`json_effect_viewer_core.js`，原参考文件与其中声明保存在 `apps/multiformat/reference/previews`。Rust 原生实现用于桌面绘制，不要求浏览器或 Node 运行这些脚本。

这些文件没有随交付资料附带可核验的公开仓库地址，因此不虚构作者主页或仓库。保留其 MIT 声明；PEQ 标准模型注释提及 W3C Audio EQ Cookbook，私有数字类型不自行猜测。

## 4. 多格式转换参考

`backend/ir_converter.py` 源于项目所有者提供的同名转换参考项目；`backend/bridge.py` 是隔离调用适配层。原交付说明保存在 `apps/multiformat/reference`。未提供可核验的独立公开上游地址，不将它误归为 audioeffect-ncm 的功能。

NumPy、SoundFile、libsndfile 与 CFFI 承担采样数据与编解码工作，各自保留适用许可证。

## 5. 许可与发布范围

为保留解密移植链的上游许可，本仓库整体以 GPL v3.0 分发；已有第三方声明不被移除或改写。对应源码、构建脚本、锁文件与参考文件随源码归档提供。依赖清单和许可位置见 [第三方声明](../THIRD_PARTY_NOTICES.md)。

仅多格式版生成可执行发行文件；历史稳定版源码不是另一个已发布的软件包。发行包不包含上游音效资源库、个人音频、个人测试文件或网易云客户端文件。
