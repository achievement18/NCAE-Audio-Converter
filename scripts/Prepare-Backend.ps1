param([string]$Python='python',[string]$ExistingBackend)
. "$PSScriptRoot/Common.ps1"
$Repo=Split-Path -Parent $PSScriptRoot
$Destination=Join-Path $Repo 'apps/multiformat/dist'
New-Item -ItemType Directory -Path $Destination -Force | Out-Null
if($ExistingBackend){
    $ExistingBackend=(Resolve-Path -LiteralPath $ExistingBackend).Path
    Copy-CleanTree -Source (Join-Path $ExistingBackend 'runtime') -Destination (Join-Path $Destination 'runtime')
    Copy-CleanTree -Source (Join-Path $ExistingBackend 'converter') -Destination (Join-Path $Destination 'converter')
}else{
    $info=& $Python -c 'import sys,struct,json; print(json.dumps(dict(root=sys.base_prefix,version=list(sys.version_info[:2]),bits=struct.calcsize("P")*8)))' | ConvertFrom-Json
    if($LASTEXITCODE -ne 0 -or $info.version[0] -ne 3 -or $info.version[1] -ne 12 -or $info.bits -ne 64){throw 'Python 3.12 x64 is required to prepare this runtime'}
    $runtime=Join-Path $Destination 'runtime'
    New-Item -ItemType Directory -Path "$runtime/Lib","$Destination/converter/deps" -Force | Out-Null
    foreach($name in @('python.exe','python3.dll','python312.dll','vcruntime140.dll','vcruntime140_1.dll','LICENSE.txt')){
        $source=Join-Path $info.root $name
        if(Test-Path -LiteralPath $source){Copy-Item -LiteralPath $source -Destination (Join-Path $runtime $name) -Force}
    }
    Copy-CleanTree -Source (Join-Path $info.root 'DLLs') -Destination "$runtime/DLLs"
    foreach($item in Get-ChildItem -LiteralPath (Join-Path $info.root 'Lib') -Force){
        if($item.Name -in @('site-packages','__pycache__','test','tests','tkinter','idlelib','turtledemo','ensurepip')){continue}
        if($item.PSIsContainer){Copy-CleanTree -Source $item.FullName -Destination "$runtime/Lib/$($item.Name)"}
        elseif($item.Extension -notin @('.pyc','.pyo')){Copy-Item -LiteralPath $item.FullName -Destination "$runtime/Lib/$($item.Name)" -Force}
    }
    Invoke-Checked $Python -m pip install --disable-pip-version-check --no-compile --only-binary=:all: --target "$Destination/converter/deps" -r "$Repo/backend/requirements-runtime.txt"
}
foreach($name in @('bridge.py','ir_converter.py')){Copy-Item -LiteralPath "$Repo/backend/$name" -Destination "$Destination/converter/$name" -Force}
Invoke-Checked "$Destination/runtime/python.exe" -I -B -X utf8 "$Repo/scripts/Runtime-Info.py" "$Destination/converter"
