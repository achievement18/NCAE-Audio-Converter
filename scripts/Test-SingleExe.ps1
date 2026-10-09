param([Parameter(Mandatory)][string]$Executable)
. "$PSScriptRoot/Common.ps1"
$Executable=(Resolve-Path -LiteralPath $Executable).Path
$repo=Split-Path -Parent $PSScriptRoot
$testRoot=Join-Path $repo ('.build/smoke-'+[guid]::NewGuid().ToString('N'))
$launchDir=Join-Path $testRoot 'standalone space 中文'
New-Item -ItemType Directory -Path $launchDir -Force | Out-Null
$copy=Join-Path $launchDir 'NCAE.exe'
Copy-Item -LiteralPath $Executable -Destination $copy
$previous=$env:LOCALAPPDATA
try {
    $env:LOCALAPPDATA=Join-Path $testRoot 'localappdata'
    $runtimePaths=@()
    foreach($iteration in 1..3){
        $report=Join-Path $testRoot "self-test-$iteration.json"
        $process=Start-Process -FilePath $copy -ArgumentList @('--self-test', ('"'+$report+'"')) -WorkingDirectory $launchDir -WindowStyle Hidden -PassThru
        if(!$process.WaitForExit(180000)){throw 'Standalone smoke test timed out'}
        if($process.ExitCode -ne 0){throw "Standalone smoke test failed: $(Get-Content $report -Raw -ErrorAction SilentlyContinue)"}
        $result=Get-Content -LiteralPath $report -Raw | ConvertFrom-Json
        if(!$result.ok){throw 'Standalone smoke test did not succeed'}
        $runtimePaths += $result.runtime
        if($iteration -eq 2){
            if($runtimePaths[0] -ne $runtimePaths[1]){throw 'Cache was not reused'}
            # A harmless unexpected module must cause a new clean cache generation.
            Set-Content -LiteralPath (Join-Path $result.runtime 'converter/unexpected_smoke_module.py') -Value '# synthetic cache contamination test'
        }
    }
    if($runtimePaths[1] -eq $runtimePaths[2]){throw 'Altered cache was reused'}
    if(@(Get-ChildItem -LiteralPath $launchDir -File).Count -ne 1){throw 'Standalone launcher directory is not EXE-only'}
    Write-Output 'Standalone smoke passed: EXE-only directory, fresh extraction, cache reuse, altered-cache recovery, WAV conversion.'
} finally { $env:LOCALAPPDATA=$previous }
