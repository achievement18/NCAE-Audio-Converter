param([Parameter(Mandatory)][string]$Package,[string]$Python='python')
. "$PSScriptRoot/Common.ps1"
if(!(Test-Path -LiteralPath $Package -PathType Leaf) -or [IO.Path]::GetExtension($Package) -ne '.exe'){throw 'This release accepts one multiformat EXE, not the old two-variant directory package'}
Invoke-Checked $Python -I -B "$PSScriptRoot/Verify-SingleExe.py" (Resolve-Path -LiteralPath $Package).Path
