# Blackhole Screensaver

A Schwarzschild black-hole screensaver for Windows, ported in spirit from
[ghostty-blackhole](https://github.com/s13k/ghostty-blackhole). Pure Rust +
wgpu, no C++.

When idle, your desktop becomes the sky around a wandering black hole:
real null-geodesic integration lenses your wallpaper into the photon ring,
a Shakura-Sunyaev accretion disk orbits with Doppler beaming, and a
screen-space wind swirls just outside the shadow.

## Build (on Windows)

Prereqs:
- `rustup` with the `stable-x86_64-pc-windows-msvc` toolchain
- Visual Studio Build Tools (only the **MSVC linker** + **Windows 10/11 SDK**
  components -- no C++ to write)

```powershell
cargo build --release
```

The binary lands at `target\release\blackhole.exe`.

## Install as a screensaver

1. Rename `blackhole.exe` to `blackhole.scr`.
2. Either:
   - Copy `blackhole.scr` into `C:\Windows\System32\`, **or**
   - Right-click the file and choose **Install**.
3. Open **Settings -> Personalization -> Lock screen -> Screen saver**,
   pick *blackhole*, set an idle timeout, click *Preview* to test.

## Command-line interface

Windows invokes the `.scr` with these arguments; you do not run them by hand.

| Args        | Mode      | Behavior                                          |
|-------------|-----------|---------------------------------------------------|
| `/s`        | Saver     | Fullscreen render, any input quits                |
| `/p <HWND>` | Preview   | Renders inside the settings panel preview pane    |
| `/c`        | Configure | Shows an info dialog with the config file path    |
| *(none)*    | Configure | Same as `/c`                                      |

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

To inspect the path on your machine, run `blackhole.exe` with no arguments --
the dialog prints it.

## What you see

- **Shadow**: rays with impact parameter below the critical value spiral
  through the horizon; the wallpaper behind the hole really is gone, not
  just dimmed.
- **Gravitational lensing**: escaped rays project back onto the desktop
  "sky" plane. Icons and wallpaper bend and double inside the Einstein ring.
- **Photon ring**: a thin bright ring at 1.5 r_s emerges from rays that
  wind around the hole and escape.
- **Accretion disk**: a thin Keplerian disk pierced multiple times per
  ray, Shakura-Sunyaev temperature profile rendered as blackbody color,
  with relativistic Doppler + beaming -- the side moving toward you is
  blue-hot and boosted, the receding side dim and red.
- **Screen-space swirl**: a warm halo just outside the shadow that
  circulates against the wallpaper so the orbit reads at any size.
- **Lensed starfield**: faint procedural stars sampled with the *bent*
  ray, so they smear into arcs around the hole.
- **Time dilation**: the disk's pattern winds down as the hole grows
  heavier (sqrt(1 - 1.5/r) factor).

## How it works

- **Background**: at startup, one `BitBlt` of the virtual desktop into an
  sRGB texture. No per-frame capture, no feedback loops.
- **Lensing**: per-pixel leapfrog integration of the Schwarzschild Binet
  acceleration `a = -(3/2) h^2 x / r^5`. Only pixels with impact parameter
  below `rout + 3` pay the integration cost; the rest go through an
  analytic weak-field deflection fitted against the integrator.
- **Disk**: thin Keplerian sheet pierced per step; emission integrated
  HDR, then tonemapped on top of the lensed wallpaper sample.
- **Animation**: the CPU drives shadow size and Lissajous position
  through a uniform buffer; everything else lives in WGSL.

## Roadmap

- [x] M1 -- project skeleton, winit fullscreen, screensaver arg parsing
- [x] M2 -- wgpu pipeline + placeholder shader
- [x] M2.5 -- one-shot desktop snapshot as background texture
- [x] M3 -- port `blackhole.glsl` to WGSL (geodesic integration, disk, lensing)
- [x] M3.5 -- screen-space swirl so the orbit reads at any size
- [x] M4 -- slow grow-and-reset cycle + Lissajous drift from CPU
- [x] M5 -- `/p` preview HWND child window
- [x] M6 -- `config.toml` for the user-facing tunables

## License

MIT. The original [ghostty-blackhole](https://github.com/s13k/ghostty-blackhole)
shader is also MIT; no code from it is linked in, but the physics derivation
and many parameter defaults are its direct legacy.
