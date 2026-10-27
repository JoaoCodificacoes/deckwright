param(
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidateRange(1,3600)][int]$TimeoutSeconds=60,
    [bool]$Worker=$false,
    [string]$ScriptPath=$PSCommandPath
)
$ErrorActionPreference='Stop'
$outputPath=[IO.Path]::GetFullPath($OutputDirectory)
if (-not $Worker) {
    if (Test-Path -LiteralPath $outputPath) { throw 'Use a fresh output directory' }
    New-Item -ItemType Directory -Path $outputPath | Out-Null
    $job=Start-Job -FilePath $PSCommandPath -ArgumentList @($outputPath,$TimeoutSeconds,$true,$ScriptPath)
    $supervision=[ordered]@{status='pending';timeout_seconds=$TimeoutSeconds;cleanup='Only owned slideshow/presentation; no Application.Quit or process termination'}
    try {
        if ($null -eq (Wait-Job $job -Timeout $TimeoutSeconds)) { Stop-Job $job; throw 'Observation timeout; no Office process terminated; copied fixture may remain open' }
        Receive-Job $job -ErrorAction Stop
        if ($job.State -ne 'Completed') { throw 'Observation worker failed' }
        if (-not (Test-Path (Join-Path $outputPath 'observation.json'))) { throw 'Observation metadata missing' }
        $supervision.status='completed'
    } catch { $supervision.status='failed';$supervision.error=$_.Exception.Message;throw }
    finally { Remove-Job $job -Force; $supervision | ConvertTo-Json | Set-Content (Join-Path $outputPath 'supervision.json') }
    return
}
$app=$null; $deck=$null; $show=$null
$events=[Collections.Generic.List[object]]::new()
$clock=[Diagnostics.Stopwatch]::StartNew()
$record=[ordered]@{schema_version=1;status='pending';provenance='PowerPoint slideshow COM state observations; no rendered frames';script_sha256=(Get-FileHash $ScriptPath).Hash;events=$events;rendered_effect_visibility='unverified';frame_rate=$null}
try {
    $app=New-Object -ComObject PowerPoint.Application
    $record.office_version=$app.Version; $record.office_build=$app.Build
    $deck=$app.Presentations.Add(0)
    $slide=$deck.Slides.Add(1,12)
    for ($n=1;$n -le 3;$n++) {
        $shape=$slide.Shapes.AddShape(1,50,($n*100),300,70)
        $shape.TextFrame.TextRange.Text="Effect $n"
        $shape.Name="neutral-effect-$n"
        [void]$slide.TimeLine.MainSequence.AddEffect($shape,1,0,1)
    }
    $fixture=Join-Path $outputPath 'neutral-effects.pptx'
    $deck.SaveAs($fixture,24)
    $record.fixture_sha256=(Get-FileHash $fixture).Hash
    $deck.SlideShowSettings.ShowType=2
    $show=$deck.SlideShowSettings.Run()
    function Sample($kind,$planned) {
        $events.Add([ordered]@{kind=$kind;planned_ms=$planned;observed_ms=$clock.Elapsed.TotalMilliseconds;slide=$show.View.Slide.SlideIndex;click_index=$show.View.GetClickIndex();click_count=$show.View.GetClickCount();state=$show.View.State})
    }
    Sample 'slide-entry' $null
    $entry=$clock.Elapsed.TotalMilliseconds
    for ($n=1;$n -le 3;$n++) {
        $deadline=$entry+($n*700)
        while ($clock.Elapsed.TotalMilliseconds -lt $deadline) { Start-Sleep -Milliseconds 10 }
        Sample 'before-next' $deadline
        $show.View.Next()
        Sample 'after-next' $deadline
    }
    $record.status='observed-click-state'; $record.visual_gate='pending: click index is engine state, not independent pixel evidence'
} catch { $record.status='failed'; $record.error=$_.Exception.ToString(); throw }
finally {
    if ($null -ne $show) { try {$show.View.Exit()} catch {} }
    if ($null -ne $deck) { try { $deck.Close() } catch { $record.cleanup_warning=$_.Exception.Message }; try { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($deck) } catch {} }
    if ($null -ne $app) { try { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($app) } catch {} }
    $record | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $outputPath 'observation.json')
}
