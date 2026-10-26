param()
$ErrorActionPreference = 'Stop'
$harness = Join-Path $PSScriptRoot 'run.ps1'
$scratch = Join-Path ([IO.Path]::GetTempPath()) ('deckwright-oracle-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $scratch | Out-Null
$source = Join-Path $scratch 'neutral.txt'
Set-Content -LiteralPath $source -Value 'Neutral invalid presentation fixture'
$hash = (Get-FileHash -LiteralPath $source).Hash
$existing = Join-Path $scratch 'existing'
New-Item -ItemType Directory -Path $existing | Out-Null
$result = & pwsh -NoProfile -ExecutionPolicy Bypass -File $harness -Source $source -ExpectedSha256 $hash -OutputDirectory $existing -TimeoutSeconds 1 2>&1
if ($LASTEXITCODE -eq 0 -or "$result" -notmatch 'Use a fresh output directory') {
    throw 'Existing output directory must be rejected before starting Office'
}
if (Test-Path (Join-Path $existing 'reference.pptx')) { throw 'Existing output directory was modified' }
$mismatch = Join-Path $scratch 'mismatch'
$result = & pwsh -NoProfile -ExecutionPolicy Bypass -File $harness -Source $source -ExpectedSha256 deadbeef -OutputDirectory $mismatch 2>&1
if ($LASTEXITCODE -eq 0 -or "$result" -notmatch 'Source SHA-256 mismatch') { throw 'Mismatched input must fail' }
if (Test-Path $mismatch) { throw 'Mismatched input created output' }
Write-Output 'PASS: existing output rejected; mismatched input rejected without writes'
Write-Output "Neutral test files retained at $scratch"
$staged = Join-Path $scratch 'staged'
New-Item -ItemType Directory -Path $staged | Out-Null
Set-Content -LiteralPath (Join-Path $staged 'reference.pptx') -Value 'Changed staged copy'
$rejected = $false
try { & $harness -Source $source -ExpectedSha256 $hash -OutputDirectory $staged -Worker $true }
catch { if ($_.Exception.Message -match 'Staged copy SHA-256 mismatch') { $rejected = $true } else { throw } }
if (-not $rejected) { throw 'Worker must reject a changed staged copy before COM open' }
Write-Output 'PASS: worker verifies the exact staged bytes before COM open'
