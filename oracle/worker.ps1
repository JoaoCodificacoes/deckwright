function Invoke-OracleWorker {
    param([Parameter(Mandatory)][string]$ScriptPath,[Parameter(Mandatory)][hashtable]$Parameters,
          [Parameter(Mandatory)][string]$OutputDirectory,[ValidateRange(1,3600)][int]$TimeoutSeconds,
          [Parameter(Mandatory)][string]$CompletionFile,[string[]]$AllowedStatuses=@('completed'))
    # Only this exact child PowerShell process may be terminated. Never kill a tree or Office.
    $configuration=@{script=$ScriptPath;parameters=$Parameters} | ConvertTo-Json -Depth 10 -Compress
    $encodedConfiguration=[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($configuration))
    $command='$ErrorActionPreference="Stop"; try { $c=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String("'+$encodedConfiguration+'")) | ConvertFrom-Json -AsHashtable; $p=$c.parameters; & $c.script @p; exit 0 } catch { Write-Error $_; exit 1 }'
    $encodedCommand=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
    $process=$null
    $metadata=[ordered]@{status='pending';timeout_seconds=$TimeoutSeconds;child_pid=$null;office_cleanup='worker closes owned presentation; timeout can leave it open';termination_scope='exact owned pwsh child only; no tree or Office termination'}
    try {
        $process=Start-Process -FilePath (Join-Path $PSHOME 'pwsh.exe') -ArgumentList @('-NoLogo','-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-EncodedCommand',$encodedCommand) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $OutputDirectory 'worker.stdout.txt') -RedirectStandardError (Join-Path $OutputDirectory 'worker.stderr.txt')
        $metadata.child_pid=$process.Id
        if (-not $process.WaitForExit($TimeoutSeconds*1000)) {
            $metadata.status='timeout'
            $process.Kill() # Deliberately not Kill(true): PowerPoint/user processes must survive.
            $metadata.child_exited_after_kill=$process.WaitForExit(2000)
            throw 'Oracle child timeout; only owned PowerShell child terminated; Office cleanup unverified'
        }
        $metadata.exit_code=$process.ExitCode
        if ($process.ExitCode -ne 0) { $metadata.status='failed'; throw "Oracle child failed with exit code $($process.ExitCode); inspect worker.stderr.txt" }
        $completionPath=Join-Path $OutputDirectory $CompletionFile
        if (-not (Test-Path -LiteralPath $completionPath)) { $metadata.status='failed'; throw 'Oracle child completion metadata missing despite zero exit' }
        $completion=Get-Content -LiteralPath $completionPath -Raw | ConvertFrom-Json
        if ($completion.status -notin $AllowedStatuses) { $metadata.status='failed'; throw 'Oracle child completion status not accepted despite zero exit' }
        $metadata.completion_file=$CompletionFile
        $metadata.completion_status=$completion.status
        $metadata.status='completed'
    } catch { if ($metadata.status -eq 'pending') { $metadata.status='failed' }; $metadata.error=$_.Exception.Message; throw }
    finally {
        $metadata | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $OutputDirectory 'worker-process.json')
        if ($null -ne $process) { $process.Dispose() }
    }
}
