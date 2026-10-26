# Reference feature coverage

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
