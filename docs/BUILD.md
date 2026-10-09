# 构建与发行

## 环境
- Windows x64
- 已验证 Rust 1.98.1；需要 Windows SDK/MSVC 链接器与资源编译器 rc.exe
- 构建多格式运行时需要 Python 3.12 x64

## 常用命令

```powershell
pwsh -File scripts/Prepare-Backend.ps1
pwsh -File scripts/Build-Release.ps1 -Version 0.2.0
```

Prepare-Backend 将依赖安装到多格式工程的 dist 子目录，不修改系统 site-packages。发布包携带独立运行时。

两个工程分别测试，确保多格式模块不会进入原版。Rust 依赖由各自 Cargo.lock 固定；Python 版本信息会写入发行清单。

CI 在 PR/push 上测试和构建；v* 标签或手动触发可生成发行资产。自动上传默认创建 draft release，正式发布前应检查许可证、内容清单与校验值。

本地有已验证后端时可使用 Prepare-Backend.ps1 的 ExistingBackend 参数，避免再次下载依赖。
