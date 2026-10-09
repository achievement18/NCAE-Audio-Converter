param([ValidatePattern('^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?$')][string]$Version='0.2.1',[string]$Python='python',[string]$ExistingBackend,[switch]$Offline,[switch]$SkipTests)
. "$PSScriptRoot/Common.ps1"
$Repo=Split-Path -Parent $PSScriptRoot
if($Version -ne (Get-Content "$Repo/VERSION" -Raw).Trim()){throw 'Version and VERSION disagree'}
$multi=Join-Path $Repo 'apps/multiformat'
if($ExistingBackend -or !(Test-Path -LiteralPath "$multi/dist/runtime/python.exe")){
    & "$PSScriptRoot/Prepare-Backend.ps1" -Python $Python -ExistingBackend $ExistingBackend
}
$bundle=Join-Path $Repo '.build/embedded-backend.bin'
Invoke-Checked $Python -I -B -X utf8 "$PSScriptRoot/Pack-Embedded.py" "$multi/dist" $Repo $bundle
$extra=@();if($Offline){$extra+= '--offline'}
$oldFlags=$env:RUSTFLAGS; $oldBundle=$env:NCAE_EMBEDDED_BACKEND; $oldTarget=$env:CARGO_TARGET_DIR
try {
    $env:RUSTFLAGS='-C target-feature=+crt-static'
    $env:NCAE_EMBEDDED_BACKEND=$bundle
    $env:CARGO_TARGET_DIR=Join-Path $Repo '.build/single-exe'
    Invoke-Checked cargo fmt --manifest-path "$multi/Cargo.toml" -- --check
    if(!$SkipTests){Invoke-Checked cargo test --locked --release --target x86_64-pc-windows-msvc --manifest-path "$multi/Cargo.toml" @extra}
    Invoke-Checked cargo build --locked --release --target x86_64-pc-windows-msvc --manifest-path "$multi/Cargo.toml" --bin ncae-gui-multiformat @extra
    $release=Join-Path $Repo 'release'
    New-Item -ItemType Directory -Force -Path $release | Out-Null
    $exe=Join-Path $release "NCAE-Audio-Converter-Multiformat-$Version-windows-x64.exe"
    if(Test-Path -LiteralPath $exe){throw 'Release EXE already exists; choose a new version or explicitly archive the previous artifact'}
    Copy-Item -LiteralPath "$env:CARGO_TARGET_DIR/x86_64-pc-windows-msvc/release/ncae-gui-multiformat.exe" -Destination $exe
    Invoke-Checked $Python -I -B "$PSScriptRoot/Verify-SingleExe.py" $exe
    if(!$SkipTests){& "$PSScriptRoot/Test-SingleExe.ps1" -Executable $exe}
    $runtime=& "$multi/dist/runtime/python.exe" -I -B -X utf8 "$PSScriptRoot/Runtime-Info.py" "$multi/dist/converter" | ConvertFrom-Json
    if($LASTEXITCODE -ne 0){throw 'Runtime metadata failed'}
    [ordered]@{version=$Version;variant='multiformat';distribution='single-exe';platform='windows-x64';rust=(& rustc --version);runtime=$runtime;personal_data_included=$false;tests_skipped=[bool]$SkipTests} | ConvertTo-Json -Depth 8 | Set-Content "$exe.build-info.json" -Encoding utf8
    (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()+'  '+[IO.Path]::GetFileName($exe) | Set-Content "$exe.sha256" -Encoding utf8
    Write-Output "Release: $exe"
} finally {
    $env:RUSTFLAGS=$oldFlags; $env:NCAE_EMBEDDED_BACKEND=$oldBundle; $env:CARGO_TARGET_DIR=$oldTarget
}
