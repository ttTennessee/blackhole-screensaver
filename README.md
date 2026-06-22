# blackhole-screensaver

A Schwarzschild black-hole screensaver for Windows. Your idle desktop becomes
the sky around a wandering black hole — real null-geodesic integration bends
the wallpaper into the photon ring, a Shakura–Sunyaev accretion disk orbits
with relativistic Doppler beaming, and a screen-space wind swirls just
outside the event horizon.

Pure Rust + wgpu, no C++, no game engine. Ships as a single `.scr` file plus
a small background daemon that keeps a fresh desktop snapshot in shared
memory so the saver can show your *real* desktop instead of the black screen
Windows hands it.

Inspired by [ghostty-blackhole](https://github.com/s0xDk/ghostty-blackhole)
(MIT). The physics derivation and many parameter defaults come from there;
no code is linked in.

---

## What you see

- **Shadow** — rays with impact parameter below `b_crit = 3√3/2 · r_s`
  spiral through the horizon. The wallpaper behind the hole really is gone,
  not just darkened.
- **Gravitational lensing** — escaped rays project back onto the desktop
  "sky" plane. Icons and wallpaper bend, magnify, and double inside the
  Einstein ring.
- **Photon ring** — a thin bright ring at `1.5 r_s` emerges from rays that
  wind around the hole and escape; it isn't painted on, it's where the
  integration lingers.
- **Accretion disk** — a thin Keplerian disk pierced multiple times per
  ray. Shakura–Sunyaev temperature profile rendered as blackbody color,
  with relativistic Doppler shift and beaming: the side moving toward you
  is blue-hot and boosted, the receding side dim and red.
- **Screen-space halo swirl** — a warm wind just outside the shadow that
  circulates against the wallpaper so the orbit reads at any size, even
  when gravitational time dilation slows the real disk.
- **Lensed starfield** — sparse procedural stars sampled with the *bent*
  ray direction, so they smear into arcs around the hole for free.
- **Time dilation** — the disk's pattern winds down as the hole grows,
  scaled by `√(1 − 1.5/r)`.

Everything except the swirl falls out of per-pixel geodesic integration —
nothing is faked.

---

## Install

### Build from source

Prereqs on Windows:

- `rustup` with the `stable-x86_64-pc-windows-msvc` toolchain
- Visual Studio Build Tools — only the **MSVC linker** + **Windows 10/11 SDK**
  components. You don't write any C++.

```powershell
git clone https://github.com/<you>/blackhole-screensaver
cd blackhole-screensaver
cargo build --release
```

The binary lands at `target\release\blackhole.exe`.

### Install as a Windows screensaver

1. Rename `blackhole.exe` → `blackhole.scr`.
2. Either copy `blackhole.scr` into `C:\Windows\System32\`, or right-click
   the file → **Install**.
3. Open **Settings → Personalization → Lock screen → Screen saver**, pick
   *blackhole*, set an idle timeout, click *Preview* to test.
4. On first run the configure dialog asks whether to enable a background
   daemon at boot. Say *Yes* — without it the screensaver only has the
   black screen Windows shows it. See [why a daemon](#why-a-background-daemon).

You can manage the daemon from the tray icon in the notification area:
**Pause capture**, **Start with Windows**, **Quit**.

---

## Command-line interface

The Windows screensaver subsystem invokes the `.scr` with these arguments
automatically; you do not run them by hand.

| Args         | Mode      | Behavior                                            |
| ------------ | --------- | --------------------------------------------------- |
| `/s`         | Saver     | Fullscreen render, any input quits                  |
| `/p <HWND>`  | Preview   | Renders inside the settings panel preview pane      |
| `/c`         | Configure | Info dialog + first-run autostart prompt            |
| *(none)*     | Configure | Same as `/c`                                        |
| `--daemon`   | Daemon    | Background snapshot capturer + tray icon            |

---

## Configuration

Optional. If the file is missing or unparseable, sensible defaults are used.

**Path:** `%APPDATA%\BlackholeScreensaver\config.toml`

```toml
# Seconds for the hole to grow from minimum to maximum, then reset.
cycle_seconds   = 300.0

# Shadow radius range as a fraction of screen height.
radius_min      = 0.05
radius_max      = 0.55

# Lissajous drift amplitude (uv units) and frequency (Hz).
drift_amp_x     = 0.30
drift_amp_y     = 0.20
drift_speed_x   = 0.013
drift_speed_y   = 0.017
```

Run `blackhole.exe` with no arguments to see the exact path on your
machine — the dialog prints it.

Most disk look constants (color temperature, Doppler mix, beaming exponent,
streak contrast, etc.) live as `const` declarations at the top of
[`src/shader.wgsl`](src/shader.wgsl). They are not exposed through
`config.toml` on purpose — editing them invites a rebuild but lets you keep
the binary lean. Lower `N_STEPS` if a giant hole tanks your frame rate.

---

## Why a background daemon?

Windows blacks the screen out *before* invoking `.scr /s`, so a `BitBlt`
inside the saver only ever captures black. The daemon (`blackhole.exe
--daemon`, autostarted by a `HKCU\...\Run` entry) wakes every 5 seconds,
grabs the desktop, and writes it into a named shared-memory region
(`Local\BlackholeScreensaver_DesktopSnapshot_v1`). When the saver fires, it
maps the same region and renders against the most recent snapshot.

Niceties:

- **Tray icon** with pause / autostart / quit.
- **Foreground-fullscreen detection** — exclusive-fullscreen games and
  videos are detected and the 5 s capture is skipped automatically, so the
  daemon won't interrupt a game even though it's running.
- **Single-instance** — a named kernel mutex keeps a second `--daemon`
  process from starting.
- **Detached process** — the daemon survives the dialog that spawned it
  closing.

Total cost: ~50 MB RSS, < 0.1% CPU on a Ryzen-class machine, zero disk I/O.

---

## How it works

### Lensing

Per-pixel leapfrog integration of the Schwarzschild Binet acceleration
`a = -(3/2) h² x / r⁵` (with `h = |x × v|` conserved). Only pixels with
impact parameter below `rout + 3` pay for the integration; the rest go
through an analytic weak-field deflection fitted against the integrator,
so the handoff radius is invisible.

### Disk

A thin Keplerian sheet at the equatorial plane (tilted `DISK_INCL`).
Each ray's intersections with the plane are detected by sign-change tests
on `dot(x, n)`; at each crossing the local emissivity uses a Shakura–
Sunyaev temperature profile rendered as a Tanner-Helland blackbody, then
boosted by `g^N` where `g = √(1 − 1.5/r) / (1 − β · k̂)` combines
gravitational redshift, Doppler shift, and relativistic beaming. Emission
accumulates HDR, transmittance multiplies down through opacity; the result
is tonemapped on top of the lensed wallpaper sample.

### Background

One `BitBlt` of the entire virtual desktop (`SM_X/YVIRTUALSCREEN` origin,
not `(0, 0)` — secondary monitors can sit in negative coordinates) into
an sRGB texture per saver invocation, read from the daemon's shared
memory. No per-frame capture, no WGC reentrancy hazards. If the virtual
desktop is larger than the GPU's `max_texture_dimension_2d`, the
snapshot is downscaled on the CPU before upload.

### Animation

CPU drives shadow size and Lissajous position into a uniform buffer; the
rest of the motion is `iTime` inside WGSL. The disk's pattern slows down
with hole intensity to evoke time dilation, but never freezes (clamped to
`DILATION_MIN`).

---

## Performance notes

The cost concentrates where it matters: pixels inside `~bmax = rout + 3`
shadow radii pay `N_STEPS = 48` leapfrog steps each. On a 4K display with
the hole at 55 % screen height, that's a few million expensive pixels per
frame — still 60 fps on midrange discrete GPUs, but if it stutters:

- Lower `N_STEPS` (top of `src/shader.wgsl`) — 32 is barely distinguishable.
- Cap `radius_max` in `config.toml` (e.g. 0.40 instead of 0.55).
- Run on an external monitor at 1440p; the cost is per-pixel.

The screen-space swirl is essentially free (a couple of sines per pixel).

---

## Limitations

- **Windows only.** The screensaver subsystem and `BitBlt` capture are
  Windows-specific. Cross-platform would need an entirely different shell
  integration.
- **Multi-monitor: one canvas across all displays.** The saver window
  spans the full virtual desktop (every physical screen's pixels in their
  Windows-arranged positions), and the black hole drifts freely across
  that whole rectangle. On asymmetric layouts (e.g. 4K + 1080p) the
  smaller monitor leaves an unrendered "void" in the virtual rect; if the
  hole wanders into that region it will appear to vanish until it drifts
  back over a physical display. Per-monitor saver windows are
  intentionally not implemented.
- **Mixed-DPI setups.** Captures and rendering happen in physical pixels
  via per-monitor DPI awareness, so a 4K @ 200% next to a 1080p @ 100%
  works correctly. If the virtual desktop is wider/taller than
  `max_texture_dimension_2d` (typically 16384–32768 on modern GPUs) the
  snapshot is downscaled with a nearest-neighbor filter before upload;
  the lensing distortion hides the slight loss.
- **First saver launch after boot may flicker once** while shared memory
  catches up if the daemon is slower to start than the user is to walk
  away from the keyboard.
- **Snapshot is up to 5 seconds stale.** Frequent enough for "shows your
  desktop"; not real-time. Reducing the capture interval is a one-line
  change in `src/daemon.rs` (`CAPTURE_INTERVAL`).

---

## Troubleshooting

If `.scr` or `blackhole.exe /s` crashes or misbehaves, check the log:

```
%TEMP%\blackhole-screensaver.log
```

It contains adapter limits, the virtual-desktop rect, captured snapshot
sizes, and any panic location + message. The release build runs as a
Windows-subsystem app (no console), so this file is the only signal you
get. It rotates automatically once it exceeds 1 MiB.

When the daemon's behavior changes between builds, remember it has to be
restarted to pick up the new binary — quit it from the tray icon and
double-click `blackhole.exe` (or invoke the configure dialog) to respawn
the new version.

---

## Project layout

```
src/
├── main.rs              entry, dispatches on /s //p //c /--daemon
├── mode.rs              argument parsing, configure dialog
├── app.rs               winit ApplicationHandler, input-to-exit
├── renderer.rs          wgpu device + pipeline + uniform buffer
├── shader.wgsl          the actual black hole
├── animator.rs          grow-and-reset cycle + Lissajous drift
├── config.rs            optional config.toml
├── desktop.rs           BitBlt + shared-memory read
├── desktop_layout.rs    virtual-desktop bounds + DPI awareness
├── preview.rs           /p HWND child window reparenting
├── daemon.rs            background daemon main loop + tray
├── shared_mem.rs        named shared memory layout
├── autostart.rs         HKCU\...\Run registry entry
├── fullscreen_check.rs  foreground-window monitor-bounds test
└── logging.rs           file logger + panic hook (%TEMP%\...log)
```

---

## License

[MIT](LICENSE). The physics derivation and many parameter defaults come
from [ghostty-blackhole](https://github.com/s0xDk/ghostty-blackhole) (also
MIT); no code from it is linked in.
