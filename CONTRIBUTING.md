# 参与开发

1. 使用 Windows x64、Rust 1.98.1 或兼容更新版本，以及 Windows SDK/MSVC 构建工具。
2. 阅读 docs/BUILD.md，分别构建 apps/stable 与 apps/multiformat。
3. 多格式改动仅放到 multiformat；共用 UI 修复需分别同步并测试。
4. 新增格式应提供格式定义或验证样本，不猜测私有 IRS、PEQ 类型或任意 EQ 预设的声音行为。
5. 测试只使用合成数据；不得自动替换真实网易云文件。
6. 运行格式检查、测试及发布内容检查后提交 PR。

PR 应说明行为变化、影响版本、验证步骤与兼容性限制。不要提交 target、dist、运行时二进制、个人路径、音效备份、替换记录或未获授权的音频样本。
