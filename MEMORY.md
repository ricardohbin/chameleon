# Camaleon — Project Memory

A 2D platformer prototype in **Rust + Bevy 0.20**. All art is generated in code (no image assets).

## Vision / story

A small, evil-looking, bone-white **chameleon** wanders a post-war wasteland **searching for the
colors that war and humans destroyed**. The world starts **grayscale**; colors are meant to be
**earned back** as the game progresses. Tone: apocalyptic, chaotic, unstable — *not cute*. It's **gory**: the chameleon slashes enemies
open and gets painted with their blood (the first color in the gray world is blood).

## Running

```sh
cargo run            # dev build (Bevy deps are optimized, see Cargo.toml profiles)
cargo run --release  # fully optimized (first build recompiles Bevy: a few minutes, extra GBs)
```

Controls: `A/D` or arrows = walk · `Space/W/Up` = jump (hold for higher) · `F` / `J` / left mouse button = claw slash · `F2` = next boss ·
`F12` = screenshot to `screenshots/`.

⚠️ `target/` takes ~4–5 GB. The first build once filled the disk (ENOSPC) — keep ~8 GB free.

## Code map (`src/`)

| File | What it does |
|---|---|
| `main.rs` | App setup, window (1280×720), plugin registration, HUD text. |
| `shapes.rs` | Vector-art toolkit: Catmull-Rom curves (`smooth_closed/open`), `fill` (triangle fan from centroid), `stroke` (ribbons with width/shade/pattern), `triangles`, `gradient_rect`, `ellipse_pts`, `rock_pts`, `scale_color`, and a seeded xorshift `Rng`. |
| `chameleon.rs` | The character rig, built from meshes in a bone hierarchy (`Part` enum: Rig, Body, Head, Leg{Hip/Knee/Foot}, EyeCone, Pupil). Procedural animation: walk cycle, body bob/breathing, curling tail (mesh regenerated every frame), darting eye, claw slash (near front leg, `Leg{swipe}`; `slash_pose`, faint crescent `SlashWave`), hurt blink, camouflage. `ChameleonAnim::striking()` + `slash_id` drive hits. Death pose (rolls belly-up, legs twitch). `ColorEarned` message → `base_skin` becomes that color and every chameleon mesh moves to the color layer. |
| `gore.rs` | Blood. Messages `Bleed` (spray drops) and `Paint` (splats on the claw). `Fluid` = Green (wasp blood) / Red (chameleon) / Venom (darker green than the blood; the boss glob is also on the color layer). Sticky drops touching a `PaintTarget` (ellipse on body/head/claw bones) become `Splat`s parented to that bone; drops landing on platforms become `Stain`s parented to the platform. Only `STICK_CHANCE` (40%) of sticky drops stick; `PaintTarget.clear` keeps a circle clean (the eye) so the face stays readable. Splats drip then dry over 25 s; caps: 50 splats, 320 stains. |
| `boss.rs` | Shared boss machinery. `BossKind` list (pick with `cargo run -- wasp|roach` or `BOSS=…`; default = newest; **F2 swaps** to the next boss). `Arena` resource (floor/left/right, found at `level::ARENA_X`). `Boss` component + per-boss `static SPEC: BossSpec` (name, hp, radius, blood `Fluid`, earned color, aggro). Does slash hit detection (`chameleon::SLASH_CENTER/REACH`), blood/paint, `BossHit` message (per-boss AI reacts), death spectacle (freeze → slow-mo, zoom punch, flash, 240-drop burst, `ColorEarned(spec.color)`), material flash, heal on `Respawned`, `BossStatus` for the HUD, `Gib`s via `tear_off()`, shared `hurt_player()` and `floor()`. `BossStuff` marks projectiles/gibs (cleared on swap). |
| `wasp.rs` | **Venom Mother** (giant wasp, `SCALE` 1.7, 14 HP, green blood). AI: hover at `KEEP_AWAY` → aim → spit venom (×2) → dive to sting; enraged below half HP (faster, 3-glob fans, dive after each spit). Death: launched spinning, aftershock bursts, wings/abdomen torn off, lies belly-up. |
| `roach.rs` | **The Gutter King** (giant cockroach, `SCALE` 1.5, 12 HP, very light yellow blood, earns golden yellow). Phase 1 on the ground: stalk at `KEEP_AWAY` → rear up (`WINDUP`) → floor dash → winded `RECOVER` (the opening); stops at arena edges; dashes power through hits. Phase 2 (≤ half HP): elytra snap open, takes off; clumsy jittery flight → `AirWindup` → air dash at the chameleon; hitting the floor = crash stun. Contact damage 1. Death: flipped up, antennae/wings torn off, lands belly-up with twitching legs. |
| `hud.rs` | Controls hint (F12 screenshot still works, just not listed), `ScreenFlash` full-screen fade, red HP pips, boss bar (name + fill in the boss's color + draining trail; shown while `BossStatus.engaged`). | |
| `player.rs` | Physics/controller: accel, gravity, coyote time, jump buffering, variable jump height, AABB collision, one-way platforms, riding moving platforms (`Platform.delta`), respawn at last stable ground. Health: `hp` (`MAX_HP` 5), `Player::hurt(damage, knock)` → `Hurt::{Ignored,Hurt,Killed}`, `invuln` i-frames, `stun`; falling in the void costs 1 HP. Death: lies there `DEATH_TIME`, then respawns at `SPAWN` and sends `Respawned`. `SCALE` (0.7) sizes the chameleon and its collider. |
| `level.rs` | Seeded procedural level (`SEED`, `LEVEL_END` = 7000): deep rubble/asphalt ground chunks with gaps, two tiers of floating debris, a wall at the end; a flat 1100-wide boss arena at `ARENA_X` (1500) with two dodge ledges. Platform visuals: cracks, rebar, strata, debris, lamp posts, barbed wire, lit top rims. `Platform` component. `lost_colors` module keeps the original bright palette for later. |
| `unstable.rs` | `Motion` component: `Crumble` (shakes 0.6 s → falls → respawns after 3 s), `Swing` (girder on chains), `Sink` (sinks under weight). `Chain` visuals. `Tremor` earthquakes every 9–16 s (screen shake, stronger swings, random slab collapse). |
| `atmosphere.rs` | Backdrop with `Parallax {origin, factor: Vec2}` (0 = world, 1 = glued to screen, negative = foreground): sky gradient, hazy sun, 3 ruined skylines (windows, twisted beams), pylons/cables/cranes, fog banks, foreground wreckage, smoke plumes over flickering fires (`Puff`, `Fire`), ash + embers (`Particle`, wrap around camera), lightning (flash overlay + regenerated bolt mesh). |
| `camera.rs` | Smooth follow with look-ahead, zoom 0.65, trauma-based `Shake` resource (`add`, `at_least`), `HitStop` (freeze via `Time<Virtual>`, timed in real time; `slow_motion()` eases back), `ZoomPunch` (zooms both cameras), snaps on teleports, and **global grayscale** via `ColorGrading.post_saturation = WORLD_SATURATION (0.0)`. `MainCamera` marker; a child overlay camera (order 1, no grading) renders `COLOR_LAYER` (1) — use `camera::color_layer()` for anything that must keep its color. |
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

- **Blood is on the color layer**, so it's drawn over *all* of layer 0 (incl. fog/foreground wreckage).
  Splats are separate meshes parented to the chameleon's bones; `RenderLayers` is not inherited, set it per entity.
- The color overlay camera is also `IsDefaultUiCamera`, so the HUD keeps its colors.
- Use `With<MainCamera>` (not `Camera2d`) in camera queries — there are two 2D cameras.

## Bevy 0.20 API notes (learned while building)

- `TextFont { font_size: FontSize::Px(18.0), .. }` (not a plain f32).
- `PrimitiveTopology` from `bevy::render::render_resource`; `Indices` from `bevy::mesh`.
- `AlphaMode2d` from `bevy::sprite_render`; `ColorGrading`/`ColorGradingGlobal` from `bevy::render::view`.
- Parenting via `ChildOf(parent)`; `Single<...>` queries; `MessageWriter<AppExit>`.
- Screenshots: `commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path))`.
- Mesh2d has no backface culling, so negative scale (flipping) is fine.
- Messages: `#[derive(Message)]`, `app.add_message::<T>()`, `MessageWriter::write`, `MessageReader::read`.
- `RenderLayers` from `bevy::camera::visibility`; non-HDR 2D tonemaps/grades in-shader, so a 2nd camera
  with `ClearColorConfig::None` and no `ColorGrading` keeps colors.
- `Time<Virtual>::set_relative_speed` is handy for slow-motion screenshots.
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
4. Gore: tongue replaced by claw slash; wasp enemy (venom, dive); green blood on a color layer that
   paints the chameleon, stains the ground and dries.
5. Boss fight: player HP/death/respawn, Venom Mother boss (phases, gibs, HUD bar), hit-stop, bigger
   slash wave hitbox; beating the boss turns the chameleon green (first earned color).
6. Boss framework (`boss.rs`, F2 swap, flat arena in `level.rs`) + second boss: the Gutter King cockroach.

## Ideas / next steps

- Collectible fragments of lost colors (unlock camouflage + raise world saturation).
- Arena walls / boss intro; real checkpoints (respawn is always `SPAWN`).
- Adding a boss: new module with `pub fn spawn(.., &Arena)` that inserts `Boss::new(&SPEC, ..)` + its AI component, its plugin (react to `BossHit`, reset on `Respawned`), and a `BossKind` entry in `boss.rs`.
- More enemies (camouflage as stealth). Maybe blood-earned colors unlock camouflage.
- Hand-made level sections, checkpoints, a real ending past the final wall.
- Not yet visually verified in screenshots: lightning bolt, earthquake, a slab mid-fall.
- Git repo initialized; nothing committed yet.
