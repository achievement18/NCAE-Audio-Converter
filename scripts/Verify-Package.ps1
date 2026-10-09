param([Parameter(Mandatory)][string]$Package)
. "$PSScriptRoot/Common.ps1"
$Package=(Resolve-Path -LiteralPath $Package).Path
foreach($name in @('NCAE音效转换.exe','NCAE音效转换-多格式支线.exe','vcruntime140.dll','runtime/python.exe','converter/bridge.py','converter/ir_converter.py','README.md')){
    if(!(Test-Path -LiteralPath (Join-Path $Package $name))){throw "Missing package file: $name"}
}
$forbidden=Get-ChildItem -LiteralPath $Package -Recurse -Force | Where-Object {
    ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -or $_.Name -in @('replacement_records.json','legacy-backups','.git','__pycache__') -or $_.Extension -in @('.bak','.ncae','.lnk','.pyc','.pyo')
}
if($forbidden){throw "Private or linked/cache content found: $($forbidden.FullName -join ', ')"}
Write-Output 'Package contents verified: no personal effects, backups, records, links or bytecode caches.'
