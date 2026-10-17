<div align="center">

# ⚙️ oxide-core

**A fast-paced low-poly top-down 3D action prototype, written in Rust with Bevy 0.15.**

*Snappy movement inspired by the momentum of ULTRAKILL, wrapped in a retro low-poly look.*

![Rust](https://img.shields.io/badge/Rust-2024_edition-orange?logo=rust)
![Bevy](https://img.shields.io/badge/Bevy-0.15-232326?logo=bevy)
![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)

</div>

---

## ✨ Features

- **Responsive movement**: very high acceleration and even higher friction. The player stops dead the moment you let go, with no sliding.
- **Dash**: a short burst of speed in the direction you are moving, with a cooldown, a glowing afterimage trail and spark particles.
- **Cursor raycast aiming**: a ray is cast from the camera through the mouse cursor onto a ground plane (`Camera::viewport_to_world`), and the player always turns to face that point.
- **Shooting**: glowing projectiles with a limited lifetime, a muzzle flash and spark bursts when they hit an obstacle.
- **Procedural VFX**: HDR + bloom, gizmo laser sight and crosshair, a dash-ready ring under the player, and particles that need no external assets.
- **Smooth camera**: an isometric-like top-down camera that follows the player with frame-rate-independent smoothing.
- **Minimal dependencies**: the only dependency is `bevy`. Physics, collisions and particles are all hand-written in a single `main.rs`.

## 🎮 Controls

| Input              | Action        |
|--------------------|---------------|
| `W` `A` `S` `D`    | Move          |
| `Space`            | Dash          |
| Mouse              | Aim           |
| Left Mouse Button  | Shoot (hold for auto-fire) |

## 🚀 Build & Run

**Requirements:** [Rust](https://rustup.rs/) (stable, with 2024 edition support) and a GPU with Vulkan / DX12 / Metal support.
On Linux, also install [Bevy's system dependencies](https://github.com/bevyengine/bevy/blob/main/docs/linux_dependencies.md).

```bash
git clone https://github.com/augustuz/oxide-core.git
cd oxide-core
cargo run
```

> [!TIP]
> The first build compiles the whole Bevy engine and can take several minutes. Later builds are incremental and fast.
> The dev profile in `Cargo.toml` already optimizes dependencies. For the best performance, run a release build:
>
> ```bash
> cargo run --release
> ```

## 🧪 Quality Checks & Logging (Windows)

The repository includes a helper script, [`check_and_log.bat`](check_and_log.bat), that runs every static check and saves the full output to files:

```bat
check_and_log.bat        :: fmt check + cargo check + clippy (pedantic)
check_and_log.bat run    :: same, then launches the game and records its runtime log
```

The script creates a local `logs/` folder if it does not exist yet. That folder is git-ignored.

| Step | Command | Report |
|------|---------|--------|
| 1 | `cargo fmt -- --check` | `logs/fmt_check.log` |
| 2 | `cargo check` | `logs/cargo_check.log` |
| 3 | `cargo clippy -- -W clippy::pedantic` | `logs/clippy_analysis.log` |
| 4 *(with `run`)* | `cargo run` with `RUST_LOG=oxide_core=debug,bevy=warn,wgpu=error` and `RUST_BACKTRACE=1` | `logs/runtime.log` |

The console shows a status line for each step. The script exits with code `1` if any step fails, so you can also use it in CI.

## 🛠️ Tuning

All gameplay values are `const` items at the top of [`src/main.rs`](src/main.rs). These include movement speed, acceleration and friction, dash speed, duration and cooldown, projectile speed and lifetime, camera offset and smoothing, the color palette and the obstacle layout.

## 📁 Project Structure

```
oxide-core/
├── src/main.rs          # the whole game: components, resources, systems
├── Cargo.toml           # single dependency: bevy 0.15
├── check_and_log.bat    # quality checks + log collection
├── LICENSE
└── README.md
```

## 📜 License

Distributed under the [MIT License](LICENSE). © 2026 augustuz.
