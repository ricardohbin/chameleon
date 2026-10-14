# Camaleon — Project Memory

A 2D platformer prototype in **Rust + Bevy 0.20**. All art is generated in code (no image assets).

## Vision / story

A small, evil-looking, bone-white **chameleon** wanders a post-war wasteland **searching for the
colors that war and humans destroyed**. The world starts **grayscale**; colors are meant to be
**earned back** as the game progresses. Tone: apocalyptic, chaotic, unstable — *not cute*.

## Running

```sh
cargo run            # dev build (Bevy deps are optimized, see Cargo.toml profiles)
cargo run --release  # fully optimized (first build recompiles Bevy: a few minutes, extra GBs)
```

Controls: `A/D` or arrows = walk · `Space/W/Up` = jump (hold for higher) · `F` (or `J`) = tongue ·
`F12` = screenshot to `screenshots/`.

⚠️ `target/` takes ~4–5 GB. The first build once filled the disk (ENOSPC) — keep ~8 GB free.

## Code map (`src/`)

| File | What it does |
|---|---|
| `main.rs` | App setup, window (1280×720), plugin registration, HUD text. |
| `shapes.rs` | Vector-art toolkit: Catmull-Rom curves (`smooth_closed/open`), `fill` (triangle fan from centroid), `stroke` (ribbons with width/shade/pattern), `triangles`, `gradient_rect`, `ellipse_pts`, `rock_pts`, `scale_color`, and a seeded xorshift `Rng`. |
| `chameleon.rs` | The character rig, built from meshes in a bone hierarchy (`Part` enum: Rig, Body, Head, Leg{Hip/Knee/Foot}, EyeCone, Pupil, TongueBody, TongueTip). Procedural animation: walk cycle, body bob/breathing, curling tail (mesh regenerated every frame), darting eye, tongue shot, camouflage. |
| `player.rs` | Physics/controller: accel, gravity, coyote time, jump buffering, variable jump height, AABB collision, one-way platforms, riding moving platforms (`Platform.delta`), respawn at last stable ground. `SCALE` (0.7) sizes the chameleon and its collider. |
| `level.rs` | Seeded procedural level (`SEED`, `LEVEL_END` = 7000): deep rubble/asphalt ground chunks with gaps, two tiers of floating debris, a wall at the end. Platform visuals: cracks, rebar, strata, debris, lamp posts, barbed wire, lit top rims. `Platform` component. `lost_colors` module keeps the original bright palette for later. |
| `unstable.rs` | `Motion` component: `Crumble` (shakes 0.6 s → falls → respawns after 3 s), `Swing` (girder on chains), `Sink` (sinks under weight). `Chain` visuals. `Tremor` earthquakes every 9–16 s (screen shake, stronger swings, random slab collapse). |
| `atmosphere.rs` | Backdrop with `Parallax {origin, factor: Vec2}` (0 = world, 1 = glued to screen, negative = foreground): sky gradient, hazy sun, 3 ruined skylines (windows, twisted beams), pylons/cables/cranes, fog banks, foreground wreckage, smoke plumes over flickering fires (`Puff`, `Fire`), ash + embers (`Particle`, wrap around camera), lightning (flash overlay + regenerated bolt mesh). |
| `camera.rs` | Smooth follow with look-ahead, zoom 0.65, trauma-based `Shake` resource (`add`, `at_least`), and **global grayscale** via `ColorGrading.post_saturation = WORLD_SATURATION (0.0)`. |
| `dev.rs` | F12 screenshot. |

## Key design decisions

- **Grayscale is a camera setting, not data.** `WORLD_SATURATION` in `camera.rs` (0 = gray, 1 = full
  color). All colors in code stay intact so they can be "earned back" (e.g. animate saturation, or
  later move to per-object color).
- **Camouflage is gated:** `UnlockedColors` resource (in `chameleon.rs`) starts empty. The chameleon only
  blends toward a platform's `color` if that color is unlocked
  (`unlocked.0.push(level::lost_colors::PINK)`). Blending is done in Oklab.
- **Skin tinting:** meshes carry grayscale vertex colors (shading); `ColorMaterial` multiplies them, so
  the 4 skin materials (`base/shadow/light/accent`, derived by `palette()`) can be re-tinted at runtime.
- **Chameleon local coords:** faces +X, origin = collider center; the rig is flipped by scaling X
  (smoothly, reads as a turn) and scaled by `player::SCALE`.
- **System order:** `unstable::move_platforms` → `player::move_player` → chameleon animation / camera
  follow; `parallax` runs in `PostUpdate` before transform propagation.
- Z layers: sky −100 … skylines −95/−90/−70 … smoke −55 … platforms 0 … player 10 … ash −10/30 …
  bottom fog 35 … foreground wreckage 45 … lightning flash 80.

## Bevy 0.20 API notes (learned while building)

- `TextFont { font_size: FontSize::Px(18.0), .. }` (not a plain f32).
- `PrimitiveTopology` from `bevy::render::render_resource`; `Indices` from `bevy::mesh`.
- `AlphaMode2d` from `bevy::sprite_render`; `ColorGrading`/`ColorGradingGlobal` from `bevy::render::view`.
- Parenting via `ChildOf(parent)`; `Single<...>` queries; `MessageWriter<AppExit>`.
- Screenshots: `commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path))`.
- Mesh2d has no backface culling, so negative scale (flipping) is fine.
- The default font has no arrow glyphs (←/→ render as boxes).

## Workflow tip

To check visuals without playing, temporarily add a `PreUpdate` system (after
`bevy::input::InputSystems`) that presses keys via `ResMut<ButtonInput<KeyCode>>`, takes a
`Screenshot` at frame N, then writes `AppExit`. Remove it afterwards.

## History

1. Basic setup: green chameleon, jungle level, colorful platforms, camouflage.
2. Chameleon made white; world turned grayscale via color grading; `UnlockedColors` gate.
3. Big redesign: smaller/evil chameleon, apocalyptic ruins, dense procedural level, unstable
   platforms, earthquakes, smoke/ash/embers/lightning, screen shake.

## Ideas / next steps

- Collectible fragments of lost colors (unlock camouflage + raise world saturation).
- Enemies/hunters (camouflage as stealth), tongue catching things.
- Hand-made level sections, checkpoints, a real ending past the final wall.
- Not yet visually verified in screenshots: lightning bolt, earthquake, a slab mid-fall.
- Git repo initialized; nothing committed yet.
