param([Parameter(Mandatory)][string]$EvidencePath)
$ErrorActionPreference='Stop'
$evidence=Get-Content -LiteralPath $EvidencePath -Raw | ConvertFrom-Json
if ($evidence.status -ne 'observed-click-state') { throw 'Neutral observation failed' }
$events=@($evidence.events)
if ($events.Count -ne 7) { throw 'Expected entry plus three dispatch pairs' }
$previous=-1
foreach ($event in $events) {
    if ($event.observed_ms -lt $previous -or $event.slide -ne 1 -or $event.click_count -ne 3) { throw 'Invalid monotonic neutral state evidence' }
    $previous=$event.observed_ms
}
for ($i=0;$i -lt 3;$i++) {
    $before=$events[1+2*$i]; $after=$events[2+2*$i]
    if ($before.click_index -ne $i -or $after.click_index -ne ($i+1)) { throw 'Dispatch did not advance the observed engine click index' }
    if ($before.observed_ms -lt $before.planned_ms) { throw 'Dispatch started before planned monotonic deadline' }
}
if ($evidence.rendered_effect_visibility -ne 'unverified' -or $null -ne $evidence.frame_rate) { throw 'Engine state must not claim rendered capture' }
'PASS: neutral observed engine progression and monotonic schedule; rendered visibility remains unverified'
