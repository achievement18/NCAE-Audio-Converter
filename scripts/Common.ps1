Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
function Copy-CleanTree {
    param([Parameter(Mandatory)][string]$Source,[Parameter(Mandatory)][string]$Destination)
    $sourceItem=Get-Item -LiteralPath $Source
    if($sourceItem.Attributes -band [IO.FileAttributes]::ReparsePoint){throw "Refusing linked source: $Source"}
    New-Item -ItemType Directory -Path $Destination -Force | Out-Null
    foreach($item in Get-ChildItem -LiteralPath $Source -Force){
        if($item.Name -in @('__pycache__','.pytest_cache','.mypy_cache','.git','tests')){continue}
        if($item.Attributes -band [IO.FileAttributes]::ReparsePoint){throw "Refusing linked item: $($item.FullName)"}
        $target=Join-Path $Destination $item.Name
        if($item.PSIsContainer){Copy-CleanTree -Source $item.FullName -Destination $target}
        elseif($item.Extension -notin @('.pyc','.pyo')){Copy-Item -LiteralPath $item.FullName -Destination $target -Force}
    }
}
function Invoke-Checked {
    if($args.Count -lt 1){throw 'A command is required'}
    $command=[string]$args[0]
    $arguments=@($args | Select-Object -Skip 1)
    & $command @arguments
    if($LASTEXITCODE -ne 0){throw "$command failed with exit code $LASTEXITCODE"}
}
