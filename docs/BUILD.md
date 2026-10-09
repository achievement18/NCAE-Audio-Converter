# 构建与发行：多格式单 EXE

[English](BUILD.en.md) · [首页](../README.md)

## 环境

- Windows x64、PowerShell 7、Git。
- Rust 1.98.1（本次验证版本）与 `x86_64-pc-windows-msvc` target。
- MSVC C++ Build Tools 和 Windows SDK，包含链接器与 `rc.exe`。
- Python 3.12 x64，用于准备嵌入的独立运行时；最终用户无需安装。
- 首次构建需获取 Cargo 和 Python 依赖；离线构建要求先准备缓存和后端。

## 一条命令构建

在仓库根目录执行：

```powershell
pwsh -File scripts/Build-Release.ps1 -Version 0.2.1 -Python "C:\path\to\python.exe"
```

脚本仅构建 `apps/multiformat`：准备后端 → 收集嵌入文件 → Rust 格式检查与测试 → 静态 CRT 编译 GUI → PE 导入检查 → 隔离单 EXE 自测 → 生成 SHA-256 与构建信息。

主要输出：

```text
release/NCAE-Audio-Converter-Multiformat-0.2.1-windows-x64.exe
release/NCAE-Audio-Converter-Multiformat-0.2.1-windows-x64.exe.sha256
release/NCAE-Audio-Converter-Multiformat-0.2.1-windows-x64.exe.build-info.json
```

不会生成稳定版发行 EXE，也不将 Python 文件夹放在发行 EXE 旁。相同输出已存在时拒绝覆盖，请明确归档旧产物或使用新版本。

## 后端与离线模式

```powershell
pwsh -File scripts/Prepare-Backend.ps1 -Python "C:\path\to\python.exe"
pwsh -File scripts/Build-Release.ps1 -Version 0.2.1 -Offline -Python "C:\path\to\python.exe"
```

已有验证过的后端目录（包含 `runtime` 和 `converter`）可以传 `-ExistingBackend "C:\path\to\backend"`。依赖安装在项目局部目录，不修改全局 site-packages。`-Offline` 只控制 Cargo；后端尚未准备时不能保证 pip 不联网。

`-SkipTests` 只供本地排查，发布验证不得使用。Python 依赖范围见 `backend/requirements-runtime.txt`；实际版本记录在 build-info，不宣称跨时间逐字节可复现。

## 单文件实现

- `scripts/Pack-Embedded.py` 生成有界、路径受限的 zlib 自定义归档，包含后端、组件许可与文档，不包含用户数据。
- `build.rs` 读取 `NCAE_EMBEDDED_BACKEND`，将归档放入编译输出供 `include_bytes!` 内嵌。
- `embedded_runtime.rs` 在后台首次需要转换时释放；每个进程首次使用逐文件比对字节，并拒绝额外文件与链接路径。验证不是对本机已拥有同等写权限攻击者的完整隔离。
- 使用 `-C target-feature=+crt-static` 与显式 Windows target，应用无需外置 VC 运行库；后端所需 DLL 已在内嵌目录中。
- 缓存路径中的内容标识仅用于区分版本，不是签名；完整缓存比对负责检查内容一致性。

没有设置嵌入变量的普通开发构建仍寻找相邻 `runtime` / `converter` 或 `dist` 后端，这不是正式单 EXE 模式。

## 开发测试

```powershell
cargo fmt --manifest-path apps/multiformat/Cargo.toml -- --check
cargo test --locked --manifest-path apps/multiformat/Cargo.toml
pwsh -File scripts/Test-SingleExe.ps1 -Executable "release\NCAE-Audio-Converter-Multiformat-0.2.1-windows-x64.exe"
```

格式测试依赖准备好的后端或编译时内嵌运行时。自测复制 EXE 到独立含空格/中文的目录，重定向测试 LOCALAPPDATA，验证转换与缓存；不替换真实网易云文件。GUI 程序支持 `--self-test <报告绝对路径>` 作为构建验证入口，该入口不打开交互窗口。

## 源码 CLI（开发能力，不额外发行 EXE）

```powershell
cargo run --release --manifest-path apps/multiformat/Cargo.toml --bin ncae-tool-multiformat -- info "effect.ncae"
cargo run --release --manifest-path apps/multiformat/Cargo.toml --bin ncae-tool-multiformat -- roundtrip "effect.ncae"
cargo run --release --manifest-path apps/multiformat/Cargo.toml --bin ncae-tool-multiformat -- decrypt "effect.ncae" "output"
```

源码 CLI 另有 encrypt / replace / restore；它沿用独立的参数与历史备份记录逻辑，不能把 GUI 的防覆盖和保存规则当作 CLI 保证。先运行无参数帮助并使用合成样本。

通用音频格式转换：

```powershell
& .\apps\multiformat\dist\runtime\python.exe -I -B -X utf8 .\apps\multiformat\dist\converter\bridge.py "input.flac" "output.wav" --subtype FLOAT
```

这里只列开发命令，单 EXE GUI 不提供通用任意目标格式选择器。

## CI 与发布

push / PR / 手动触发执行多格式构建；`v*` 标签校验 VERSION 后创建 Release 草稿，不自动覆盖现有版本。CI 同时通过 `git archive` 生成该提交的对应源码 ZIP。公开发布时只附多格式 EXE、校验、构建信息和源码，不上传旧两版本 ZIP。

历史 `apps/stable` 不参与发行流程。主题和安全改动若用于该历史工程，应单独测试，不能声称它已随本次发行更新。
