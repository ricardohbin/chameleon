# Ghore, The Chameleon

A 2D action platformer about a small, bone-white, evil-looking chameleon wandering a post-war wasteland,
searching for the colors that war and humans destroyed.

The world starts **grayscale**. Colors are **earned back**: every giant insect boss you kill gives the
chameleon a color and a power, and the color spreads through the world around its lair (vines, water, stone,
flowers, fumes). The tone is apocalyptic and gory. You slash enemies open and get painted with their blood.

Built in Rust with [Bevy](https://bevy.org) 0.20. All art is generated in code. The only asset files are the
world sketch, the sound effects (recorded by the author) and a font.

---

# Playing

## What you do

- Explore a huge hand-drawn map made of regions: a trash dump, a ruined park, sewers, a subway, catacombs, a
  mall, a water treatment plant, a sinkhole, a broken dam, a military bunker and a cave system.
- Each region is the home of a **giant insect boss**, and the smaller creatures living there are its ecosystem.
  Bosses sleep in their lairs until you enter their arena. They introduce themselves, commit attacks to where
  you stood (moving dodges), and answer back if you hit them too many times in a row.
- Kill a boss to earn a **color** (it stays on the chameleon's skin) and a **power**.
- Eat what you kill to heal. Boss carcasses also raise your maximum health.

## Powers

| Boss | Power |
|---|---|
| The Gutter King (cockroach) | Dash |
| The Venom Mother (wasp) | Venom spit |
| The Razorwing (dragonfly) | Air spin |
| The Sawleg (grasshopper) | Big jump |
| The Plague Frog | Water walk: water no longer slows, tires or drowns you |
| The Weaver (spider) | Wall walking |
| The BattleTank Beetle (final boss) | Shield: 2 seconds untouchable, long cooldown |

Powers show up in the controls hint only once you have earned them.

## Controls

The game is set up for a mouse and keyboard by default. The control scheme can be changed in the menu
(Mouse + keyboard, Directional, or Dual, where the arrow keys aim) and a gamepad works too.

| Action | Keys |
|---|---|
| Walk | `A` `D` or arrow keys |
| Jump (hold for higher) | `Space` / `W` / `Up` |
| Slash (hold to keep slashing) | `F` / `J` / left mouse button, toward the mouse |
| Eat remains | `E` |
| Burrow through rubble (aim at it) | `C` |
| Dash (on firm ground only; needs the full strength meter and drains all of it, like swimming) | `Shift` / `K` |
| Venom spit (lobbed like a grenade; each spit costs half the strength meter) | `Q` / right mouse button |
| Shield | `G` |
| Air spin | attack while in the air |
| Big jump | hold `S` / `Down` on the ground, then let go |
| Wall walk | push into a steep surface (or stand on one that would slide you) to grip it, then `A` `D` `W` `S` walk along it, `Space` leaps off. Works on any surface short of a ceiling, with no sliding; costs the strength bar (a full bar is ~5 floors of vertical wall, less on gentler slopes, overhangs wear it out much faster). At zero the grip lets go and only a full bar grips again |
| Power pill (take one you carry; see below) | `Z` |
| Pause | `P` / `Pause` |
| Menu | `Esc` / `Tab` / `M` |
| Quick save / quick load | `F5` / `F9` |
| Restart | `F8` |
| Brightness contrast | `F6` / `F7` |
| Screenshot (lossless WebP in `screenshots/`) | `F12` |
| Full map export | `F10` |

**Menu** (`Esc`, `Tab` or `M`, or a gamepad's Start): resume, load the last checkpoint, start a new game, settings (difficulty,
volume, contrast, control scheme) and exit.

**Gamepad:** left stick or d-pad moves, right stick aims, South jumps, West / right trigger slashes, East spits, left
trigger raises the shield, North eats, left bumper dashes.

**Where things are saved:** the quick save (`ghore.save`) and the settings (`ghore.settings`) are small text
files next to the executable.

## Difficulty

Four steps: Easy, Normal, Tough, Hard. The bosses were tuned on Normal.

---

# Development

## Requirements

- A recent stable **Rust** toolchain (edition 2024).
- A GPU with Vulkan, DirectX 12 or Metal support.
- **Disk space:** `target/` takes about 4–5 GB and the first build can fill a small disk. Keep about 8 GB free.
- **Windows (GNU toolchain):** the project is developed with Rust from **msys2 UCRT64**. Put
  `C:\msys64\ucrt64\bin` on the `PATH` (in Git Bash: `export PATH="/c/msys64/ucrt64/bin:$PATH"`).
  [`.cargo/config.toml`](.cargo/config.toml) links with `lld`, which ships with that toolchain, and
  [`build.rs`](build.rs) embeds the icon with `windres` when it is available (otherwise the exe just keeps the
  default icon).
- If the game is running, `target/debug/ghore.exe` is locked and the build "fails" at the copy step. Close the
  game, or run the fresh `target/debug/deps/ghore-*.exe` instead.

Dependencies (see [`Cargo.toml`](Cargo.toml)): `bevy` 0.20 (with the `aac` and `mp4` features for the `.m4a`
sound effects), `miniz_oxide` (compressed sketch bake and PNG encoding), `image` (PNG only), `winit` and `webp`
(screenshots and map export).

## Build and run

```sh
cargo run              # dev build
cargo run --release    # fully optimized, slow first build (recompiles Bevy: a few minutes)
```

The dev profile optimizes the dependencies (Bevy) at `opt-level = 3` and compiles the game's own code lightly,
so the first build is slow, but later builds only recompile the game and run fast.

Release builds embed `assets/world.baked` and the sound effects in the executable. Debug builds read the world
from disk instead (see below).

### Web (WebAssembly)

```sh
web/build.sh           # optimized build → web/dist (index.html + ghore.js + ghore_bg.wasm.br, Brotli-compressed) and web/ghore-web.zip (for itch.io); 20–25 minutes
web/build.sh --small   # smallest download (opt-level s), a bit slower to run
web/build.sh --dev     # faster to build, much bigger and slower to run
python web/serve.py                    # serves web/dist like itch.io does (.br with Content-Encoding), then open http://localhost:8000
```

One-time setup: the `wasm32-unknown-unknown` target and `wasm-bindgen-cli` at the same version as `wasm-bindgen` in
`Cargo.lock` (`cargo install wasm-bindgen-cli --version <that>`); `wasm-opt` (binaryen) and `brotli` are used if found.
The web build renders with WebGL2, keeps saves and settings in the browser's `localStorage` (`src/storage.rs`)
instead of files next to the executable, and leaves out the native-only dev tools (F10/F12 exports, `cargo run -- bake`,
hot reload, window icon, mouse confinement). The files must be served over HTTP (not opened from disk) and the game
embeds all its assets, so `web/dist` can be hosted as is on any static host that serves `.br` files with `Content-Encoding: br`
(itch.io does: upload `web/ghore-web.zip`; `web/serve.py` does for local tests). The page shows a loading screen with the download
progress and tells compiling and building the world apart; the game sends a `ghore-ready` event to the page when the world is built.

## Power pills

Prismatic vermin crawl slowly on some walls. Touch one to pocket it (icons over the health, up to 12). `Z` swallows one:
about 30 seconds of being faster, jumping higher and, above all, much stronger (the strength meter that wall walking,
swimming, dashing and spitting spend drains far slower and refills faster; a dash needs only a bit of it). Meanwhile the
health pips shimmer in every color, fewer of them as it wears off. Take as many as you dare, it is a gamble: each
extra dose adds less than the one before, and from the second on the controls get more and more chaotic (what you
press is briefly flipped, the body moves on its own, now and then it jumps by itself), until around six doses it is
almost unplayable. When it ends comes the crash: exactly as much worse as the peak was better, for a fifth of the time
spent high at one dose, and disproportionately longer the more you took (about 6 s for one, 17 s for two, 31 s for three,
over a minute for five). The pips go ash-gray and sag meanwhile. No pill can be taken during the crash. Pills are precious: picking one up and swallowing one both write a checkpoint, and saves remember how many you carry and which ones
were already taken from the map and whether you are high or crashing, with the time left.

## The world is a sketch

The whole map is one Aseprite file, [`assets/world.aseprite`](assets/world.aseprite) (4096×2048, RGBA). It holds
the layout only. The game reads the **colors** of the pixels, not the layer names:

| Color | Meaning |
|---|---|
| White | solid ground |
| Dark gray (Multiply layer over white) | passable structure / interiors |
| Black | air |
| Magenta cross | world origin (0, 0) |
| Red | player spawn |
| Pure blue `#0000ff` | a power pill: an iridescent vermin crawling slowly around the spot (the tile under it is a normal wall) |
| Pale green `#bdffc2` | a health pill bug: a pulsating red vermin (red even in a colorless world) that is eaten on touch for one health tick, only when health is below max (the tile under it is a normal wall) |
| Yellow, green, dusty blue `#7f9fd6`, dusty pink `#d1b0c6`, brown `#8f563b`, violet `#b800ff` | boss markers: Gutter King, Venom Mother, Weaver, Razorwing, Sawleg, Plague Frog (the last four are matched exactly) |

**Slices** name the areas. `region: …` is an area, which gets an art style from keywords in its name and is
the lair of the boss whose marker is inside it. `feature: …` is a special spot described in words:
"slippery" / "sliding" / "one way down" make slopes slide-only, "fall apart" / "crumbl" shed junk, "dash" gaps
need the dash, and names with sludge / flood / sewage / water / pool / lake fill with water.

Scale: 1 sketch pixel = 8 world units. A 1 px rise is a ramp, 2 px is a stair that is climbed automatically, and
3–10 px needs a jump (the max jump is about 17 px high and 22 px across).

`cargo run -- mark <boss> <x,y>` paints a boss marker into the sketch on a new layer (it backs the file up
first). Bosses: spider, centipede, frog, grasshopper, dragonfly, wasp, roach.

### Hot reload

In debug builds the game polls `assets/world.aseprite` every 0.5 s. When the file changes (and holds still for
one poll) the world is rebuilt in place: terrain, props, bosses and mobs. If Aseprite has the file locked or
half-written, the reload waits and retries. `F8` restarts and re-reads the sketch from disk too. A boss marker
that moved restarts its fight. So the usual loop is to edit in Aseprite, save, and look at the running game.

### The review loop: diff and bake

The sketch only gives the layout. The look and the content (art styles, props, parallax, mob placement, pinned
moving pieces) are interpreted from it in code. To keep track of what has already been interpreted, the project
keeps a **baked** copy, [`assets/world.baked`](assets/world.baked): the sketch as it was when its changes were
last handled. Release builds embed this file, not the `.aseprite`.

```sh
cargo run -- diff           # what changed in the sketch since the last bake → report + screenshots/world-diff.png
cargo run -- diff <a> <b>   # compare any two sketches (.aseprite or .baked)
cargo run -- bake           # world.aseprite → world.baked
```

1. Edit `world.aseprite`. Debug builds print `world sketch: not interpreted yet (…)` at startup and on reload
   while it differs from the bake.
2. Run `cargo run -- diff` to see the changes: regions of changed terrain, moved or added markers, and
   resized, added or removed slices. The diff aligns both versions on the origin marker, so resizing or
   extending the canvas is not a change.
3. **Interpret** the changes: adjust styles/keywords, pins, props and mob placement, spawn any new boss, and
   check that nothing is now unreachable or buried.
4. Run `cargo run -- bake` to record the sketch as handled. The warning goes away, and release builds ship the
   new world.

Baking without step 3 silences the warning but ships a world the code hasn't been adjusted for.

## Dev and debug tools

| Key / variable | What it does |
|---|---|
| `F1` | debug overlay: positions, movement, ground, region, boss state, plus gizmos (player box, velocity, foot samples, regions, platforms, paths) |
| `F12` with `F1` on | also writes `screenshots/shot-*.txt` with the panel text |
| `F2` | teleport to the next boss's lair |
| `F3` | invulnerability (any blow kills a boss) |
| `F4` | flight (jump = impulse toward the mouse); small wings flap while flying, and burrow (`C`) works in flight. After the ending's credits you get control back flying and invulnerable (the key hints are hidden during the ending show) |
| `F10` | full-map WebP export (painted, or annotated with `F1`) |
| `TOUR=1 cargo run` | visits the spawn, bosses, every slice and pin, then an overview → `screenshots/tour-*.webp`, then quits |
| `START=x,y EXIT_AFTER=4 cargo run` | drops the chameleon at a world spot, logs where it ends up (+ `screenshots/exit.webp`), quits |
| `HOLD=d TAP=space@1.2,k@1.3` | simulated input for those tests |
| `AIM=-200,150` | fake mouse offset from the chameleon |
| `GREEN=1`, `YELLOW=1`, `PINK=1`, `BLUE=1`, `BROWN=1`, `VIOLET=1` | start with that boss already beaten |

`screenshots/` is git-ignored.

## Performance diagnostics

### Frame-drop watchdog (always on in dev builds)

[`src/perf.rs`](src/perf.rs) is compiled only with `debug_assertions` (so not in `--release`). When a frame is
over 25 ms and about 1.8x the recent average, it logs a `WARN` with where the time went and a few counters:

```text
frame drop: 568.9ms (avg 19.1ms) | render-wait/First 2.5 PreUpdate 0.4 FixedLoop 0.1 Update 552.8 PostUpdate 13.1 Last 0.1
  entities 125487 (+18777 since last drop) sprites 214 mesh2d 95129 | image assets 258 (+0) mesh assets 72015 (+9333)
```

Every 5 s, if anything dropped, it also prints the average fps and the worst frame. The first 60 frames are ignored.

| What stands out | What it usually means |
|---|---|
| `render-wait/First` large | the main thread waited on the render thread, GPU or vsync: too many draw calls or overdraw, pipeline compilation, asset uploads |
| `Update` / `PostUpdate` large | game logic, or transform and visibility propagation |
| `FixedLoop` large | fixed-timestep updates catching up |
| entities jump between drops | per-frame spawning |
| image / mesh assets keep growing | textures or meshes regenerated every frame, or leaked |

Settings: `PERF_MS=40` sets the threshold in ms (default 25); `PERF=0` turns it off.

### Finding the slow system

The watchdog says which phase is slow, not which system. For that, record a per-system trace:

```bash
cargo run --features bevy/trace_chrome    # play until the lag shows up, then quit normally
python tools/perf_trace.py trace-*.json   # optional second argument: minimum ms (default 20)
```

[`tools/perf_trace.py`](tools/perf_trace.py) prints the spans whose slowest run is over the threshold: worst run,
how many runs were slow, run count and total time. The raw `trace-*.json` also opens in
[ui.perfetto.dev](https://ui.perfetto.dev) for a timeline view. `bevy/trace_tracy` works too, with the Tracy
profiler.

Tracing slows the game and the file grows fast, so keep the session short. Compare against a release run
(`cargo run --release`) to tell code that is slow from a dev build that is slow. The `trace-*.json` files are
local output and shouldn't be committed.

## Code layout

`src/` is one module per system or creature. The main ones:

| File | What it does |
|---|---|
| `sketch.rs`, `sketch_tool.rs` | Aseprite reader and the baked format; `bake` / `diff` / `mark` CLI |
| `level.rs`, `level/loose.rs` | loads the sketch, hot reload, moving and crumbling platforms |
| `terrain.rs`, `terrain_art.rs`, `terrain_props.rs`, `outline.rs` | the collision grid and the generated art and decoration, painted progressively on the async task pool |
| `player.rs`, `chameleon.rs` | controller and the procedurally animated rig |
| `boss.rs` + one file per boss | shared boss machinery and each boss's AI |
| `mobs.rs`, `mobs/`, `mob_art.rs` | ordinary enemies, per region |
| `regrowth.rs`, `fumes.rs`, `terrain_props/moss.rs` | colors returning to the world |
| `menu.rs`, `settings.rs`, `controls.rs`, `save.rs`, `hud.rs` | menu, settings, control schemes and gamepad, saves, HUD |
| `camera.rs`, `atmosphere.rs`, `water.rs`, `gore.rs`, `sound.rs` | camera and grayscale, backdrop and weather, water, blood, audio |

[`MEMORY.md`](MEMORY.md) holds the detailed notes: a full code map, design decisions, tuning values and Bevy
0.20 API notes learned while building.

Adding a boss: a new module with `pub fn spawn(.., &Arena)` that inserts `Boss::new(&SPEC, ..)` and its AI
component, a plugin that reacts to `BossHit` and resets on `Respawned`, and a `BossKind` entry in `boss.rs`.

Every new ground material / `Style` must declare its `softness()` in `player.rs` (it's an exhaustive match).

## Tools

- [`tools/font/make_font.py`](tools/font/make_font.py) builds `assets/fonts/GhoreSerif.ttf`, the display serif
  (`pip install fonttools shapely`, then `python tools/font/make_font.py`).
- [`tools/blender/`](tools/blender): Blender scripts for the chameleon model and walk cycle
  (`blender -b -P make_chameleon.py`, `blender -b -P make_chameleon_walk.py`).

## Known quirks

- A Vulkan loader may print `Failed to open JSON file ...` errors at startup if the registry still lists layers
  from an uninstalled Vulkan SDK. They are harmless. Reinstall the SDK or remove the stale entries under
  `HKLM\SOFTWARE\Khronos\Vulkan\ExplicitLayers`.
- On macOS, screenshots come out black when the game window is hidden or on another Space.
