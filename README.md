# Backlight

Windows screen-edge colour sampler using DXGI Desktop Duplication, with optional
live DDP RGB output and a separate addressing test.
Rust modules separate capture (`capture`),
geometry (`layout`), averaging/smoothing (`colour`), and preview/debug output (`output`).

The native interface uses egui/eframe with smooth fonts, standard controls and a
resizable sampling canvas. Lighting controls, sampling, profiles and Setup have
separate views. The UI runs on its own Windows event thread, while screen capture
and LED output keep their existing update loop. Existing configuration files,
sampling geometry, colour processing and controller mapping are unchanged.

## Setup and run

Click **Setup** at the top right of Live lighting to configure a controller and a rectangular
monitor strip. The seven steps cover its IP/DDP port, capture screen, connected
LED count, direction as viewed from the front, each of the four corners, and a
four-colour side check. Configure the connected LED count in WLED first.
Click within a number/address field to place the cursor. Arrow keys, Home/End,
Backspace and Delete edit individual characters; omit the port to use 4048.
The length/direction page automatically runs a repeating three-second
bright red chase in physical LED order. Watch from the front and choose the observed
clockwise/anticlockwise direction; changing that choice does not reverse the test.

Corner pages automatically illuminate the selected LED at dim output. Valid
number edits update it immediately; incomplete or out-of-range edits keep the
last valid selection lit. Leaving the page stops its lighting pattern.
Choose **Corner has an LED** for a dedicated corner LED, or **Corner is between two LEDs** and
enter the last LED before that corner along the physical strip. Positions are
one-based; LED 1 can begin anywhere, including partway along a side. The final
check automatically shows top red, right green, bottom blue and left yellow. DDP cannot confirm
delivery: check the lights yourself before saving.

**Save** applies the new controller, screen and physical mapping immediately and
remembers them in `backlight-hardware.ini` beside the executable. Sampling bands
and exclusions are kept; individual reading overrides are cleared because the
reading count/order may change. Side totals include LEDs assigned at corners;
dedicated corner outputs blend their two neighbouring readings. **Cancel** resumes
the existing lighting without saving the draft. Hardware settings take precedence
over launcher DDP/default strip geometry, so another user does not need to edit
the `.cmd` files. `--no-saved-settings` ignores hardware setup too. The existing
140-LED installation remains the compatibility fallback until Setup is saved;
the separate legacy position-test utility still describes that installation.

Click **Edit sampling** in the same Backlight window to see the captured active
picture. The window opens at 1200×800 logical pixels; resize or maximize it for easier editing.
The interface respects display scaling, and the sampling canvas keeps the captured
picture's aspect ratio. The captured picture is rendered at the viewport's resolution,
so enlarging the window reveals more detail rather than stretching the small
thumbnail. Geometry outlines are redrawn over that detailed picture. Larger corner handles make resizing boxes easier to grab.
To move just one reading, enable **Individual LEDs**, click a small white reading box,
then drag it. The selected part turns yellow, with a bottom-right resize handle.
Only dragging the solid yellow square resizes the reading; dragging its body moves it.
Its edge and part number appear below the picture. **Delete** restores that part
to its section's default position; selecting an edge returns to whole-section
editing. Individual overrides are remembered and stay attached to the same LED
reading when an edge band changes. Exclusion boxes still apply to moved parts.
Existing saved layouts migrate without losing their bands or exclusions.
Select **Top**, **Right**, **Bottom** or **Left**, drag its coloured box
to move the sampling band, and drag the square at its bottom-right corner to
resize it. Small white boxes show the actual LED readings, including readings
relocated around exclusions. The live colour frame remains visible around the
editor; **Done - lighting** returns to the brightness and timing controls.

Choose **Exclude**, then draw a red box over UI such as buffs, right-hand action
bars, a map or chat. Click inside an exclusion or on its red border to select and move it; use its corner handle to resize it. **Delete** removes the
selected exclusion. Exclusions move affected readings to nearby clear pixels;
bottom readings keep their horizontal positions and move straight upwards.
If necessary, their depth shrinks while their horizontal span stays unchanged;
a bottom reading with no clear pixels above it is blocked rather than moved sideways.
they cannot recover scenery hidden behind the game UI. If a reading cannot fit,
it shrinks until it fits; an entirely excluded picture produces black output.
**Reset** asks for confirmation before clearing all exclusions and individual-part overrides and restoring the
standard edge bands, and removes any bottom inset. It saves that clean layout
within the active profile.

Changes apply live and save when you release the mouse, or close the application.
While editing sampling with Backlight focused, **Ctrl+Z** undoes one complete
action: a drag, resize, exclusion addition/deletion, part deletion, or Reset.
Up to 100 actions are remembered for this session; undo also applies live and
saves the restored layout. Selection clicks do not create undo entries. There
is no redo, and undo history clears when the application closes.
Named layouts save beside the executable as `backlight-profile-<name>.ini`. Coordinates
scale with the active picture when the resolution or black bars change. Saved
custom bands replace preset/CLI sampling positions; Reset saves clean edge bands. LED numbering
and strip ordering stay unchanged. `--no-saved-settings` also ignores these layouts.
The editor picture refreshes at up to 20 Hz; LED output retains the chosen rate.
The Windows taskbar is excluded automatically when exposed on the selected monitor,
including with a custom layout or a clean Reset. When a fullscreen game covers it,
the exclusion disappears. This reads shell window visibility and rectangles only;
it does not inspect or manipulate a game process. Saved exclusions and part positions
are preserved, and the temporary taskbar exclusion is not written into the layout.
Backlight appears in Print Screen and other screenshots. Its visible window
rectangle is temporarily excluded from LED readings to prevent preview feedback;
move it to another monitor for unobstructed readings and editor pictures.
Win+D/minimizing hides the window while capture and LED updates continue. Restore
it from the taskbar or with Win+D. Escape quits only while Backlight has focus.
This uses Backlight's own window handle only. It uses desktop
pixels, with no game memory access, injection, game inputs or game-process control.

The lighting application now uses one GUI window: the colour segments stay
around its edges, with three live sliders in the centre. Drag **Brightness**
(0.00–2.00), **Update rate** (1–240 Hz), or **Smoothing** (0–500 ms) to apply
changes immediately to the preview and DDP output. Smoothing is the time blending
setting; 0 ms disables it. The existing neighbour-colour blend remains unchanged.
The window continues processing input at least 60 times per second even when
you choose a low LED update rate. Actual achieved LED rate still depends on load.
Brightness, adaptive mode, whiteness correction, update rate and smoothing are saved automatically
after a 300 ms pause in adjustments and on close. Reopening restores these
controls from `backlight-settings.ini` beside the executable. Both launchers
share the saved controls and last selected sampling profile.
Explicit CLI control values override saved ones. Use `--no-saved-settings` to
ignore saved controls without changing the file. Headless and bounded diagnostic
runs do not load or save preferences. Missing settings use the defaults below;
invalid files fall back to defaults and report a warning in the window title.

Tick **Adaptive brightness** to disable the manual brightness slider and adjust
the global scale from the current edge samples: dark scenes target 0.15, bright
scenes target up to 2.0, with a smooth transition between them. The scale uses
the selected profile's sampling boxes and exclusions, rather than identifying game
zones by name. It adapts with its own 600 ms time constant even with colour
smoothing disabled. Unticking restores your manual value. Enable it on startup
with `--adaptive-brightness`. Scales above 1.0 boost dim channels but can clip
bright channels at 255, reducing colour detail. Black samples still produce black.

Adaptive mode also preserves local light sources in dark scenes. Each region
tracks both its average and the mean colour of its brightest 5% of spatially
supported pixels. Bright pixels must fill at least 60% of a 16×16 sampling tile
and contain a fully bright 8×8 interior, with encoded luma at least 96/255.
Rejected regions get no highlight boost, including via their ordinary average.
This suppresses name-label/text strokes while keeping broad light patches.
A bright lamp can boost its own region toward a scale of
2.0 and use more of that highlight's colour while other dark regions keep the
global scale. Absolute brightness and contrast thresholds reject dim noise;
boosting fades out in bright scenes. The smoothing slider now fades the final
LED drive after local boosts, gamma, clipping and whiteness correction, including
immediate response at 0 ms. This also smooths manual brightness changes and
correction toggles, rather than letting later processing bypass the fade. Highlights stay local
instead of spreading through neighbour blending. The selected profile's boxes and
exclusions apply to highlight sampling too. Manual mode keeps ordinary averaging.
This is a spatial filter, not text recognition: large thick labels may still
qualify, and very small/thin lights can be suppressed. It detects bright pixel
groups within the sampling strips, not game objects;
lights outside those strips do not affect an LED. The preview shows the same
local output sent to the strip. The displayed AUTO BRIGHTNESS is the global scale.

Tick **Whiteness correction** if neutral light looks blue on your wall. It uses
your WLED colour-picker reference `#FFE0A0` to warm neutral light. That picker
applies colour gamma 2.8 on your controller; DDP bypasses receiver gamma, so the
reference is converted to approximately RGB **255,177,69** in LED PWM. These
fixed calibration values assume the same WLED settings used for your visual test.
Correction matches a narrow range from neutral grey to the measured grey-scene
tint (RGB 106,156,172), using channel ratios independent of brightness. Matching
colours map to the warm reference while retaining each region's peak output
brightness. It fades smoothly as colours diverge from that range, leaving
lavender, pink and other unrelated hues unchanged. Matching occurs before gamma
and clipping, so boosting brightness cannot make purple qualify as white.
Saturated colours remain unchanged. A genuinely cyan colour with the same ratios
as the calibration will still be corrected; colour alone cannot identify intent.
It operates after gamma and brightness clipping, so a 2.0 brightness boost does
not cancel the correction. Full white/full output becomes RGB 255,177,69;
lower scales dim it proportionally. The preview shows corrected output (it may
look cream on the screen even if the wall looks neutral). This is a starting
calibration from your visual test, not a measured strip colour profile.
The screen preview shows the warm reference for matching grey scenes.
Disable the checkbox for faithful sampling without this calibration.
The checkbox is off initially and is remembered. Existing saved settings migrate
without resetting other controls. CLI: `--whiteness-correction`.

Double-click `run-installed-strip.cmd` or `run-wow-lighting.cmd`: each starts
the GUI and finishes without keeping a console open. Both launchers restore the
last selected sampling profile and use the same fallback control settings.
When an update is waiting as `backlight-next.exe`, close Backlight and open a
launcher to replace `backlight.exe`. Launchers pass no custom flags. The original
installation can be restored from `backlight-installed-strip.ini` beside the EXE;
saved hardware Setup takes priority over that fallback.
For terminal diagnostics run the executable directly with `--no-preview`,
`--debug-rgb`, `--list-monitors`, `--help`, or `--updates`; those modes attach
to the invoking console. Fatal GUI errors appear in a message box.

Install stable Rust (MSVC toolchain) and Visual Studio Build Tools with **Desktop
development with C++** and a Windows 10/11 SDK. Run from an x64 Visual Studio Developer
PowerShell or Developer Command Prompt if the linker is not found in a regular shell.

```powershell
cargo run --release -- --list-monitors
cargo run --release -- --monitor 0
cargo run --release -- --monitor 1 --top 40 --right 20 --bottom 40 --left 20 --depth 80 --smoothing-ms 150 --fps 30
cargo run --release -- --debug-rgb
cargo run --release -- --no-preview --debug-rgb
cargo run --release -- --no-preview --updates 60
```

Defaults: monitor 0, top/bottom 32 regions, left/right 18, sampling depth 120
physical pixels, 40 ms exponential smoothing time constant, 60 updates/second.
Use `--help` for all options. `--smoothing-ms 0` disables smoothing. Depth clamps
to display dimensions. Counts must be positive and no greater than edge pixel
length; with preview enabled they must also fit its 800×400 logical canvas.

Both `.cmd` launchers use `--smoothing-ms 0` for immediate colour changes without
time smoothing; their starting rates are listed above. Running the executable directly retains the general defaults
of 60 Hz and 40 ms smoothing. Achieved rate depends on capture/readback, CPU load,
Wi-Fi and the receiver. Bounded `--updates` diagnostics print
the achieved sender update rate, not just the configured target. Smoothing is a
time constant, not a fixed delay; it takes about three time constants to reach
95% of a colour change. It can be disabled entirely with `--smoothing-ms 0`.

The preview displays individual output-colour segments around a dark centre; Escape or closing
the window exits. Headless mode exits with Ctrl+C. Debug output emits one line per
update, space-separated `R,G,B` triples in clockwise order: top left-to-right,
right top-to-bottom, bottom right-to-left, left bottom-to-top. Corners intentionally
overlap between adjacent edges. Regions partition each edge without gaps, including
dimensions not divisible by the count. The first real frame initializes smoothing;
later updates use `alpha = 1 - exp(-elapsed / time_constant)` and floating-point state.

Ambient processing blends each region with its immediate clockwise neighbours,
including across corners, before time smoothing. Defaults are `--blend 0.35`,
`--brightness 1.0`, and `--saturation 1.15`. Blend accepts 0..1; brightness and
saturation accept 0..2. Saturation zero gives greyscale. Brightness zero gives black;
increasing values can clip channels. Deeper sampling reduces the influence of small
edge UI elements but does not remove them. To preserve unmodified region averages,
use `--blend 0 --brightness 1 --saturation 1 --depth 60`.

```powershell
cargo run --release -- --blend 0.45 --depth 160 --saturation 1.2 --brightness 0.8
cargo run --release -- --glow-preview
```

Individual region blocks are the default: each displays a monitor-encoded
visualisation of its processed, time-smoothed LED colour. `--raw-preview` remains a compatibility option
for that same mode. `--glow-preview` explicitly selects the diffuse glow instead.
Debug RGB also reports the processed, time-smoothed colours. The glow is
a stylised preview: its visual falloff and interpolation do not alter output RGB,
and do not predict a specific wall, LED diffuser, or installation brightness.

Capture reuses a CPU-readable staging texture, copies mapped rows respecting their
byte pitch, ignores alpha, and averages RGB bytes with nearest-integer rounding.
This averages encoded SDR channel values, not linear-light radiance. A GPU readback
and full-frame CPU copy occur per new frame; only edge pixels are averaged. The loop
sleeps for the remaining update period; 60 Hz is a target, not a real-time guarantee.
Slow GPU readback, large depths, preview, or console output can reduce throughput.

Unchanged/missing frames retain the last target and continue smoothing. On capture
errors the program releases acquired frames, drops the device/duplication, and retries
once per second. It tracks the selected Windows display name across re-enumeration;
new frame sizes regenerate geometry and staging resources. An unplugged monitor holds
its last colours until it returns. If a mode change makes region counts invalid, the
program reports an error and exits; reduce counts and restart.

## Capture limitations

Automatic black-bar detection is enabled by default. It checks 64 sparse lines per
axis for nearly black borders (all RGB channels at most 8), matched opposite widths
within two pixels or 10% of the smaller bar thickness, whichever is larger, and
consistent boundaries on at least 90% of probes. At least
95% must contain no bright intrusion into the proposed bar; a few dark picture
areas extending beyond the boundary are tolerated. A visible
picture must begin immediately inside both borders. Each bar must be at least two
pixels and no more than a quarter of that axis. The outer 5% of cross-axis positions
are ignored to tolerate thin title/task bars; this is a heuristic, not window detection.
Both introducing and removing a crop require a stable candidate for 500 ms. Missing
DXGI frames on an unchanged desktop still count toward that delay; capture loss resets
detection. The preview title shows the active crop as top,right,bottom,left.

Sampling follows the four edges of the detected picture, keeping the same region
counts and clockwise order. Uneven/asymmetric dark borders and fully dark frames
leave the full screen selected after the stability delay. Dark content can still
resemble real bars; detection cannot guarantee the distinction. Combined bars on all
four sides may fail the conservative boundary checks. Disable or override it when needed:

```powershell
cargo run --release -- --no-auto-crop
cargo run --release -- --black-threshold 5 --bar-delay-ms 800
cargo run --release -- --crop "0,450,0,450"
```

Vertical detection can skip a title strip up to 48 pixels (and at most 5% of image
height) when three nearly full-width black rows follow it. The remaining bar must
still match the opposite bar; a title strip alone does not trigger a crop. The
accepted top crop includes both the strip and the bar. This remains a pixel
heuristic, not detection of a specific application window.

Manual `--crop top,right,bottom,left` uses physical pixels and overrides automatic
detection. It must leave enough pixels for the requested region counts. Automatic
crops that would leave too few pixels are ignored. Sampling depth clamps to the
remaining picture size. Cropping does not remove arbitrary taskbars or other UI.

Use an interactive Windows desktop with a graphics driver supporting Desktop
Duplication (Windows 8 or newer; Windows 10/11 recommended). Landscape/identity
rotation is supported; rotated outputs are explicitly rejected. Secure desktops,
locked sessions, disconnected sessions, protected content, or driver restrictions
may prevent capture or produce black content. The separate hardware cursor is not
composited into the sampled image.

This uses `IDXGIOutput1::DuplicateOutput`, yielding BGRA8 SDR bytes. It does **not**
implement HDR/scRGB capture, colour management, or HDR tone mapping. HDR desktops
and games can therefore yield clipped or inaccurate colours; disable HDR for
predictable SDR results. Exclusive fullscreen games and transitions can invalidate
duplication or return unavailable/black frames depending on game and driver.
Borderless windowed mode is recommended when capture fails. No game injection is used.
The preview itself is part of the captured desktop: keep it away from sampled edges
or place it on another monitor to avoid colour feedback.

Microsoft references: [Desktop Duplication API](https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api)
and [DuplicateOutput](https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutput1-duplicateoutput).

## Checks

## DDP addressing test

## Installed 140-LED monitor strip

### Sampling profiles

Open **Edit sampling**, then click **Profile** at the top right to select a named layout.
Select a profile from the dropdown; the active profile is marked **(current)**.
Click **Apply** to switch. **Default** begins with the same clean edge sections as
Reset, with no exclusions or individual reading overrides. **New Profile** always
starts with clean default sampling; type a name and click Create, then configure
its boxes in Edit sampling. For long dropdown lists, use Up/Down or Home/End.
Switching profiles applies immediately and remembers the selection for the next
launch, including when opening the EXE directly. Brightness controls and hardware
setup remain shared across profiles.

Select a custom profile and click **Delete Profile**, then confirm with **Delete**.
Deleting the current profile switches to Default; Default itself cannot be deleted.

An existing `backlight-sampling-wow.ini` is copied once into the custom **WoW**
profile without changing its boxes, exclusions or individual readings. The old
file is retained. Existing installations initially select WoW so their current
sampling continues; new installations start with Default. The previous desktop
layout is also retained as a file but is not used for Default.

CLI: `--profile "WoW"` or `--profile "Default"`. The old `--wow` flag remains
only as a compatibility alias selecting WoW; it no longer applies special masks
or a bottom inset. The launchers no longer pass it. No game process is inspected.

### Check physical positions

Close the live screen-lighting program so two senders cannot compete, enable WLED,
then double-click `run-position-test.cmd`. It displays top red,
right green,bottom blue,left yellow and the three corners white (front view).
Each stage stays lit until you press Enter; type Q then Enter to finish early.
It then lights LEDs 1,24,44,93,114,140 individually, waiting for Enter at each one,
printing the expected position. LED 44 is assumed top-left. It finishes with black
and does not change WLED settings. The launcher keeps the console open afterward.

For a manual walk through every LED (Enter advances one LED):

```powershell
.\target\release\position-test.exe --chase
```

This diagnostic sends raw test colours independently of game capture, blending,
smoothing and brightness controls. Each channel is capped at 32. It verifies
physical strip order; it does not automatically measure spacing or recalibrate.
Report any incorrect side or marker before changing the mapping. Normal exit sends
black; closing its console early relies on WLED's realtime timeout.

Launch `run-installed-strip.cmd` to preview and stream to `192.168.4.23:4048` at
35% output brightness with LED gamma 2.2. The launcher uses
`target/release/backlight.exe`. Enable WLED power first; controller brightness also affects
the physical result. The controller must be configured for 140 RGB LEDs.

```powershell
cargo run --release -- --installed-strip --ddp 192.168.4.23:4048 --brightness 0.35
```

The installed profile uses 48 top,20 right,50 bottom,19 left regions. Physical
order (left/right viewed from the front): 1–23 bottom towards left; 24 bottom-left
corner; 25–43 left upwards; 44 top-left corner; 45–92 top left-to-right;
93 top-right corner; 94–113 right downwards; 114–140 bottom right-to-left back to
LED 1. LED 44 is assumed to mean top-left despite the original label saying
top-right. Three corner LEDs average their two adjacent edge colours. Each edge
region maps to exactly one remaining LED. The bottom split is 23/50 of the way
from the left, based on uniform spacing; LED 1 begins at that split and LED 140
ends beside it. There is no separate bottom-right corner LED.

The preview remains in screen order, while DDP packets follow physical strip
order. The three extra corner LEDs are derived colours rather than additional
preview segments. `--installed-strip` overrides individual edge-count arguments.
Without it, `--ddp` sends the generic clockwise region order. DDP is opt-in;
running the executable without arguments still previews only. Closing the preview
normally sends black; forced termination relies on WLED's realtime timeout.
This program does not persistently change WLED power, brightness or configuration.

Installed-strip output now applies a configurable `--gamma` curve (1..3) after
time smoothing and before output brightness. Default is 2.2 for the installed
strip and 1 for generic preview mode. This reduces midtone light intensity;
it does not invent saturated colours from neutral screen content. Pure white
still produces white. Use `--gamma 1` for the previous byte-scaling behaviour.
Receiver-side realtime gamma must be disabled when sender gamma is enabled;
the inspected WLED configuration already has it disabled. Debug RGB shows the
resulting LED drive values. The preview applies the inverse curve for monitor
display so those values do not look almost black on screen. That display encoding
never affects DDP output and is an approximate colour preview, not a calibrated
prediction of wall brightness. This exponent is an initial adjustment,
not measured calibration of the strip or wall.

```powershell
cargo run --release --bin ddp-test -- --address 192.168.4.23:4048 --leds 100
```

This sends three-second dim-white tests at the first, middle and last LED, followed
by red/green/blue on LEDs 1/2/3, then black. It sends RGB24 DDP v1 over UDP 4048 at
20 Hz; packets use byte offsets and the final push flag, splitting above 480 LEDs.
Channel values are limited to 16 for this test. Match `--leds` to the receiver's
configured count. It does not change WLED configuration, power state or brightness;
enable the controller before running it. Ordinary exit sends black, but forcibly
terminating it relies on the receiver's realtime timeout. UDP sends have no delivery
acknowledgement; check WLED's realtime status and visually confirm LED positions.

The controller reported only 100 configured LEDs during the first test, despite
300 physical LEDs being connected. Its LED count was not changed. The main sampler
still runs by default with `cargo run --release`.

Protocol reference: [WLED DDP](https://kno.wled.ge/interfaces/ddp/).

```powershell
cargo fmt --check
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

Tests cover clockwise geometry, remainder coverage, bounds/invalid inputs, BGRA
channel ordering, ignored alpha, padded rows, truncated frames, and time-based
smoothing. These deterministic tests do not prove live capture, HDR behaviour,
fullscreen compatibility, or recovery on a particular GPU. See `VALIDATION.md` for
the checks actually executed in the development environment.
