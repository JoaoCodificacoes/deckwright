param(
    [Parameter(Mandatory)][string]$Source,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$ExpectedSha256 = 'ee8cce03cf0201150fdd6f9c75f5d551538ed90d6ffae1c417e82b939e027e2b',
    [ValidateRange(1, 3600)][int]$TimeoutSeconds = 120,
    [bool]$Worker = $false
)
$ErrorActionPreference = 'Stop'
$sourcePath = (Resolve-Path -LiteralPath $Source).Path
$sourceHash = (Get-FileHash -LiteralPath $sourcePath -Algorithm SHA256).Hash.ToLowerInvariant()
if ($sourceHash -ne $ExpectedSha256.ToLowerInvariant()) { throw 'Source SHA-256 mismatch' }
$outputPath = [IO.Path]::GetFullPath($OutputDirectory)
$copyPath = Join-Path $outputPath 'reference.pptx'
if ($sourcePath -eq $copyPath) { throw 'Output must differ from immutable source' }
if (-not $Worker) {
    if (Test-Path -LiteralPath $outputPath) { throw 'Use a fresh output directory' }
    New-Item -ItemType Directory -Path $outputPath | Out-Null
    Copy-Item -LiteralPath $sourcePath -Destination $copyPath
    $metadata = [ordered]@{ schema_version=1; input_sha256=$sourceHash; script_sha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash; utc=[DateTime]::UtcNow.ToString('o'); export_width=1920; export_height=1080; status='pending'; behavior='unverified'; clicks=@(); frame_rate=$null; font_substitutions='unverified' }
    # Isolated PowerShell worker; never terminate PowerPoint/user processes.
    . (Join-Path $PSScriptRoot 'worker.ps1')
    try {
        Invoke-OracleWorker -ScriptPath $PSCommandPath -Parameters @{Source=$sourcePath;OutputDirectory=$outputPath;ExpectedSha256=$ExpectedSha256;TimeoutSeconds=$TimeoutSeconds;Worker=$true} -OutputDirectory $outputPath -TimeoutSeconds $TimeoutSeconds -CompletionFile 'office.json' -AllowedStatuses @('static-export-complete')
            if (-not (Test-Path -LiteralPath (Join-Path $outputPath 'office.json'))) { throw 'Office metadata missing' }
            $metadata.status = 'static-export-complete'
    } catch { $metadata.status = 'failed: ' + $_.Exception.Message }
    finally { $metadata | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $outputPath 'oracle.json') }
    if ($metadata.status -ne 'static-export-complete') { throw $metadata.status }
    return
}
$copyHash = (Get-FileHash -LiteralPath $copyPath -Algorithm SHA256).Hash.ToLowerInvariant()
if ($copyHash -ne $ExpectedSha256.ToLowerInvariant()) { throw 'Staged copy SHA-256 mismatch' }
$app = $null
$deck = $null
try {
    $app = New-Object -ComObject PowerPoint.Application
    $deck = $app.Presentations.Open($copyPath, -1, 0, 0)
    $details = [ordered]@{ status='static-export-complete'; version=$app.Version; build=$app.Build; slide_width_points=$deck.PageSetup.SlideWidth; slide_height_points=$deck.PageSetup.SlideHeight; slides=@() }
    for ($i=1; $i -le $deck.Slides.Count; $i++) {
        $slide = $deck.Slides.Item($i)
        $png = Join-Path $outputPath ('slide-{0:D2}.png' -f $i)
        $slide.Export($png, 'PNG', 1920, 1080)
        if (-not (Test-Path -LiteralPath $png) -or (Get-Item -LiteralPath $png).Length -eq 0) { throw "Export missing for slide $i" }
        $shapeEvidence=@()
        foreach ($shape in $slide.Shapes) {
            $text=$null; $font=$null
            if ($shape.HasTextFrame -eq -1 -and $shape.TextFrame.HasText -eq -1) {
                $text=$shape.TextFrame.TextRange.Text
                $font=$shape.TextFrame.TextRange.Font.Name
            }
            $shapeEvidence+=@{id=$shape.Id;name=$shape.Name;type=$shape.Type;left=$shape.Left;top=$shape.Top;width=$shape.Width;height=$shape.Height;rotation=$shape.Rotation;text=$text;font=$font}
        }
        $effects=@()
        foreach ($effect in $slide.TimeLine.MainSequence) {
            $effects+=@{type=$effect.EffectType;target_id=$effect.Shape.Id;trigger=$effect.Timing.TriggerType;duration=$effect.Timing.Duration;delay=$effect.Timing.TriggerDelayTime;repeat_count=$effect.Timing.RepeatCount}
        }
        $details.slides += @{ index=$i; png_sha256=(Get-FileHash -LiteralPath $png).Hash; shapes=$shapeEvidence; effects=$effects; transition=@{entry=$slide.SlideShowTransition.EntryEffect;duration=$slide.SlideShowTransition.Duration;advance_on_click=$slide.SlideShowTransition.AdvanceOnClick;advance_on_time=$slide.SlideShowTransition.AdvanceOnTime;advance_time=$slide.SlideShowTransition.AdvanceTime} }
    }
    $details | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $outputPath 'office.json')
} finally {
    if ($null -ne $deck) { $deck.Close(); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($deck) }
    if ($null -ne $app) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($app) }
    # Deliberately no Application.Quit: COM may share a user's running instance.
}
