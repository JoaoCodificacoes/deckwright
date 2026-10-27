param([Parameter(Mandatory)][string]$OutputDirectory,[ValidateRange(1,3600)][int]$TimeoutSeconds=60,[bool]$Worker=$false)
$ErrorActionPreference='Stop'
$outputPath=[IO.Path]::GetFullPath($OutputDirectory)
if (-not $Worker) {
    if (Test-Path -LiteralPath $outputPath) { throw 'Use a fresh output directory' }
    New-Item -ItemType Directory -Path $outputPath | Out-Null
    . (Join-Path $PSScriptRoot 'worker.ps1')
    $supervision=@{status='pending';timeout_seconds=$TimeoutSeconds;script_sha256=(Get-FileHash $PSCommandPath).Hash}
    try {
        Invoke-OracleWorker -ScriptPath $PSCommandPath -Parameters @{OutputDirectory=$outputPath;TimeoutSeconds=$TimeoutSeconds;Worker=$true} -OutputDirectory $outputPath -TimeoutSeconds $TimeoutSeconds -CompletionFile 'identity.json'
        if (-not (Test-Path (Join-Path $outputPath 'identity.json'))) { throw 'Identity evidence missing' }
        $supervision.status='completed'
    } catch { $supervision.status='failed';$supervision.error=$_.Exception.Message;throw }
    finally { $supervision | ConvertTo-Json | Set-Content (Join-Path $outputPath 'supervision.json') }
    return
}
$fixture=Join-Path $outputPath 'neutral-identity.pptx'
if (Test-Path -LiteralPath $fixture) { throw 'Use a fresh output directory' }
$app=$null; $deck=$null
try {
    $app=New-Object -ComObject PowerPoint.Application
    $deck=$app.Presentations.Add(0)
    $slide=$deck.Slides.Add(1,12)
    $shape=$slide.Shapes.AddShape(1,10,10,100,50)
    $shape.Name='!!neutral-morph'
    $shape.Tags.Add('deckwright.source','neutral.shape.one')
    $shape.TextFrame.TextRange.Text='Neutral identity fixture'
    $deck.SaveAs($fixture,24)
    $deck.Close()
    $deck=$app.Presentations.Open($fixture,0,0,0)
    $slide=$deck.Slides.Item(1); $shape=$slide.Shapes.Item(1)
    $observations=@()
    function Observe($operation,$object) {
        @{ operation=$operation; id=$object.Id; name=$object.Name; tag=$object.Tags.Item('deckwright.source') }
    }
    $observations+=Observe 'save-reopen' $shape
    $shape.Left=60; $shape.Top=30
    $observations+=Observe 'move' $shape
    $copied=$shape.Duplicate().Item(1)
    $observations+=Observe 'shape-duplicate' $copied
    $duplicateSlide=$slide.Duplicate().Item(1)
    $observations+=Observe 'slide-duplicate' $duplicateSlide.Shapes.Item(1)
    $observations+=@{operation='copy-paste';status='pending: clipboard paste failed in headless COM; manual isolated clipboard test required'}
    $second=$slide.Shapes.AddShape(1,200,100,80,40)
    $second.Name='neutral-second'; $second.Tags.Add('deckwright.source','neutral.shape.two')
    try {
        $range=$slide.Shapes.Range([object[]]@(1,$slide.Shapes.Count))
        $group=$range.Group()
        $children=@()
        for ($i=1;$i -le $group.GroupItems.Count;$i++) { $children+=Observe 'group-child' $group.GroupItems.Item($i) }
        $observations+=@{operation='group';type=$group.Type;range_count=$range.Count;children=$children}
        $ungrouped=$group.Ungroup()
        for ($i=1;$i -le $ungrouped.Count;$i++) { $observations+=Observe 'ungroup-child' $ungrouped.Item($i) }
    } catch { $observations+=@{operation='group-ungroup';status='failed';error=$_.Exception.ToString()} }
    $deck.Save(); $deck.Close(); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($deck); $deck=$null
    $deck=$app.Presentations.Open($fixture,-1,0,0)
    foreach ($item in $deck.Slides.Item(1).Shapes) { $observations+=Observe 'final-save-reopen' $item }
    $deck.Close(); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($deck); $deck=$null
    @{ status='completed'; office_version=$app.Version; office_build=$app.Build; fixture_sha256=(Get-FileHash $fixture).Hash; observations=$observations } | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $outputPath 'identity.json')
} finally {
    if ($null -ne $deck) { $deck.Close(); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($deck) }
    if ($null -ne $app) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($app) }
}
