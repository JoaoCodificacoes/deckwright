$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'worker.ps1')
$root=Join-Path ([IO.Path]::GetTempPath()) ('deckwright-worker-test-'+[guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$success=Join-Path $root 'success'
$blocked=Join-Path $root 'blocked'
New-Item -ItemType Directory -Path $success,$blocked | Out-Null
$fixture=Join-Path $root 'neutral child.ps1'
@'
param([string]$OutputDirectory,[int]$DelaySeconds=0,[string]$Literal,[string]$CompletionStatus='completed')
if ($DelaySeconds) { Start-Sleep -Seconds $DelaySeconds }
$Literal | Set-Content (Join-Path $OutputDirectory 'literal.txt')
if ($CompletionStatus -ne 'missing') { @{status=$CompletionStatus} | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory 'completion.json') }
'@ | Set-Content -LiteralPath $fixture
$literal='spaces, quotes " and $() remain data'
Invoke-OracleWorker -ScriptPath $fixture -Parameters @{OutputDirectory=$success;Literal=$literal} -OutputDirectory $success -TimeoutSeconds 10 -CompletionFile 'completion.json'
if ((Get-Content (Join-Path $success 'literal.txt') -Raw).TrimEnd() -ne $literal) { throw 'Argument round trip changed data' }
$clock=[Diagnostics.Stopwatch]::StartNew()
try {
    Invoke-OracleWorker -ScriptPath $fixture -Parameters @{OutputDirectory=$blocked;DelaySeconds=30;Literal='not reached'} -OutputDirectory $blocked -TimeoutSeconds 1 -CompletionFile 'completion.json'
    throw 'Expected timeout'
} catch { if ($_.Exception.Message -notlike 'Oracle child timeout*') { throw } }
$clock.Stop()
$result=Get-Content (Join-Path $blocked 'worker-process.json') -Raw | ConvertFrom-Json
if ($clock.Elapsed.TotalSeconds -gt 8 -or $result.status -ne 'timeout' -or -not $result.child_exited_after_kill) { throw 'Owned child timeout was not bounded' }
if (Test-Path (Join-Path $blocked 'literal.txt')) { throw 'Timed-out child reached continuation' }
"PASS: neutral argument round trip and 1-second child timeout bounded to $([Math]::Round($clock.Elapsed.TotalSeconds,2))s; no COM used"
foreach ($state in @('missing','failed')) {
    $rejected=Join-Path $root $state
    New-Item -ItemType Directory -Path $rejected | Out-Null
    try {
        Invoke-OracleWorker -ScriptPath $fixture -Parameters @{OutputDirectory=$rejected;CompletionStatus=$state} -OutputDirectory $rejected -TimeoutSeconds 10 -CompletionFile 'completion.json'
        throw 'Zero exit without accepted completion was allowed'
    } catch { if ($_.Exception.Message -notlike 'Oracle child completion*') { throw } }
    $rejectedMetadata=Get-Content (Join-Path $rejected 'worker-process.json') -Raw | ConvertFrom-Json
    if ($rejectedMetadata.status -ne 'failed' -or $rejectedMetadata.exit_code -ne 0) { throw 'Failed metadata guard did not reject zero-exit child' }
}
'PASS: zero-exit children with missing or failed completion metadata are rejected'
