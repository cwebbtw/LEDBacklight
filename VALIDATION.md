# Validation performed

## Generic sampling profiles — 11 October 2026

Added a main-window profile selector and creation by copying the current layout.
Profile selection applies live and persists across EXE/launcher starts. Default
uses clean edge sampling; the existing WoW file migrates once, retaining all
bands, exclusions and individual overrides. The legacy file remains untouched.
Launchers no longer pass --wow; the hidden compatibility flag selects WoW only.
Passed 90 regular tests, formatting, all-target Clippy and release build. Migration
tests cover exact geometry preservation, active selection, repeat migration and
profile filename validation. No live GUI, WoW process access or lighting tests.

## Setup direction rotation — 11 October 2026

The direction page now offers Start/Stop rotation with a repeating seven-second
chase in physical LED order. Direction selection records the observed direction
without reversing the chase. Static LED-1 controls are hidden on this page.
Passed 87 regular tests, formatting, all-target Clippy and release build.
Tests cover LED order, wraparound, continued repetition and the removed control's
inactive click area. No live GUI or LED test performed.

## Setup field cursor editing — 11 October 2026

Address and numeric setup fields now insert characters at a clicked cursor
position. Left/Right, Home/End, Backspace and Delete operate within the field;
first typing no longer clears its value. A visible cursor marks the position.
Passed 83 regular tests, formatting, all-target Clippy and release build.
Tests change an IP digit and port digit independently, validate the result, and
cover insertion/deletion and cursor bounds. No live GUI or LED test performed.

## Configurable hardware Setup — 11 October 2026

Added Setup opposite Edit sampling in the existing window. Seven steps configure
DDP address/port, capture monitor, LED count (4–1200, subject to preview dimensions),
front-view direction, four corner anchors and a final coloured side check. Corners
may be dedicated LEDs or between successive LEDs. LED 1 can start anywhere.
Explicit test buttons hold dim white probes or side colours at 20 Hz; calibration
pauses normal lighting and tests stop on cancel/save/exit. No timeout advances a
step. UDP status does not imply delivery. Text inputs replace their value on first
typing, with numeric corner stepping. Cancel discards the draft.

Validated corner order yields a one-to-one physical/canonical mapping and per-edge
counts. Dedicated corner outputs blend adjacent readings. Saved hardware overrides
the legacy launcher layout/address and restores on launch; no saved hardware leaves
the user's previous installation intact. Save validates screen/crop geometry and
persists before applying, clears the old controller, reinitializes sampling/capture
and final smoothing, retains bands/masks and clears obsolete part overrides.

Passed 81 regular tests, formatting/check, all-target Clippy and release build.
Tests cover the existing off-centre bottom start, between-LED corners, reversed
direction, varied counts/start offsets, every physical LED mapped exactly once,
invalid corner order, persistence, wizard count editing/completion and cancel.
Five tests are ignored by default; the pure offline wizard renderer was run
explicitly and all seven pages generated. Visually inspected connection, corner
and final-check pages. No application windows were opened or controlled, live
screen frames captured, or DDP lighting packets sent. Hardware verification and
live keyboard/mouse checks remain for the user. Release staged and hash checked.

## Action-based sampling undo — 11 October 2026

Ctrl+Z, checked only with Backlight focused, restores one sampling action while
the editor is open. Mouse-down/release boundaries group a complete drag/resize
into one entry; history stores up to 100 profile snapshots for the session.
Exclusion additions/deletions, part changes/deletions and Reset use the same
snapshot path. Selection/no-movement clicks do not change geometry or create
entries. Undo during a drag commits/restores that action and stops the drag.
Restored profiles use the existing live-update and save path; no redo added.

Passed 76 regular tests, formatting, all-target Clippy and release build. Tests
cover multi-update drag grouping, subsequent resize undo ordering, restoring a
deleted exclusion and Reset, selection-only clicks, default-profile restoration
and cancelling an active drag. Four external-image tests remain ignored. Staged
and hash-checked; live shortcut/LED behavior has not been tested on the desktop.

## Draggable picture-edge handles — 11 October 2026

Editor hit testing now uses unclipped picture coordinates for visible borders
and selected corner handles extending into the surrounding margin. New exclusions
still start inside the picture. Move/resize/drawing results remain constrained to
valid picture coordinates, while distant margin clicks do not select an edge.
Passed 73 regular tests, formatting, all-target Clippy and release build. Added
regression coverage for a bottom-right handle outside the picture, a bottom-border
drag from the margin, and a distant non-hit. Four image tests remain ignored.
Staged and hash-checked; no live GUI control or lighting test performed.

## Bottom exclusion alignment — 11 October 2026

Bottom readings now relocate upwards only, preserving each reading's horizontal
position and width. Both preset and saved/individual layouts use this rule.
Stacked exclusions are checked together. Saved readings reduce depth if no
full-depth reading fits; an entirely obstructed column is blocked rather than
relocated sideways. Other edges retain their existing relocation behaviour.
Saved geometry and LED ordering are unchanged.

Passed 72 regular tests, formatting/check, all-target Clippy and release build.
Regression tests cover partially overlapping exclusion ends, stacked exclusions,
individual bottom overrides, depth reduction and a fully blocked column. Four
external-image tests remain ignored. Staged and hash-checked the new executable;
no live GUI or lighting test was performed.

## Screenshots and Show Desktop — 11 October 2026

Removed Backlight's display-affinity capture exclusion so Windows screenshots
can include the application. Feedback protection now adds the visible own-window
rectangle to temporary sampling exclusions, clipped to the selected monitor and
active picture. Moving/minimizing the window invalidates those exclusions. Saved
profiles combine these zones with their existing exclusions; default readings
that cannot move clear of the application are blocked rather than sample its UI.
Pixels obscured by the window cannot be recovered; the editor picture can show
the window when it is on the captured monitor.

Minimized/zero-sized windows pump events without rendering or applying mouse
controls. The capture/DDP loop continues. Escape only exits when Backlight has
foreground focus, preventing an inactive window's stale key state from quitting.
No global hotkeys, focus changes, or game-process interaction were added.

Passed 69 regular tests (four external-image tests remain ignored), formatting,
all-target Clippy and the release build. Added rendering suspension/restoration
coverage; existing runtime-zone and clipping tests exercise sampling exclusions.
The release was staged and hash checked. No applications were opened or controlled;
Print Screen and Win+D behavior still require manual Windows verification.

## Detailed editor picture and visible-taskbar avoidance — 11 October 2026

Editor capture now creates a viewport-resolution picture rather than stretching
the small logical thumbnail. Editor-only presentation uses a window-sized buffer;
reading outlines, band/exclusion outlines and handles are redrawn at that size.
Logical hit coordinates are retained, including letterbox offsets. Native picture
sampling remains capped at 20 Hz; closed-editor rendering retains its lightweight
buffer. The enlarged editor adds rendering work but does not change the configured
LED update rate or the final-output smoother.

Added read-only inspection of shell primary/secondary taskbar windows. Geometry
is excluded only when sample points resolve to an exposed shell taskbar root;
a covered fullscreen taskbar is not treated as visible merely because its HWND
has the visible flag. Rectangles are clipped to the selected monitor and active
picture, with negative-origin monitors and auto-hidden offscreen bars handled.
Visibility/geometry changes invalidate the sampling layout. Runtime taskbar
zones are combined with saved masks and individual overrides and are not saved.
No game process is opened or modified; no window positioning/focus/input changes.

Passed 68 regular tests, formatting/check, all-target Clippy and release build.
New tests preserve alternating one-pixel source detail lost by the small
thumbnail, retain the outer colour frame, cover visible/covered/offscreen bars,
monitor/crop offsets, and ensure a moved part cannot sample the taskbar or bypass
saved exclusions. Replayed the supplied screenshot offline and visually inspected
the 1600x800 editor render. Four external-image tests remain ignored ordinarily;
this editor replay was run explicitly. Live shell detection and wall appearance
remain unverified. Update staged and hash checked; no GUI opened, new desktop
capture performed or LED packets transmitted during the work.

## Reset to a saved clean layout — 11 October 2026

User requested Reset clear everything rather than reactivate the preset UI
exclusions. Reset now saves an explicit layout with standard edge bands, no
exclusions, no individual-part overrides and no WoW bottom inset. It preserves
LED counts/order and lighting controls. An explicit saved profile is necessary:
removing it would reinstate WoW's built-in masks on restart.

Passed 65 regular tests, formatting/check, all-target Clippy and release build.
New regression resets an edited WoW layout, checks every mask/override is cleared,
verifies standard bottom-edge geometry and round-trip persistence, and confirms
reapplying the WoW preset after reload does not restore its exclusions. Four
external-image tests remain ignored. Update staged and hash verified; no GUI
control, desktop capture or LED transmissions performed.

## Move individual reading parts — 11 October 2026

Added Parts mode to the sampling editor: click a white reading box to select
and drag it independently; a yellow outline and larger resize handle identify
the selection. The footer names its edge and part. Delete resets only that
override; selecting a band returns to whole-section editing. Overrides use
normalized active-picture coordinates keyed by the unchanged clockwise reading
index, and exclusions still relocate/block them as needed. Layout format v2
stores overrides; v1 files load with their bands/exclusions unchanged.

Passed 64 regular tests, formatting, all-target Clippy and release build.
New regressions move/resize/delete a part through the editor, round-trip its
saved override, validate v1 migration and duplicate rejection, confirm every
other reading remains unchanged and check the moved top part still drives
physical LED 55. Four external-image tests remain ignored. Staged executable
verified against the release build. No app control, live desktop capture or
LED transmission; live usability remains for the user to check.

## Larger resizable preview — 11 October 2026

Preview opens at 1600x800 instead of 800x400, with resizing/maximizing enabled.
Aspect-ratio scaling preserves the interface and screen-picture proportions.
Mouse input is mapped from unscaled client coordinates through the rendered
content rectangle, matching minifb's integer letterbox offsets; margin clicks
are ignored. Selected resize handles grow from 10x10 to 16x16 logical pixels.
The rendering buffer remains 800x400, so enlarging the window does not add a
larger CPU rendering pass or change LED capture, output rate or smoothing.

Passed 62 regular tests, formatting/check, all-target Clippy and release build.
New input regression covers doubled dimensions, square/wide windows, odd client
sizes, margins and zero dimensions. Four external screenshot tests remain
ignored. Release is staged for user restart; no GUI control or live testing
performed while WoW is running.

## Smooth the final LED output — 11 October 2026

User reported abrupt changes with smoothing enabled; saved controls showed 150 ms.
Code inspection found smoothing before nonlinear local highlight boost, gamma,
clipping and whiteness correction. Those later operations were not constrained
by the output fade. Replaced the separate average/highlight smoothing stages in
the application loop with one final RGB-drive smoother. The slider and saved
duration are retained; 0 ms remains immediate. Adaptive global brightness keeps
its existing 600 ms adaptation. Completely excluded readings still turn black
immediately instead of replaying stale colours.

Passed 61 regular tests, all-target Clippy with warnings denied, formatting and
release build. New regression drives a dark scene through lamp appearance,
white-correction toggle and lamp disappearance, verifying a 150 ms final fade
limits each 10 ms channel step to 17/255; setting zero then produces the exact
target immediately. Four external-image tests remain ignored. Staged a matching
release executable for user restart. No GUI control, desktop capture or LED
transmission performed; the user's actual abrupt-change cause and wall response
have not been observed directly.

## Configurable sampling editor — 11 October 2026

Added an editor inside the existing preview window with an aspect-preserving
captured active-picture thumbnail, four movable/resizable edge bands, up to
32 editable exclusion rectangles, actual reading outlines, reset and return to
lighting controls. The colour frame remains visible. Changes apply immediately;
the thumbnail alone is capped at 20 Hz. Existing LED numbering and DDP ordering
are unchanged. WoW's preset now includes top buffs and right action bars.

Separate WoW/desktop normalized layouts are validated on load and written beside
the executable on mouse release and shutdown using temporary-file replacement.
Reset removes the custom profile. Invalid saved files fall back with a title
warning. Exclusions relocate readings; oversized readings shrink when necessary,
and completely blocked readings are forced black even after smoothing/blending.
They do not reconstruct the world underneath UI. Saved layouts override preset
sampling geometry; the existing no-saved-settings switch bypasses them.

Passed 60 regular tests, formatting/check, all-target Clippy with warnings denied,
and release compilation. Tests cover bright-icon exclusion while retaining a
world lamp, tiny-band bounds/LED count, fully blocked regions, profile validation,
save/reload/replacement/reset, moving and resizing, reverse-direction exclusion
drawing, overlapping band handles and UI masks, and padded/cropped thumbnail
rendering. Four external-image tests remain ignored in the ordinary suite.

Ran the new screenshot render separately using the supplied 3439x1439 image
exported read-only to BGRA, pitch 13756, and visually inspected its offline
800x400 render. No GUI was opened, no new desktop capture performed and no LED
packets sent. SetWindowDisplayAffinity excludes only Backlight's own HWND from
desktop capture to prevent feedback; it logs a failure. No game-process access,
memory reads, injection, game-file changes or simulated game input were added.
Actual live interaction, window-exclusion behavior and wall output remain to
be verified by the user. The checked executable is staged for manual restart.

## Stronger label rejection and no fallback boost — 11 October 2026

The user reported that the first tile filter still highlighted names in the
live application. Its earlier cropped screenshot replay did not validate the
full-monitor text size. Tightened support to 16x16 tile coverage >=60% plus a
solid 8x8 bright interior, with the same encoded-luma threshold. No sorting or
additional image pass: per-tile bit masks test interior support.

Also fixed a separate leakage path: rejected regions formerly returned their
average as the highlight, which could trigger the above-scene boost for a large
label. Unsupported regions now return zero highlight, leaving ordinary colour
averages intact at the global scale. The existing smoother releases stale boosts
using its configured duration rather than introducing a hard visual switch.

Passed formatting/check, 52 regular tests, all-target Clippy with warnings
denied, and release build. Added large glyph tests with 2/4/6-pixel strokes,
requiring no local boost, while a filled same-colour lamp still boosts. The
six-pixel fixture also reproduces the old average-fallback boost. Replayed the
original supplied lantern/label screenshot separately: lantern highlight
RGB 254,228,7 retained; rejected label highlight RGB 0,0,0. That replay passed;
three external screenshot tests remain ignored in the regular suite.

Staged the release update with matching hash; no app control, new desktop capture
or LED transmission. The latest photographed scene has not been replayed at its
native desktop resolution, and actual wall appearance remains unverified. Large
solid text or very small lights remain limitations of spatial filtering.

## Reject thin labels from local highlights — 11 October 2026

Highlight sampling now visits pixels in 8x8 tiles. Only tiles with at least
16 bright pixels and >=60% coverage at encoded luma >=96/255 contribute to
the highlight histogram. Ordinary RGB averaging still includes every pixel
and is identical to manual mode. The highlight mean uses up to the brightest
5% of region pixels from eligible bright support; regions without support
fall back to their ordinary average. This rejects thin strokes while retaining
filled light patches without sorting or a second image pass.

Passed 51 regular tests, formatting/check, all-target Clippy with warnings
denied and release compilation. Added a two-pixel yellow-glyph versus same-RGB
lantern test, checking actual local boost and unchanged ordinary averages.
Updated padded-row/channel and hot-pixel regressions for spatial support.

Separately replayed the user's supplied 567x766 screenshot exported read-only
to BGRA (pitch 2268), using lantern and label rectangles. The lantern average
was RGB 75,67,32 and highlight RGB 254,232,10; the label average/highlight both
remained RGB 29,30,7. The label received no local boost and the lantern scale
exceeded 1.5 in the dark-scene fixture. Screenshot replay passed. Three external
image tests are ignored in the regular suite; this new replay was run explicitly.

Staged backlight-next.exe with matching build hash. No GUI control, new desktop
capture, controller writes, LED transmission or app restart. Actual wall response
remains unverified. This is spatial filtering rather than OCR: large/thick text
can still qualify and very small/thin lights can be suppressed.

## Local highlight brightness — 11 October 2026

Adaptive mode now samples regional averages and brightest-5% RGB means using a
single-pass 64-bin integer-luma histogram. A minimum 16-pixel support suppresses
isolated hot pixels. Manual sampling retains ordinary averages. Local boost uses
absolute highlight brightness, contrast against its region/scene and a dark-scene
gate, smoothly approaching a maximum scale of 2.0. Average samples still drive
the original 600 ms global adaptation; highlights never raise its target.
Highlights use the existing colour smoothing duration and stay within their
sampling region. Crop, WoW UI avoidance, whiteness calibration, physical strip
mapping and preview/DDP share the same output pipeline. The global slider display
continues to show scene-wide scale. No new setting or preferences migration.

Passed formatting/check, all 50 regular tests (two screenshot replays ignored),
all-target Clippy with warnings denied and the release build. New regressions
cover padded BGRA rows and channel ordering, local yellow-lamp colour, ordinary
average equivalence, single hot pixels and small-region support; dark neighbours,
bright individual regions, manual/uniform/noise/empty/missing-sample cases,
highlight release and 0 ms smoothing. Existing calibration and DDP tests pass.

Staged target/release/backlight-next.exe with matching release-build hash. No
live capture, GUI control, controller writes, LED transmission or running-app
restart performed for this change. Real lamp-scene wall appearance, sustained
frame rate and additional CPU cost have not been measured.

## Selective grey-tint correction — 10 October 2026

Replaced the broad pale-colour match that washed out lavender/pink with a narrow
normalised RGB corridor from neutral grey toward the previously captured
RGB 106,156,172 ghost-scene tint. Project onto that calibrated direction and
fade by distance: full match within 0.035 normalised RGB distance, zero at 0.085.
Allow tint strength up to 1.25 times the measured axis, retaining the saturation
limit. Matching uses source colour before gamma and clipping; output brightness,
warm PWM reference, saved checkbox, GUI controls and removed footer are retained.
Matching cyan colours can still be corrected, since RGB alone cannot distinguish
a real scene hue from this calibrated tint.

Passed formatting/check, cargo test (45 regular tests, two screenshot tests
ignored), all-target Clippy with warnings denied and release compilation in
target/whiteness-build. Tests retain the measured ghost-scene reference and
verify lavender, pink, warm beige, unrelated cyan and strong colours remain
unchanged at brightness 0.35, 1.0 and 2.0. Also verify tint matching across
brightness levels and a smooth monotonic boundary toward purple. Existing
installed-strip/DDP packet regression confirms calibrated white for all LEDs.

Staged target/release/backlight-next.exe with hash matching the release build.
No live capture, GUI control, controller writes, LED transmission or running-app
restart performed for this revision. Real purple-scene/wall appearance remains
for user verification after manually closing Backlight and launching again.

## Pale-scene correction and footer removal — 10 October 2026

The user clarified that #FFE0A0 is the desired reference for the grey/white WoW
scene, including its captured cyan tint. A five-update background headless run
without DDP acquired four frames and measured typical processed source colours
such as RGB 106,156,172 (gamma 1, brightness 1, no correction). The former
neutral-component correction retained a substantial coloured remainder.

The enabled correction now maps source saturation <=50% fully to the warm PWM
reference, retaining regional peak output brightness. It fades smoothly between
50% and 80% saturation; stronger source colours remain unchanged. Classification
occurs before gamma and clipping. This is an intentional pale-colour effect and
will also warm genuinely pale cyan scenes. Removed the calibration footer text
at the user's request; retained the checkbox and sliders.

Formatting, 44 regular tests, all-target Clippy with warnings denied and release
build passed. Two external screenshot tests remain ignored. Added regressions
for measured WoW samples, full pale correction, the transition and strong-colour
preservation. A corrected five-update headless run acquired five frames, exited
0 and produced warm RGB for all 137 regions, e.g. 164,114,44 and 175,121,47.
These short startup runs are not update-rate benchmarks. No DDP transmission,
GUI control, controller write or running-app restart was performed. Physical
wall colour remains unverified. Final build staged as backlight-next.exe for
the launchers to install after the user closes the old app.

## Whiteness calibration fix — 10 October 2026

Read-only diagnosis confirmed the running release executable, saved correction
enabled, active WLED 0.15.1 DDP, effect colour gamma 2.8 and realtime no-gc=true.
WLED's v0.15.1 cfg.cpp, FX_fcn.cpp and udp.cpp confirm that picker/effect colours
are gamma-corrected while this realtime configuration bypasses gamma. The old
raw #FFE0A0 gains therefore did not reproduce the user's picker calibration.

Converted that visual reference to rounded PWM RGB 255,177,69 using exponent
2.8. Replaced the saturation cutoff with neutral-component decomposition:
calibrate the shared minimum RGB channel and retain the coloured remainder.
This keeps saturated RGB/CMY untouched and provides meaningful correction for
pale cyan. Preview footer identifies WHITE CALIBRATION V2. The reference is
specific to the controller settings used for the user's visual calibration.

Passed cargo fmt/check, cargo test (43 passed, two screenshot tests ignored),
all-target Clippy with warnings denied, and the release build in
target/whiteness-build. Regression checks cover pale cyan, calibrated white,
black/greys, zero brightness, boosted clipping, disabled equivalence and
corrected RGB surviving installed-strip mapping and DDP packet construction
for all 140 LEDs, including corners. Staged target/release/backlight-next.exe
for installation by the existing launchers after the user closes the old app.

No GUI control, app restart, new capture session, controller configuration
write or LED transmission performed. The screenshot visibly contains a cyan
tint; this change calibrates its neutral component rather than removing all
scene colour. Physical wall appearance remains unverified.

## Optional whiteness correction — 10 October 2026

Added a remembered GUI checkbox and `--whiteness-correction` CLI option. Uses
the user's WLED reference #FFE0A0 (RGB 255,224,160) as LED output gains for
neutral source colours. Smoothly fades to no correction at 50% saturation.
Applied after gamma/brightness clipping so boost values cannot clip the
calibration back to equal-channel white. Existing v1 preferences load with the
option off and retain all other controls; new saves include the option in v2.

Formatting, all 41 regular tests, all-target Clippy and a release build in
`target/whiteness-build` passed; two screenshot replay tests ignored. New checks
cover white and grey calibration, black, zero brightness, boost to 2.0,
saturated colours, gradual fade, disabled equivalence, independent checkbox
behaviour and saved-setting migration/round-trip. No GUI automation, capture
session or LED transmission was performed for this change, respecting the
user's request not to control apps while WoW is running.

The standard executable was locked by a running process. Staged the new build
as `target/release/backlight-next.exe`; both launchers install the staged file
on the next user-initiated launch after the old app is closed. Physical wall
appearance and live checkbox behaviour remain for user verification.

## Remember GUI controls — 10 October 2026

Added automatic preferences for manual brightness, adaptive checkbox, update
rate and smoothing. Saves are debounced by 300 ms and flushed on close, using
a temporary file and atomic replacement beside the executable. Saved controls
override defaults; explicit CLI controls override saved values. Launchers no
longer pass fixed control values, and retain their old values as first-run
fallbacks. Diagnostics and `--no-saved-settings` bypass persistence.

Formatting, compilation, all-target Clippy, release build and all 37 regular
tests passed; two screenshot tests ignored. Tests cover validation, precedence,
save/reload, replacing an existing file and preserving the manual value while
adaptive is enabled. An isolated copy under `target/settings-smoke` saved
brightness 1.23, adaptive=true, 90 Hz and 25 ms without DDP, closed with exit
code 0, then reopened without those options. Visually verified adaptive mode,
90 Hz and 25 ms were restored; the file retained manual brightness 1.23.
No physical LED response was tested in this persistence check.

## Adaptive brightness up to 2.0 — 10 October 2026

Adaptive mode now targets 0.15–2.0 from the existing edge samples, with a 600 ms
time constant. Its GUI checkbox disables the manual slider and preserves the
manual value for restoration. Updated tests verify monotonic bounded targets,
time independence, empty-sample retention, manual restoration and enabling from
a manual scale of 2.0 without truncating it to 1.0. Physical wall appearance
and channel clipping at the higher scale require user verification.
Formatting, all 34 regular tests, all-target Clippy and the release build passed;
two screenshot replay tests remain ignored.

## Single-window live controls — 10 October 2026

Added mouse-drag controls for brightness, update rate and smoothing inside the
existing colour preview. Settings are read by the active loop; smoothing changes
preserve its existing state, while 0 ms responds immediately. Input polling is
independent of low LED update rates. Windows GUI subsystem removes the automatic
console; launchers now start the GUI and return, preserving existing user values
(WoW brightness 0.75 / 100 Hz / 0 ms; normal 0.35 / 60 Hz / 0 ms).

Formatting, compilation, release build and all-target Clippy passed. 31 regular
tests passed across three binaries; two screenshot replay tests were ignored.
New tests cover drag capture across rows, bounds, FPS period, zero smoothing,
and runtime smoothing changes without resetting state. Opened the rebuilt GUI
without DDP and visually confirmed live DXGI colour segments and all three
sliders in one window. Automated dragging could not complete because concurrent
user mouse input interrupted automation; physical response while dragging is
not claimed as verified. No LED latency or game performance benchmark performed.
A bounded 120-update GUI smoke test exited successfully with 67 captured frames
(46.9 updates/s during that brief run at a 100 Hz target). A subsequently queried
controller reported no active DDP session; ongoing transmission from this GUI
was not verified in this check.

## Launcher target reduced to 60 Hz — 10 October 2026

Changed both installed-strip launchers to `--fps 60`, retaining disabled
smoothing (`--smoothing-ms 0`). Checked both launcher commands with `--help`.
No Rust source or executable changed; CPU usage at this setting was not measured.

## Smoothing disabled in launchers — 10 October 2026

Changed both installed-strip launchers from `--smoothing-ms 10` to
`--smoothing-ms 0`, retaining the 120 Hz target. This uses the existing
executable; no Rust source or executable changed. Checked both launcher commands
with `--help`; physical LED response and flicker require user verification.

## Faster launcher settings — 10 October 2026

Both installed-strip launchers now request 120 Hz and 10 ms smoothing using
the existing release executable; no Rust source or executable changed.
Non-transmitting 240-update desktop benchmarks with the installed WoW geometry
measured 107.3 updates/s without preview (111 acquired frames) and 106.8 updates/s
with preview (178 acquired frames). Both captured DISPLAY1 at 3440×1440.
The sandboxed preview attempt was denied capture access; the successful preview
benchmark ran with approval outside the sandbox. These runs did not send DDP,
measure physical LED latency, or establish performance under game load.

UI geometry is cached until frame dimensions or active crop/inset change.
Final 120-update non-transmitting smoke runs measured 54.4 updates/s with the
WoW UI profile (54 shifted regions) and 54.6 without it (zero shifted regions).

## WoW UI avoidance zones — 10 October 2026

Added explicit normalised zones based on the supplied minimap/chat/action-bar
layout, plus objectives, damage/issue panel and player frame. A contaminated
rectangle moves to the nearest clear rectangle of the same size, without using
brightness as a selection rule. Tests verify clean samples unchanged, bounds and
sizes preserved, synthetic UI colours excluded, and no-space fallback.
Passed 23 main tests and six utility tests, all-target Clippy and standard release
build; two screenshot tests ignored. The WoW launcher now points to the standard
release binary. Real UI positions and wall appearance require user verification;
this is a fixed-layout approximation, not automatic UI recognition or recovery
of obscured game pixels.

## WoW bottom sampling inset — 10 October 2026

Added an explicit `--wow` preset (bottom inset = active picture height / 6),
overridable by `--bottom-inset` in physical pixels. Only bottom regions move;
the strip mapping and remaining edge rectangles stay unchanged. Added tests for
ordering, zero inset, depth clamping, invalid inset and sampling scene colours
above a synthetic dark action bar. Passed 20 main tests plus six utility tests,
all-target Clippy, formatting and release build in `target/letterbox`.
Two screenshot replay tests were ignored. The supplied real WoW screenshot
was not replayed through this inset; chosen 240-pixel default at 1440p is a
starting point based on its visible UI and requires user confirmation.

## Faster response — 10 October 2026

Changed the default update target from 30 to 60 Hz and smoothing time constant
from 120 to 40 ms. Passed all 24 regular tests across the three binaries, Clippy,
formatting and the standard release build; two screenshot tests ignored.
120-update runs measured 54.6 updates/s with preview and 54.5 without preview.
A separate 180-update DDP run measured 55.9 updates/s; WLED reported live DDP
and 56 LED fps during the run. It completed successfully, sent black on exit,
and previous controller power/brightness/transition/override were restored.
These brief desktop runs do not establish sustained performance under game load.

## Preview encoding correction — 10 October 2026

The gamma-adjusted PWM values had been displayed directly on the monitor, making
the segments much darker. Added an inverse curve in the preview path only. DDP
packets and debug output still use the same LED drive values. A new regression
checks display brightening, colour ordering, black, gamma-1 identity and immutable
drive input. Passed 18 main tests, two DDP tests, four position tests, all-target
Clippy, formatting and the standard release build; two screenshot tests ignored.
The installed-strip launcher again points to `target/release/backlight.exe`.
The preview is an approximate display of the LED colour, not measured wall light.

## Pale-light diagnosis and LED gamma — 10 October 2026

User reported distinct primary colours during position calibration. Readback
confirmed one installed-strip sender, WLED live DDP from this PC, RGB hardware,
and realtime gamma disabled (`no-gc:true`, output correction `cr:false`). A separate
non-transmitting screen sample produced approximately neutral output averages
(top 18,18,18; right 19,18,14; bottom 10,9,7; left 18,17,15). These samples were
of the desktop at diagnostic time, not extracted from the user's phone photo.

Added adjustable gamma before brightness scaling, default 2.2 for installed-strip
output. Tests verify neutral preservation, primary-colour preservation, black,
identity gamma, and brightness applied after gamma. Passed 17 main tests, two
DDP utility tests, four calibration utility tests, Clippy and formatting; two
screenshot replays were ignored. The locked standard executable required a build
to `target/letterbox/release/backlight.exe`; the installed launcher now uses it.
No controller settings were changed. Physical wall appearance of this correction
requires user verification.

## Manual calibration pacing — 10 October 2026

Removed automatic timing from position calibration, including the optional chase.
Each stage refreshes DDP every 50 ms until Enter; Q then Enter or stdin EOF ends
the test and sends black. Verified with a localhost UDP test that the stage
continues sending while awaiting input, advances only on command, and accepts
quit. Four position-test tests, release build, formatting and all-target Clippy
passed. Physical LEDs were not exercised for this pacing change.

## Position calibration utility — 10 October 2026

Built `target/release/position-test.exe`; passed three utility tests (side range
counts/corners and DDP packet tests), formatting check and all-target Clippy with
warnings denied. Added side-colour and individually labelled marker sequences,
plus optional 250 ms per LED chase. The new utility has not been run against the
physical strip; the user can launch `run-position-test.cmd` to verify positions.

## Installed 140-LED mapping — 10 October 2026

Controller configuration readback confirmed 140 RGB LEDs. Added physical mapping
for the split bottom, 19 left LEDs,48 top LEDs,20 right LEDs,50 bottom LEDs and
three derived corners, assuming LED 44 is top-left. Mapping tests verify every
edge sample occurs exactly once, range endpoints, bottom wrap, and corner averaging.
Passed formatting, release compilation, all regular tests (16 main tests plus
two DDP-test tests; two screenshot replay tests ignored), and all-target Clippy
with warnings denied.

A 90-update headless installed-strip run at brightness 0.2 acquired five frames
and exited successfully. WLED reported live DDP from 192.168.4.20 during the run.
The test temporarily enabled WLED at brightness 64 and restored its previous
power, brightness, transition and realtime override settings afterward. Normal
sampler exit sends a black frame. Physical side orientation and the assumed
top-left corner require user confirmation; no visual wall-colour verification
was performed.

## First hardware DDP test — 10 October 2026

WLED 0.15.1 at 192.168.4.23 reported 100 RGB LEDs configured. Built the separate
`ddp-test` release executable, passed both packet tests (RGB order/header/length,
byte offset and final push across packet boundaries), Clippy on all targets with
warnings denied, and formatting check.

A twelve-second low-brightness addressing test sent dim white to LEDs 1,50,100,
then red/green/blue to LEDs 1,2,3. WLED reported `live:true`, `lm:DDP`, and sender
192.168.4.20 during the test. The executable exited successfully and sent black.
Temporary controller changes were restored; confirmed `on:false`, `bri:124`,
`transition:7`, `lor:0`. The configured LED count remained 100.

This confirms the controller receives DDP, not that the physical LEDs lit correctly.
Physical position and channel-order verification require the user's observation.
Neither LEDs 101..300 nor live screen-to-LED streaming were tested.

## Letterbox with title strip — 10 October 2026

Passed 12 regular tests and both explicitly enabled screenshot replay regressions,
Clippy with warnings denied, and formatting check. The supplied 3439×1387 screenshot
contains a 23-pixel title strip, black rows 23..93, and a 66-pixel bottom bar.
The detector now reports `94,0,66,0`, including the title strip in the top crop.
It permits opposite bar widths to differ by up to 10% (minimum two pixels).
A new synthetic regression checks a title strip preceding letterbox bars and
rejects a title strip without bars. The earlier pillarbox screenshot still passes.
These are screenshot replays; live game detection remains unverified.

The standard executable was locked; the updated release is built in
`target/letterbox/release/backlight.exe`.

## WoW black-bar regression — 10 October 2026

The supplied 3436×1390 screenshot was decoded with System.Drawing to BGRA bytes.
The left boundary measured 450 pixels on 60/64 probes; four dark artwork areas
extended farther inward. The right boundary measured 452 on all probes. The old
61/64 agreement requirement rejected the left edge.

The detector now accepts 58/64 matching boundaries while still requiring 61/64
probes to have no bright intrusion inside the proposed bar. Eleven regular tests
passed, plus the explicitly enabled `supplied_wow_screenshot` regression test.
That test detected crop `0,452,0,450` and verified nonblack region averages on both
side edges. The replay test is normally ignored and requires the exported BGRA
file via `BACKLIGHT_SCREENSHOT_BGRA`; the user's screenshot is not bundled.
Clippy with warnings denied and formatting checks passed. Live game behaviour
was not verified; this validation replays the attached screenshot.

The standard executable was locked by a running instance. The fixed release
was built to `target/ambient/release/backlight.exe` instead.

## Ambient glow update — 10 October 2026

Passed formatting check, release build (`--target-dir target/ambient`), all ten
tests, and Clippy with warnings denied. New processing tests verify neighbour wrap
at the clockwise seam, neutral settings, brightness zero, greyscale saturation,
empty input, and channel clipping.

Live executable smoke tests completed 30 glow-preview updates (29 acquired frames)
and ten block-preview updates (ten acquired frames) without reported errors.
A two-update headless debug run with brightness zero produced only black RGB values.
The preview appearance was not visually inspected; no physical LED hardware was tested.
The executable is `target/ambient/release/backlight.exe`. Earlier executables have
not been overwritten, so an already running instance does not gain these changes.

## Black-bar update — 10 October 2026

Passed `cargo fmt --check`, `cargo build --release --target-dir target/bar-update`,
`cargo test` (nine tests), and `cargo clippy --all-targets -- -D warnings`.
The extra tests cover symmetric pillar/letterboxing with padded rows, uneven and
asymmetric borders, fully black/no-bar frames, stability delay/removal, and invalid
manual crop bounds.

Live smoke tests acquired frames with automatic detection (45 updates, 16 acquired
frames, no bars detected), `--no-auto-crop`, and manual `--crop "0,100,0,100"`
(three updates, two acquired frames, expected crop reported). Malformed and
image-removing crops correctly failed. Automatic detection has not been tested
against the supplied WoW scene or other real black-bar content, and the updated
preview has not been visually inspected.

The existing `target/release/backlight.exe` was running and locked by Windows.
The updated executable is `target/bar-update/release/backlight.exe`; the running
instance was left untouched. Close the old program before rebuilding in the
usual target folder with `cargo build --release`.

## Initial implementation

Development checks on Windows, 9 October 2026, using Rust 1.99.0
(`x86_64-pc-windows-msvc`), Visual Studio 2022, Windows SDK 10.0.26100.
Rust/rustfmt/Clippy and the missing SDK were installed during implementation.

Passed on the final source:

- `cargo fmt --check`
- `cargo build --release` (a debug build also passed before the final iterator change)
- `cargo test`: five tests passed, zero failed
- `cargo clippy --all-targets -- -D warnings`

Live release-binary diagnostics:

- `--list-monitors` identified DISPLAY1 at 3440×1440 and DISPLAY2 at 2560×1440.
- `--no-preview --updates 60 --debug-rgb` on monitor 0 completed with one acquired
  frame (unchanged desktop). Total process wall time was 2285 ms, including startup;
  this is not a sustained changing-desktop performance benchmark.
- Debug output had 60 lines with 100 RGB triples per line; the final line had
  nonzero channel values. The initial waiting update emitted black.
- `--monitor 1 --no-preview --updates 30` completed with 22 acquired frames.
- `--updates 60` created and updated the preview window without reported errors,
  acquiring 58 frames on monitor 0. Its appearance was not visually inspected.
- `--depth 0 --no-preview --updates 1` correctly exited with a configuration error.

These runs verify basic DXGI frame acquisition and processing on both connected
monitors, plus execution of the preview update path. They do not independently
validate captured colours against a known live test pattern. Synthetic tests
verify geometry, averaging, padding, channel order, and smoothing.

Not tested: games, sustained 30 Hz under load, exclusive fullscreen, HDR,
display mode changes, unplug/replug, GPU/device loss, lock/unlock, protected
content, rotated monitors, or recovery after actual capture loss.
