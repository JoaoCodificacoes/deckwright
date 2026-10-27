param(
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidateRange(1,3600)][int]$TimeoutSeconds=60,
    [bool]$Worker=$false,
    [string]$ScriptPath=$PSCommandPath,
    [string]$Source='',
    [string]$ExpectedSha256='ee8cce03cf0201150fdd6f9c75f5d551538ed90d6ffae1c417e82b939e027e2b',
    [int[]]$Slides=@(2,3,9,10,11,12,15,16,17,18,21,22),
    [ValidateRange(0,100)][int]$MaximumClicksPerSlide=10
)
$ErrorActionPreference='Stop'
$outputPath=[IO.Path]::GetFullPath($OutputDirectory)
if (-not $Worker) {
    if ($Source) {
        $Source=(Resolve-Path -LiteralPath $Source).Path
        if ((Get-FileHash -LiteralPath $Source).Hash -ne $ExpectedSha256) { throw 'Source SHA-256 mismatch' }
    }
    if (Test-Path -LiteralPath $outputPath) { throw 'Use a fresh output directory' }
    New-Item -ItemType Directory -Path $outputPath | Out-Null
    if ($Source) { Copy-Item -LiteralPath $Source -Destination (Join-Path $outputPath 'reference.pptx') }
    . (Join-Path $PSScriptRoot 'worker.ps1')
    $supervision=[ordered]@{status='pending';timeout_seconds=$TimeoutSeconds;cleanup='Only owned slideshow/presentation; no Application.Quit or process termination'}
    try {
        Invoke-OracleWorker -ScriptPath $PSCommandPath -Parameters @{OutputDirectory=$outputPath;TimeoutSeconds=$TimeoutSeconds;Worker=$true;ScriptPath=$ScriptPath;Source=$Source;ExpectedSha256=$ExpectedSha256;Slides=$Slides;MaximumClicksPerSlide=$MaximumClicksPerSlide} -OutputDirectory $outputPath -TimeoutSeconds $TimeoutSeconds -CompletionFile 'observation.json' -AllowedStatuses @('observed-click-state')
        if (-not (Test-Path (Join-Path $outputPath 'observation.json'))) { throw 'Observation metadata missing' }
        $result=Get-Content (Join-Path $outputPath 'observation.json') -Raw | ConvertFrom-Json
        if ($result.status -ne 'observed-click-state') { throw 'Observation did not complete successfully' }
        $supervision.status='completed'
    } catch { $supervision.status='failed';$supervision.error=$_.Exception.Message;throw }
    finally { $supervision | ConvertTo-Json | Set-Content (Join-Path $outputPath 'supervision.json') }
    return
}
$app=$null; $deck=$null; $show=$null
$events=[Collections.Generic.List[object]]::new()
$clock=[Diagnostics.Stopwatch]::StartNew()
$record=[ordered]@{schema_version=1;status='pending';provenance='PowerPoint slideshow COM state observations; no rendered frames';script_sha256=(Get-FileHash $ScriptPath).Hash;events=$events;rendered_effect_visibility='unverified';frame_rate=$null}
$record.selected_slides=$Slides; $record.maximum_clicks_per_slide=$MaximumClicksPerSlide
try {
    if ($Source) {
        $copy=Join-Path $outputPath 'reference.pptx'
        $record.source_sha256=(Get-FileHash -LiteralPath $Source).Hash
        $record.copied_sha256=(Get-FileHash -LiteralPath $copy).Hash
        if ($record.source_sha256 -ne $ExpectedSha256 -or $record.copied_sha256 -ne $ExpectedSha256) { throw 'Source/staged SHA-256 mismatch' }
    }
    $app=New-Object -ComObject PowerPoint.Application
    $record.office_version=$app.Version; $record.office_build=$app.Build
    if ($Source) { $deck=$app.Presentations.Open($copy,-1,0,0) }
    else {
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
    $Slides=@(1)
    }
    $record.selected_slides=$Slides
    $deck.SlideShowSettings.ShowType=2
    $show=$deck.SlideShowSettings.Run()
    function Sample($kind,$planned) {
        $sample=[ordered]@{kind=$kind;planned_ms=$planned;observed_ms=$clock.Elapsed.TotalMilliseconds;slide=$null;click_index=$null;click_count=$null;state=$null;errors=@()}
        try { $sample.slide=$show.View.Slide.SlideIndex } catch { $sample.errors+=@{field='slide';error=$_.Exception.Message} }
        try { $sample.click_index=$show.View.GetClickIndex() } catch { $sample.errors+=@{field='click_index';error=$_.Exception.Message} }
        try { $sample.click_count=$show.View.GetClickCount() } catch { $sample.errors+=@{field='click_count';error=$_.Exception.Message} }
        try { $sample.state=$show.View.State } catch { $sample.errors+=@{field='state';error=$_.Exception.Message} }
        $events.Add($sample)
    }
    foreach ($slideIndex in $Slides) {
    if ($slideIndex -lt 1 -or $slideIndex -gt $deck.Slides.Count) { throw 'Selected slide out of range' }
    if ($Source) { $show.View.GotoSlide($slideIndex,-1) }
    Sample 'slide-entry' $null
    if ($Source) {
        # Entry during Morph can report zero before the engine installs click steps.
        # This fixed observation delay is not evidence of visual transition completion.
        Start-Sleep -Milliseconds 2500
        Sample 'post-entry-delay' $null
    }
    $entry=$clock.Elapsed.TotalMilliseconds
    $clicks=3
    if ($Source) {
        try { $clicks=[Math]::Min($show.View.GetClickCount(),$MaximumClicksPerSlide) }
        catch { $clicks=0; $events.Add(@{kind='dispatch-skipped';slide=$slideIndex;observed_ms=$clock.Elapsed.TotalMilliseconds;error=$_.Exception.Message}) }
    }
    for ($n=1;$n -le $clicks;$n++) {
        $deadline=$entry+($n*700)
        while ($clock.Elapsed.TotalMilliseconds -lt $deadline) { Start-Sleep -Milliseconds 10 }
        Sample 'before-next' $deadline
        $show.View.Next()
        Sample 'after-next' $deadline
    }
    }
    $record.status='observed-click-state'; $record.visual_gate='pending: click index is engine state, not independent pixel evidence'
} catch { $record.status='failed'; $record.error=$_.Exception.ToString(); throw }
finally {
    if ($null -ne $show) { try {$show.View.Exit()} catch {} }
    if ($null -ne $deck) { try { $deck.Close() } catch { $record.cleanup_warning=$_.Exception.Message }; try { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($deck) } catch {} }
    if ($null -ne $app) { try { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($app) } catch {} }
    if ($Source) {
        $record.source_sha256_after=(Get-FileHash -LiteralPath $Source).Hash
        $record.copied_sha256_after=(Get-FileHash -LiteralPath $copy).Hash
        if ($record.source_sha256_after -ne $ExpectedSha256 -or $record.copied_sha256_after -ne $ExpectedSha256) { $record.status='failed-source-integrity' }
    }
    $record | ConvertTo-Json -Depth 10 | Set-Content (Join-Path $outputPath 'observation.json')
}
