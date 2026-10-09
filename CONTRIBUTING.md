# 参与开发 / Contributing

## 中文

1. 阅读 README、docs/BUILD.md、docs/FORMATS.md 与 docs/ATTRIBUTION.md。
2. 正式发行目标仅为 `apps/multiformat`；不要把历史稳定版重新放进发行包。
3. 更改转换语义时补充合成数据回归测试；不要在自动测试中替换真实音效。
4. 保留 GPL v3.0 和第三方署名，不猜测私有 IRS、未知 PEQ 类型或任意 EQ 预设的声音行为。
5. 文档功能变更需同步中文与英文。英文文档不代表 UI 已翻译。
6. 运行格式检查、Rust 测试与单 EXE 自测。发布禁止跳过测试。
7. PR 描述行为变化、验证结果和限制；不提交 token、个人路径、缓存、备份或未获授权样本。

## English

1. Read the README, build/format guides and attribution notes.
2. Release only `apps/multiformat`; do not reintroduce historical stable binaries.
3. Add synthetic regressions for conversion changes. Never target real effects in automated tests.
4. Retain GPL v3.0 and third-party notices. Do not guess private formats, PEQ types or preset rendering behavior.
5. Update both Chinese and English documentation for user-facing changes. English docs do not imply an English UI.
6. Run formatting, Rust tests and standalone smoke tests; do not skip tests for a release.
7. Explain behavior, verification and limitations in PRs. Exclude tokens, private paths, caches, backups and unauthorized samples.
