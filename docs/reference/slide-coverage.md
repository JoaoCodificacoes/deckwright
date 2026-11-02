# Slide coverage

## Current reference: v2 thesis deck (adopted 2026-11-02)

The owner adopted the PowerPoint-authored v2 thesis deck (34 slides, 16:9) as
the reconstruction reference. Its bytes, extracted source and outputs stay
private under ignored `fixtures/private/` and `build/`. This table holds
structural counts only, derived locally with `dw query timeline` and package
inspection.

Every slide uses the same validation method:
- `verify_static` reports exact native semantics;
- parsed timelines and transitions are identical (canonical comparison);
- the LibreOffice render is pixel-identical;
- the owner reviewed playback in PowerPoint.

The results are exact on 34/34 slides, with 397 effects and 0 opaque timing
entries. The deck is built by `examples/thesis` (#5, #6) using the engine from
#4; see the closing evidence on #4, #5 and #6.

| Slide | Shapes/pictures/connectors/groups | Effects (typed) | Features |
|---|---|---|---|
| 1 | 3/1/1/0 | none | static |
| 2 | 32/7/1/0 | fade_in:35 | Morph |
| 3 | 93/1/36/3 | fade_in:12 | Morph |
| 4 | 24/5/1/0 | fade_in:20 | Morph |
| 5 | 19/2/1/0 | fade_in:15 | Morph |
| 6 | 22/2/11/0 | fade_in:10, wipe_in:10 | Morph |
| 7 | 26/2/7/0 | fade_in:25 | Morph, auto-start |
| 8 | 21/2/18/0 | fade_in:34 | Morph |
| 9 | 16/10/1/0 | fade_in:17 | Morph |
| 10 | 39/18/9/0 | fade_in:57 | Morph |
| 11 | 21/4/2/0 | fade_in:8, wipe_in:3 | Morph, custom geometry |
| 12 | 27/5/13/0 | fade_in:23, wipe_in:7 | Morph, custom geometry |
| 13 | 5/3/1/0 | none | Morph |
| 14 | 5/3/1/0 | media_play:1 | Morph, auto-start, video |
| 15 | 6/3/1/0 | none | Morph |
| 16 | 11/3/1/0 | fade_in:10 | Morph |
| 17 | 9/3/1/0 | fade_in:1 | Morph |
| 18 | 18/3/1/0 | fade_in:9 | Morph |
| 19 | 13/8/1/0 | fade_in:14, motion:4 | Morph, motion chain |
| 20 | 26/2/2/0 | fade_in:18, wipe_in:4 | Morph |
| 21 | 25/2/6/0 | fade_in:22, wipe_in:2 | Morph, custom geometry |
| 22 | 13/3/1/0 | fade_in:10 | Morph |
| 23 | 23/9/1/0 | fade_in:21, wipe_in:5 | Morph |
| 24 | 8/2/1/0 | none | Morph |
| 25 | 20/5/1/0 | none | static |
| 26 | 24/1/14/0 | none | static |
| 27 | 15/6/4/0 | none | static |
| 28 | 34/1/10/0 | none | static |
| 29 | 31/1/9/0 | none | static |
| 30 | 22/4/10/0 | none | static |
| 31 | 23/1/7/0 | none | custom geometry, static |
| 32 | 47/1/7/0 | none | static |
| 33 | 35/1/5/0 | none | static |
| 34 | 14/1/7/0 | none | static |

## Historical: frozen 25-slide reference

Supervision followup: a later neutral rerun (`deckwright-neutral-observe-5`) blocked in COM before fixture creation, and the former Start-Job/Stop-Job parent also failed to complete its timeout. This is a failed rerun, not replacement evidence for the successful neutral-4 trace. Only the verified owned PowerShell worker/supervisor processes were stopped; no Office process was terminated. Export/identity/observation now share an explicit hidden child-process supervisor; a neutral 30-second sleep was interrupted at the one-second timeout in 1.07 seconds, with argument-fidelity checks and no COM. Office was not retried while the user's PowerPoint instance was busy. Prior static exports and source trace 3 remain the actual successful Office evidence.

Wave 2 first-three-slide static acceptance evidence: authored output SHA-256 `5474339c64e21daec11b6a4e56dade57d7d4adc777dd5068d49bfbc6cf07aeb4` opened and exported under PowerPoint 16.0 build 20527. All three 1920 × 1080 PNG hashes exactly match the approved reference's same-build exports. Native top-level shape counts 5/40/18, IDs/names/types, text/assigned fonts and exact geometry/rotation all match. Independent package comparison also found all nested identity/text/kind sequences and all nine picture placement asset hashes/transforms/crops equal, with native counts including slide 3's three groups and 178 authored source-key extensions overall. Private evidence: Windows Temp `deckwright-authored-static-office-1/`, Linux `/tmp/deckwright-static-office-comparison-1.json` and `/tmp/deckwright-static-package-comparison-1.json`. Static timing/transitions are intentionally absent from the authored output; these results do not establish animation equivalence or identity-extension survival after Office edits/save.

Read-only reference engine trace: `deckwright-reference-trace-3/observation.json` in Windows Temp completed under the same Office build with unchanged approved source and staged hashes before/after. After a fixed 2500 ms entry observation delay, click counts for slides 2/3/9/10/11/12/15/16/17/18/21/22 were 3/2/8/4/3/5/0/1/3/5/8/0. Trace records scheduled and before/after Next timestamps with observed engine indices. Slide 15's post-entry GetClickIndex returned `Illegal value`; that field is null with its error retained, so this is explicitly partial engine evidence. The first immediate-entry trace misleadingly reported zeros during initialization; it remains preserved, and the delayed trace supersedes those zero-count interpretations. No live pixel/frame-rate evidence or media/autoplay/repeat/Morph equivalence is claimed. Reference behavior gates remain open.

Wave 2 calibration (2026-10-26): `oracle/observe.ps1` generates a separate neutral three-numbered-effect fixture and observes slideshow COM state under a bounded worker. Office 16.0 build 20527 reported slide 1, click count 3 and click indices 0→1→2→3 around scheduled `View.Next()` calls at monotonic deadlines. Private evidence: Windows Temp `deckwright-neutral-observe-3/observation.json`; planned deadlines and before/after observations are separate fields. This proves engine click dispatch for the neutral fixture only. No rendered frames were captured; visual entrance timing, media/repeats/Morph and all reference-slide schedules remain pending. `Slide.Export` and CreateVideo are not used as live-frame evidence. A cleanup warning records that slideshow exit had already invalidated the temporary presentation before Close; no Application.Quit or process termination occurred.

`oracle/fonts.ps1` captured 265 installed GDI font families and 538 hashed font files on this host (`deckwright-fonts-wave2.json` in Windows Temp). These describe the environment. COM Font.Name in office.json describes assignment, not the font used for each rendered glyph; actual substitutions and line-break calibration remain unverified. File presence alone is not treated as rendering evidence.

Against the reference's direct declarations, Segoe Print appears in this GDI family list, while Aptos and Aptos Display do not. GDI enumeration does not inventory all Office-managed/cloud font availability; absence here therefore does not prove a PowerPoint substitution. All three rendered glyph-font identities remain unverified.

The user authorized the current Downloads file on 2026-10-25 after the frozen-plan hash no longer matched. The former SHA-256 was `795452eefdaadf44201ecdcbc3ad2311cdcf734d36a21ffc502bbacd38e75ef9` (4,205,625 bytes). The authoritative replacement is `ee8cce03cf0201150fdd6f9c75f5d551538ed90d6ffae1c417e82b939e027e2b` (3977194 bytes), rechecked before and after read-only PowerPoint export. It has 25 slides and 164 package parts; dimensions remain 12192000 × 6858000 EMU.

Private evidence: `fixtures/private/inventory.json`, `fixtures/private/oracle-authorized/reference.pptx`, and `fixtures/private/oracle-metadata/{oracle.json,office.json,slide-*.png}`. These files are ignored and must not be published. Inventory includes exact raw XML for shapes, text runs, geometry, notes, crop, timing and relationships.

PowerPoint 16.0 build 20430 exported all 25 PNGs at 1920 × 1080; slide dimensions are 960 × 540 points. COM metadata records top-level shape geometry/text/font, MainSequence effects and transitions. Nested geometry remains in package evidence. Font substitutions, installed font inventory and live playback are pending.

| Slide | Shapes/pictures/connectors/groups | Effects (package preset classes) | Other features | Task / validation |
|---|---|---|---|---|
| 1 | 3/1/1/0 | none | static | 3; XML + static PNG; playback pending |
| 2 | 32/7/1/0 | entr:35 | Morph | 3/4/6; XML + static PNG; playback pending |
| 3 | 93/1/36/3 | entr:12 | Morph | 3/4/6; XML + static PNG; playback pending |
| 4 | 28/5/1/0 | entr:24 | Morph | 5/4/6; XML + static PNG; playback pending |
| 5 | 36/2/1/0 | entr:32 | Morph | 5/4/6; XML + static PNG; playback pending |
| 6 | 23/2/12/0 | entr:22 | Morph | 5/4/6; XML + static PNG; playback pending |
| 7 | 49/2/7/0 | entr:43 | Morph | 5/4/6; XML + static PNG; playback pending |
| 8 | 39/18/12/0 | entr:61 | Morph | 5/4/6; XML + static PNG; playback pending |
| 9 | 27/5/13/0 | entr:31 | Morph, custom geometry | 5/4/6; XML + static PNG; playback pending |
| 10 | 16/7/4/0 | entr:18, exit:3, path:3 | Morph, motion | 5/4/6; XML + static PNG; playback pending |
| 11 | 34/2/24/0 | entr:52, exit:1, path:1 | Morph, motion | 5/4/6; XML + static PNG; playback pending |
| 12 | 25/2/14/0 | entr:31 | Morph | 5/4/6; XML + static PNG; playback pending |
| 13 | 20/6/1/0 | entr:19 | Morph | 5/4/6; XML + static PNG; playback pending |
| 14 | 16/11/1/0 | entr:18 | Morph | 5/4/6; XML + static PNG; playback pending |
| 15 | 6/3/1/0 | mediacall:1 | Morph, video | 5/4/6; XML + static PNG; playback pending |
| 16 | 8/2/1/0 | entr:1 | Morph | 5/4/6; XML + static PNG; playback pending |
| 17 | 17/2/1/0 | entr:9 | Morph | 5/4/6; XML + static PNG; playback pending |
| 18 | 37/8/2/0 | emph:3, entr:38, path:4 | Morph, motion, scale | 5/4/6; XML + static PNG; playback pending |
| 19 | 27/1/6/0 | entr:27 | Morph, custom geometry | 5/4/6; XML + static PNG; playback pending |
| 20 | 18/4/1/0 | entr:15 | Morph | 5/4/6; XML + static PNG; playback pending |
| 21 | 25/9/2/0 | entr:29 | Morph | 5/4/6; XML + static PNG; playback pending |
| 22 | 8/2/1/0 | none | Morph | 5/4/6; XML + static PNG; playback pending |
| 23 | 34/1/10/0 | none | static | 5; XML + static PNG; playback pending |
| 24 | 47/1/7/0 | none | static | 5; XML + static PNG; playback pending |
| 25 | 35/1/5/0 | none | static | 5; XML + static PNG; playback pending |

Static equivalence requires ordered text, native editability, asset hash/placement/crop and notes comparison, then line-break/local geometry review on this same Office/font environment. PNG existence is not visual equivalence. The first 2–3, 9–12, 15–18 and 21–22 click schedules remain pending; remaining slides need recordings before reconstruction gates. Slide 15 video autoplay, slide 18 emphasis/repeats/motion and all Morph incoming/outgoing states require explicit timed observations. No video has been claimed as click evidence.

Task 1 remains partial: input frozen and static export captured; live schedules, font calibration and independent benchmark-host evidence are open. Task 8 identity matrix is provisional; see ../decisions/identity.md.

Review verification on 2026-10-26 used the staged approved reference because Downloads had changed again. Hardened harness exports under PowerPoint 16.0 build 20527 are in ignored `fixtures/private/oracle-reviewed/` and `oracle-roundtrip-reviewed/`. The package worker's `roundtrip-a.pptx` opened and exported successfully: all 25 Office slide metadata records (including text/geometry/effects/transitions) and all 25 static PNG hashes matched the approved reference. This is static/recorded-metadata evidence, not animation playback or a complete guarantee against silent Office repair. Earlier captures remain labeled build 20430.
