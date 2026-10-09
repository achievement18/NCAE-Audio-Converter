param([ValidatePattern('^\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?$')][string]$Version='0.2.0',[string]$Python='python',[string]$ExistingBackend,[switch]$Offline,[switch]$SkipTests)
. "$PSScriptRoot/Common.ps1"
$Repo=Split-Path -Parent $PSScriptRoot
$stable=Join-Path $Repo 'apps/stable'; $multi=Join-Path $Repo 'apps/multiformat'
if($ExistingBackend -or !(Test-Path -LiteralPath "$multi/dist/runtime/python.exe")){
    & "$PSScriptRoot/Prepare-Backend.ps1" -Python $Python -ExistingBackend $ExistingBackend
}
$extra=@();if($Offline){$extra+= '--offline'}
foreach($project in @($stable,$multi)){
    Invoke-Checked cargo fmt --manifest-path "$project/Cargo.toml" -- --check
    if(!$SkipTests){Invoke-Checked cargo test --locked --release --manifest-path "$project/Cargo.toml" @extra}
    Invoke-Checked cargo build --locked --release --bins --manifest-path "$project/Cargo.toml" @extra
}
$package=Join-Path $Repo "release/NCAE-Audio-Converter-$Version-windows-x64"
if(Test-Path -LiteralPath $package){throw 'Release directory already exists; use a new version or move the previous build explicitly'}
New-Item -ItemType Directory -Path "$package/tools","$package/licenses" -Force | Out-Null
Copy-Item -LiteralPath "$stable/target/release/ncae-gui.exe" -Destination "$package/NCAE音效转换.exe"
Copy-Item -LiteralPath "$multi/target/release/ncae-gui-multiformat.exe" -Destination "$package/NCAE音效转换-多格式支线.exe"
Copy-Item -LiteralPath "$stable/target/release/ncae-tool.exe" -Destination "$package/tools/NCAE-CLI.exe"
Copy-Item -LiteralPath "$multi/target/release/ncae-tool-multiformat.exe" -Destination "$package/tools/NCAE-多格式CLI.exe"
Copy-CleanTree -Source "$multi/dist/runtime" -Destination "$package/runtime"
Copy-CleanTree -Source "$multi/dist/converter" -Destination "$package/converter"
foreach($name in @('vcruntime140.dll','vcruntime140_1.dll')){
    if(Test-Path -LiteralPath "$package/runtime/$name"){
        Copy-Item -LiteralPath "$package/runtime/$name" -Destination "$package/$name"
        Copy-Item -LiteralPath "$package/runtime/$name" -Destination "$package/tools/$name"
    }
}
foreach($name in @('THIRD_PARTY_NOTICES.md','LICENSE_PENDING.md')){Copy-Item -LiteralPath "$Repo/$name" -Destination "$package/licenses/$name"}
Copy-Item -LiteralPath "$stable/reference/previews/LICENSES.txt" -Destination "$package/licenses/preview-cores.txt"
Copy-Item -LiteralPath "$Repo/docs/PORTABLE_README.md" -Destination "$package/README.md"
$rust=(& rustc --version)
$runtime=& "$package/runtime/python.exe" -I -B -X utf8 "$Repo/scripts/Runtime-Info.py" "$package/converter" | ConvertFrom-Json
if($LASTEXITCODE -ne 0){throw 'Packaged runtime validation failed'}
$info=[ordered]@{version=$Version;platform='windows-x64';rust=$rust;runtime=$runtime;build_time_utc=[DateTime]::UtcNow.ToString('o');personal_data_included=$false}
$info | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath "$package/build-info.json" -Encoding utf8
& "$PSScriptRoot/Verify-Package.ps1" -Package $package
$entries=Get-ChildItem -LiteralPath $package -Recurse -File | ForEach-Object {
    $relative=[IO.Path]::GetRelativePath($package,$_.FullName).Replace('\','/')
    "$((Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant())  $relative"
}
$entries | Sort-Object | Set-Content -LiteralPath "$package/SHA256SUMS.txt" -Encoding utf8
$zip="$package.zip"
Compress-Archive -LiteralPath $package -DestinationPath $zip -CompressionLevel Optimal
(Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()+"  "+[IO.Path]::GetFileName($zip) | Set-Content -LiteralPath "$zip.sha256" -Encoding utf8
Write-Output "Release: $zip"
