# Blackhole Screensaver v0.1.0

First public release. A Schwarzschild black hole, rendered on the GPU, that slowly devours your desktop.

## What it does

- Real-time wgpu fragment shader: photon sphere, Einstein ring, gravitational lensing of a starfield background.
- The shadow grows from a pinprick to nearly full-screen on a slow cycle (default 5 minutes), then resets — so leaving the machine for a while is rewarded with a noticeably different scene.
- Lissajous drift keeps the singularity wandering instead of sitting dead-center.
- Multi-monitor aware: spawns one window per display, captures each from the correct virtual-desktop origin, exits on the first input on any screen.
- Ships as both a regular `.exe` and a Windows `.scr` screensaver — same binary, behavior selected by command-line flag (`/s`, `/p`, `/c`).

## Install (Windows)

1. Download `BlackholeScreensaver-v0.1.0-x86_64-windows.zip` from the assets below.
2. Unzip. You get `blackhole.exe`, `BlackholeScreensaver.scr`, an example `config.toml`, and the README.
3. Right-click `BlackholeScreensaver.scr` → **Install**. Windows opens the Screen Saver Settings dialog with it preselected.
4. (Optional) **Settings…** in that dialog opens the config-file path; copy `config.toml` to `%APPDATA%\BlackholeScreensaver\config.toml` to tweak it.

Or just double-click `blackhole.exe` to run it as a regular fullscreen app — move the mouse or press any key to exit.

## Configuration

All knobs are optional. The defaults are tuned for a slow, ambient feel. See `config.toml` in the zip for the full list with comments — the big ones are:

| Field             | Default | Meaning                                                  |
| ----------------- | ------- | -------------------------------------------------------- |
| `cycle_seconds`   | `300.0` | Seconds for the shadow to grow from min to max.          |
| `radius_min/max`  | `0.05` / `0.55` | Shadow radius bounds, as a fraction of screen height. |
| `drift_amp_x/y`   | `0.30` / `0.20` | Lissajous drift amplitude (uv units).            |
| `drift_speed_x/y` | `0.013` / `0.017` | Drift frequencies (Hz). Keep small.            |

Bad TOML logs a warning and falls back to defaults — a screensaver should never refuse to start because of a typo.

## Build from source

```sh
cargo build --release
```

Requires a recent Rust toolchain and a GPU that speaks Vulkan / D3D12 / Metal (wgpu picks the best backend). Tested on Windows 10/11 with both Intel iGPU and discrete NVIDIA.

## Verifying the download

A SHA-256 of the zip is published alongside it as `BlackholeScreensaver-v0.1.0-x86_64-windows.zip.sha256.txt`.

## License

MIT.
