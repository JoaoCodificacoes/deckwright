param([Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
$outputPath=[IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $outputPath | Out-Null
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
    $observations+=@{operation='group-ungroup';status='pending: COM group lookup failed; no survival claim'}
    $deck.Save(); $deck.Close(); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($deck); $deck=$null
    @{ office_version=$app.Version; office_build=$app.Build; fixture_sha256=(Get-FileHash $fixture).Hash; observations=$observations } | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $outputPath 'identity.json')
} finally {
    if ($null -ne $deck) { $deck.Close(); [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($deck) }
    if ($null -ne $app) { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($app) }
}



