# 安全与隐私

- 不要在公开 issue 中提交个人音效、备份、替换记录、账号信息或未脱敏路径。
- 可复现问题请优先使用合成样本。
- 预览是只读操作；真实写入需要显式生成/替换操作。关闭备份存在不可恢复风险。
- 私有格式、未知 PEQ 类型不得靠猜测执行。

发现漏洞时，如仓库已启用 Private vulnerability reporting，可从 Security 页面私密报告；否则请先通过维护者提供的私密渠道联系。不要公开发布可用于破坏用户文件的利用样本。当前没有承诺特定响应时限。

## English

Do not post account data, personal effects, backups, replacement records or unredacted paths in public issues. Use synthetic reproductions. Previews are read-only; replacement writes real files, and disabling backup can make recovery impossible. Embedded-runtime cache validation is defense-in-depth, not isolation from a local user with equivalent privileges.

Use GitHub private vulnerability reporting if enabled, otherwise contact the maintainer through an explicitly provided private channel. Do not publish destructive exploit samples. No guaranteed response time is promised.
