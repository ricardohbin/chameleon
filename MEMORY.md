# Camaleon — Project Memory

A 2D platformer prototype in **Rust + Bevy 0.20**. All art is generated in code; the only asset is the
world sketch `assets/world.aseprite` (layout only; its reviewed bake `world.baked` is embedded in the binary).

## Vision / story

A small, evil-looking, bone-white **chameleon** wanders a post-war wasteland **searching for the
colors that war and humans destroyed**. The world starts **grayscale**; colors are meant to be
**earned back** as the game progresses. Tone: apocalyptic, chaotic, unstable — *not cute*. It's **gory**: the chameleon slashes enemies
open and gets painted with their blood (the first color in the gray world is blood).

## Running

```sh
cargo run            # dev build (Bevy deps are optimized, see Cargo.toml profiles)
cargo run --release  # fully optimized (first build recompiles Bevy: a few minutes, extra GBs)
cargo run -- diff    # what changed in world.aseprite since the last bake → report + screenshots/world-diff.png
cargo run -- bake    # world.aseprite → world.baked (after Claude has interpreted the changes)
cargo run -- diff a b  # compare any two sketches (.aseprite or .baked)
TOUR=1 cargo run     # visit spawn, bosses, every slice, every pin, overview → screenshots/tour-*.webp, quit
START=x,y EXIT_AFTER=4 cargo run   # drop the chameleon at a world spot, log where it ends up (+ screenshots/exit.webp), quit
HOLD=d TAP=space@1.2,k@1.3 ...     # simulated input for those tests (keys: a d w s space k shift f j g f1-f10 f12)
AIM=-200,150 ...                    # fake mouse at that offset from the chameleon (test head/eyes/slash/spit aim)
```

**Sketch review loop:** `assets/world.baked` = the sketch state Claude last interpreted, and what
release builds embed. When the user edits the sketch: `cargo run -- diff` (debug builds also warn
"not interpreted yet" at startup / on reload) → interpret the changes (pins, styles, parallax…) →
`cargo run -- bake`. Diffs align on the origin marker, so canvas resizes aren't changes.

Rust is installed in msys2 UCRT64: `export PATH="/c/msys64/ucrt64/bin:$PATH"` in Git Bash.
If the user has the game open, `target/debug/camaleon.exe` is locked (build "fails" at the copy step):
run the fresh `target/debug/deps/camaleon-*.exe` instead.

Debug: **F1** = overlay (panel = strip along the bottom of the screen, three columns (the last lists the F keys): position in world / sketch px from origin / canvas px, movement, ground, slope, region/features, state/distance/direction of the boss whose lair you're in; gizmos on the color layer: player box, velocity, ground normal, foot samples, region/feature/lair/arena rects, platforms, boss line of sight, goal and planned path). **F12 with F1 on** also writes `screenshots/shot-*.txt` with the panel text — the user reports problems with these.

Controls: `A/D` or arrows = walk (the **body faces where it walks**; the **head turns toward the mouse**: pitches toward it (`HEAD_PITCH`, `HEAD_FOLLOW`) and flips back over the shoulder when the mouse is behind (`ChameleonAnim.head_side` scales the head through 0); the **eyes** do the exact look; `Player.aim` = unit vector from `chameleon::neck()` to the mouse) · `Space/W/Up` = jump (hold for higher) · `F` / `J` / left mouse button = claw slash **toward the mouse**, front or back: the swipe arm and the wave arc pivot on the front shoulder (`chameleon::slash_center(ppos, facing)`), sweeping top to bottom on the aimed side; hitbox = half-disc of `SLASH_REACH` around the shoulder on the side of `ChameleonAnim::slash_dir()` (fixed at slash start) · **powers earned from bosses** (`boss::Defeated`, kill order, saved, cleared by F8; HUD hint lists a power only once earned, plus an "EARNED: …" announcement via `boss::Beaten`): Gutter King → `Shift/K` dash (along the walking input, else the facing), Venom Mother → `G` / right mouse button venom spit (`spit.rs`: glob toward the mouse, hurts bosses/mobs through `SpitHit`, splashes on walls) · captured mouse = the OS cursor becomes a crosshair image (`CursorIcon::Custom`, drawn in code in `hud.rs`), the arrow comes back on release · `F2` = teleport to the next boss's lair · `F3` = invulnerability (`Player.god`: `hurt()` ignores everything, so no damage/knockback, void falls just return you to safe ground; HP pips turn gold) · `F8` = restart (`level::restart` in `PreUpdate` re-reads the sketch — from disk in debug — rebuilds the world, sends `WorldReloaded { bosses_moved: true }` + `level::Restart`; modules reset on `Restart` `.after(level::restart)`: fresh chameleon (keeps god), colors forgotten, blood/debris cleared, camera/hit-stop/flash/tremor reset; bosses and mobs respawn from the reloaded sketch) ·
`F4` = flight · `F6`/`F7` = less/more brightness contrast between solid ground and interiors (`terrain_art::Contrast`, in stops, default +0.5; tiles are painted as two layers — ground `Layer::Front`, interiors `Layer::Back` child sprite — tinted live) · `P`/`Pause` = pause; `F10` = full-map WebP export to `screenshots/` (painted plain export uses current contrast; annotated export uses flat slate/charcoal on near black without waiting for textures; painted export has twice the annotated resolution, capped at 16383 px, lossy WebP quality 90; annotated stays up to 8192 px and lossless WebP, no parallax; F1 enables larger region/feature notes, creature proportions and spawn shares, and solid boss/player spawn squares with top legends; shows section progress, then presents a compression notice for two frames before encoding on a worker; elapsed real-time feedback keeps updating and the game stays responsive) · `F12` = lossless WebP screenshot to `screenshots/`.

⚠️ `target/` takes ~4–5 GB. The first build once filled the disk (ENOSPC) — keep ~8 GB free.

## Code map (`src/`)

| File | What it does |
|---|---|
| `main.rs` | App setup, window (1280×720), plugin registration, HUD text. |
| `shapes.rs` | Vector-art toolkit: Catmull-Rom curves (`smooth_closed/open`), `fill` (triangle fan from centroid), `stroke` (ribbons with width/shade/pattern), `triangles`, `gradient_rect`, `ellipse_pts`, `rock_pts`, `scale_color`, and a seeded xorshift `Rng`. |
| `chameleon.rs` | The character rig, built from meshes in a bone hierarchy (`Part` enum: Rig, Body, Head, Leg{Hip/Knee/Foot}, EyeCone, Pupil). Procedural animation: walk cycle, body bob/breathing, curling tail (mesh regenerated every frame), darting eye, claw slash (near front leg, `Leg{swipe}`; `slash_pose`, faint crescent `SlashWave`), hurt blink, camouflage. `ChameleonAnim::striking()` + `slash_id` drive hits. Death pose (rolls belly-up, legs twitch). `ColorEarned` message → `base_skin` becomes that color and every chameleon mesh moves to the color layer. |
| `sketch.rs` | Minimal Aseprite reader (RGBA only; layers with Normal/Multiply blend, hidden layers/groups, slices) → `Sketch`: per-pixel `Cell` (Air/Solid/Back), `Marker`s (blob centroids), `Slice`s. Marker pixels take the cell of what surrounds them. `Sketch::bake` = compact zlib'd RLE format (magic `CAMSKT01`); `decode` reads both. |
| `sketch_tool.rs` | CLI run from `main` before the App when arg 1 is `bake`/`diff`: bake, diff (origin-aligned; cell changes clustered in 8-px blocks → bbox, counts, slices; marker/slice changes), tiny PNG encoder (miniz + CRC32). `warn_if_new` (debug) is called by `level.rs` on load/reload. |
| `terrain.rs` | `Terrain` resource built from the sketch (cropped + `MARGIN`): grid queries for collision (`box_hits/box_top/box_bottom`), `surface`, `rest`, `landing`, `raycast`, `flat_span`, `clearance` (distance to solid), `distance_field`, areas (`area_at`, `style_at` → `Style` from slice names by keyword). `PX` = 8 world units per sketch pixel (tuned by the user after playtesting 5 and 10), `STEP_UP` = 2 px. Spawn / boss positions from markers. |
| `outline.rs` | Mask → traced pixel-corner loops → 1-px stairs collapsed to ramps → Douglas-Peucker; `rasterize` = antialiased even-odd scanline coverage. |
| `terrain_art.rs` | Paints the terrain into 768² sprite tiles (3 texels/px, z −1; patterns and light scaled by `K` = PX/5 so they keep their world size): coverage from `outline`, light from above (`under` depth), lit rims, per-`Style` materials (concrete cracks/strata, junk mosaic with tires, sewer brick + slime, park soil), dark interiors for Back cells. `TerrainPart` marks everything rebuilt on reload. **Progressive**: `spawn` clones the `Terrain` and prepares fields/outlines on the `AsyncComputeTaskPool` (`CanvasTask`), then paints each tile there, nearest to the focus (spawn, or the player on hot reload) first; tiles appear when ready (`finish_tiles`). Pending tasks are `TerrainPart`s, so a reload cancels them. Newer regions' materials live in `src/terrain_art/<region>.rs` (`GROUND`, `material`, `rim`, `interior`). |
| `terrain_props.rs` | Decoration along the terrain per `Style` (rebar, tires, pipes, benches…; no collision, `TerrainPart`). **Progressive**: sites are found on the task pool, then `place_props` visits them within `BUDGET` (4 ms) per frame, always in key order (same result whatever the frame rate). Sites need room to move, not open sky: interiors (Back cells) count as open for edges/walls/ceilings, and props over an interior get the `INDOOR` depth. Newer regions (`Subway`, `Treatment`, `Sinkhole`, `Catacombs`, `Mall`, `Dam`, `Bunker`, `Cave`) place props through `Hooks` in `src/terrain_props/<region>.rs`. |
| `gore.rs` | Blood. Messages `Bleed` (spray drops) and `Paint` (splats on the claw). `Fluid` = Green (wasp blood) / Red (chameleon) / Venom (darker green than the blood; the boss glob is also on the color layer). Sticky drops touching a `PaintTarget` (ellipse on body/head/claw bones) become `Splat`s parented to that bone; drops landing on platforms become `Stain`s parented to the platform (on the terrain: free `Stain`s marked `TerrainPart`). Stains are decals on any face (floor, wall, ceiling, platform sides/underside): `contact` finds the entry point + a smoothed normal, the decal is rotated to it, and its mesh comes from one of `TILTS` (24) pre-built tilt buckets whose drips follow gravity (run downhill along walls/slopes, seep into floors, hang off ceilings). Only `STICK_CHANCE` (40%) of sticky drops stick; `PaintTarget.clear` keeps a circle clean (the eye) so the face stays readable. Splats drip then dry over 25 s; caps: 50 splats, 320 stains. |
| `boss.rs` | Shared boss machinery. `BossKind::ALL`: every boss with a sketch marker is always spawned, **asleep in its `Lair`** (the slice holding its marker ∪ its arena); entering its **arena** wakes it (`Lair.arena`: the roach's floor span, the wasp's open `chamber()` flood-filled around its marker; `Boss.awake`, its AI hunts); leaving the lair or getting `LEASH` away despawns and respawns it asleep at home; dead bosses stay dead; `Respawned` resets living bosses; **F2 teleports** to the next boss's lair. `Arena` (floor/left/right/home) found per boss by `Arena::find` around its sketch marker (`flat_span`). `Boss` component + per-boss `static SPEC: BossSpec` (name, hp, radius, blood `Fluid`, earned color, aggro). Does slash hit detection (`chameleon::SLASH_CENTER/REACH`), blood/paint, `BossHit` message (per-boss AI reacts), death spectacle (freeze → slow-mo, zoom punch, flash, 240-drop burst, `ColorEarned(spec.color)`), material flash, heal on `Respawned`, `BossStatus` for the HUD, `Gib`s via `tear_off()`, shared `hurt_player()` and `floor()` (terrain + platforms). `BossStuff` marks projectiles/gibs (cleared on swap). |
| `wasp.rs` | **Venom Mother** (giant wasp, `SCALE` 1.7, 14 HP, green blood). AI: hover at `KEEP_AWAY` → aim → spit venom (×2) → dive to sting; enraged below half HP (faster, 3-glob fans, dive after each spit). Death: launched spinning, aftershock bursts, wings/abdomen torn off, lies belly-up. Navigation plans ahead (`Nav`, every `REPLAN` s): a BFS out from the wasp over coarse 4-px cells with `NAV_CLEARANCE` scores every reachable cell for what it wants (`Want::Perch` = line of sight at `PERCH_RANGE`, above the chameleon; `Want::Dive` = a straight `dive_lane` with body room down to it; `Home`), keeps its spot unless another beats it by `HYSTERESIS`, and flies the waypoints (cutting corners where `clear_line` allows); no spot → gets as close as it can. Aims only with line of sight; dives only down a checked lane (velocity lined up with it), pulls out if a wall shows up ahead, bounces back up the lane after a sting and replans (gives up on diving after `DIVE_PATIENCE`). Body keeps `BODY_CLEARANCE` from walls; inside that margin it may move along/away (clearance is cell-quantized: requiring a strict increase froze it after dives). |
| `roach.rs` | **The Gutter King** (giant cockroach, `SCALE` 1.5, 12 HP, very light yellow blood, earns golden yellow). Phase 1 on the ground: stalk at `KEEP_AWAY` → rear up (`WINDUP`) → floor dash → winded `RECOVER` (the opening); stops at arena edges; dashes power through hits; follows the terrain height under it. Phase 2 (≤ half HP): elytra snap open, takes off; clumsy jittery flight → `AirWindup` → air dash at the chameleon; hitting the floor = crash stun. Contact damage 1. Death: flipped up, antennae/wings torn off, lands belly-up with twitching legs. |
| `mobs.rs` | Ordinary enemies = each boss's **ecosystem**. Every region slice belongs to the boss whose marker is inside it, else the nearest (`plan`); `terrain.areas` holds only regions (features are in `terrain.features`); the slice holding the spawn is skipped, nothing within `SPAWN_CLEAR` of the spawn or inside both bosses' arena zones (`Arena::find`: Gutter King left..right ± `ROACH_ARENA_MARGIN`; Venom Mother `WASP_CLEAR` radius + her chamber). Deterministic (FNV seed per slice name), count ∝ slice area (`CELLS_PER_MOB`, 2..16), per-kind quotas from `menu`. Kinds by movement (`Moves`): gutter = Roachling/SewerRoach (`Walk`: patrol → rear → charge; sewer ones tougher, `stubborn`), Fly (`Swarm` of 4: erratic buzz, orbit the chameleon, nip, scatter), Flea (`Hop`: little hops, crouch → ballistic leap; bleeds **red**), Rat (`Pack` of 3, AI in `mobs/rat.rs`: lurk / sneak / hit-and-run / cornered fury; red blood); hive = Drone (`Fly`: hover → aim → dive), Grub (`Walk`, slow crawl → lunge), Soldier (`Perch`: still, spits venom `Glob`s in arcs), Nest (`Hang` from ceilings, releases up to `BROOD` drones), Spider (`Dangle` on silk from a ceiling: bobs, drops on the chameleon passing below, dangles a moment, climbs back). States Idle/Windup/Attack/Recover/Hurt/Dead; everyone leashed to its spot (`leash`); walkers use `walk` (steps ≤ `STEP_UP`, body box `fits` a step above ground so slopes work), flyers `fly` (clearance). Slashed with `chameleon::SLASH_*` like bosses, blood/paint scaled by size, corpse flips and falls (`boss::floor`). All rebuilt on `Respawned`, and a frame after `WorldReloaded`. Own `Mob` component (not `Boss`); `MobStuff` = gibs/globs. |
| `mobs/rat.rs` | Rat minds (`Mind` on each `Mob`). **Attention** 0..1 = 0.7 body facing it + 0.3 head gaze², ×0.35 when the chameleon is busy (slashing elsewhere / eating), ×0.7 airborne, 0 stunned; charging at it counts. **Territory** = its walkable floor mapped on the first frame (`walk` up to `leash` each way), `home` recentered on it. **Lurk**: spot on its own side of the chameleon at `SNEAK_DIST`..`WARY_DIST` by attention, dark (Back cell) spots +, in front of a watching chameleon −; unseen → hide in the dark near home. Sneaks in stop-and-go with sniff pauses, freezes when the attention jumps (`STARTLE`) then backs off, faces the chameleon. **Hit and run**: within `STRIKE`, dares at `AMBUSH`×(1−a)², rush breaks off at `NERVE`×a², bite → bolt (fading) → watch. **Cornered** (`CORNER_ROOM`/`CORNER_DIST`): +`DESPERATE`, rears up (`windup` 0.4 s) if watched, then `fury` (no breaking off). Rats move in ≥4-unit steps (`Mind::stride`): tiny steps catch the body box on pixel-step edges in `walk`. |
| `mob_art.rs` | Mob bodies (shapes.rs, own materials per mob for the hit flash; z 11): roach (sewer variant banded), fly, flea (big jumping leg), rat (tail lashes), spider (hourglass, `Thread` bone stretched to its anchor), drone (also the soldier, wings folded), grub (6 segments, crawl wave), paper nest on a stalk (`NEST_STALK`, sways from the ceiling). `MobBone` animation, `dismember` tears legs/antennae/wings/abdomen off via `boss::tear_off`. |
| `feeding.rs` | **Feeding on remains**: an attack press with no living mob/boss in the slash hitbox but a corpse there becomes a bite (`ChameleonAnim::bite`, head dips and chews) instead of a slash. The corpse's `food` (`MobSpec::food` / `BossSpec::food`, in health ticks: ~¼ per hp plus a bonus for danger; nest-born drones 0, no farming) becomes **shadow health** (`Player::shadow`); mob corpses are `Devoured` (squash, despawn), boss carcasses are `PickedClean` (stay) and first add **+1 max HP** (`Player::max_hp`, kept through deaths). Mob remains are refused when there's no room (the slash happens instead). |
| `hud.rs` | Controls hint (F12 screenshot still works, just not listed), `ScreenFlash` full-screen fade, red HP pips (one per `max_hp`; shadow health fills empty pips from the bottom: dark = promised, brighter = digested so far), boss bar (name + fill in the boss's color + draining trail; shown while `BossStatus.engaged`). | |
| `player.rs` | Physics/controller: accel, gravity, coyote time, jump buffering, variable jump height, dash (Shift/K: `DASH_SPEED` burst, no gravity, once per airtime, keeps momentum), collision against the terrain grid in small sub-steps (walks up ramps and steps ≤ `STEP_UP`, sticks to slopes going down; slopes (measured over ±4 px): ≤ `WALK_SLOPE` 45° walk normally, 45–60° uphill gets slower and a downhill pull grows (`hardness`), ≥ `STEEP_SLOPE` 60° forced slide (slippery features: 39°/52°); jumps tilt toward the slope normal by `hardness` (no hopping up walls); `perched` slips you off a corner only over a real drop; slides keep momentum through the hops (`slipping`), no climbing while sliding; `step` = visual offset easing those hops, applied on the rig), AABB collision with moving platforms, one-way platforms, riding moving platforms (`Platform.delta`), respawn at last stable ground. Health: `hp` (`max_hp`, starts at `MAX_HP` 5), shadow health (`shadow` promised ticks, `digest` 0..1 progress; one tick per `DIGEST_TIME` 2.5 s; a hit throws away the tick being filled; death clears it), `Player::hurt(damage, knock)` → `Hurt::{Ignored,Hurt,Killed}`, `invuln` i-frames, `stun`; falling in the void costs 1 HP. Death: lies there `DEATH_TIME`, then respawns at `spawn_point()` (red marker) and sends `Respawned`. The void is `VOID_DEPTH` below the world's bounds. `SCALE` (0.7) sizes the chameleon and its collider. **Fall damage**: `fall_top` tracks the highest the feet got since last grounded; on landing, `drop × (1 − softness) / FLOOR_HEIGHT` (one building storey = `terrain::FLOOR_HEIGHT` 96 units, the window grid; a full jump ≈ 1.4 floors) → under `FALL_SAFE_FLOORS` (2) nothing, then 1 HP at 2 floors +1 per extra floor (HP 5 dies at 6). `Style::softness()` (exhaustive match: **every new ground material/style must declare it**; Concrete 0, Park .35, Sewer .5, Trash .6); platforms are hard. Teleports call `Player::forget_fall`. Invulnerability ignores it. |
| `level.rs` | Loads the sketch (`include_bytes!("world.baked")` for release; debug builds read the file on disk and **hot-reload** it (polled every 0.5 s by mtime + size; a change must hold still for one poll; a failed read — locked or half-written by Aseprite — stays pending and is retried each poll instead of being forgotten): despawn `TerrainPart`s, rebuild, replace `Terrain`). Spawns the terrain art and the hand-placed moving pieces `PINS` (slab / crumble / plate / girder), each pinned to a slice name + pixel offset from its top-left. `Platform` component (moving pieces only). `lost_colors` keeps the bright palette for later. |
| `unstable.rs` | `Motion` component: `Crumble` (shakes 0.6 s → falls → respawns after 3 s), `Swing` (girder on chains), `Sink` (sinks under weight). `Chain` visuals. `Tremor` earthquakes every 9–16 s (screen shake, stronger swings, random slab collapse). |
| `atmosphere.rs` | Backdrop (x ranges cover `Terrain.bounds` for each parallax factor) with `Parallax {origin, factor: Vec2}` (0 = world, 1 = glued to screen, negative = foreground): sky gradient, hazy sun, 3 skylines whose pieces follow the area they're seen from (`Piece`: ruined towers / junk heaps / sewer walls with culverts and pipes / dead trees + one fairground wheel), pylons/cables/cranes, fog banks, foreground wreckage, smoke plumes over flickering fires (`Puff`, `Fire`), ash + embers (`Particle`, wrap around camera), lightning (flash overlay + regenerated bolt mesh). |
| `camera.rs` | Smooth follow with look-ahead, zoom 0.65, trauma-based `Shake` resource (`add`, `at_least`), `HitStop` (freeze via `Time<Virtual>`, timed in real time; `slow_motion()` eases back), `ZoomPunch` (zooms both cameras), snaps on teleports, and **global grayscale** via `ColorGrading.post_saturation = WORLD_SATURATION (0.0)`. `MainCamera` marker; a child overlay camera (order 1, no grading) renders `COLOR_LAYER` (1) — use `camera::color_layer()` for anything that must keep its color. |
| `dev.rs` | F12 screenshot; `TOUR=1` screenshot tour (areas, features, pins); `START`/`EXIT_AFTER` drop tests. |
| `debug_overlay.rs` | F1 overlay (`DebugInfo` resource holds the panel text; `boss::Sense` component is filled by the boss AIs: goal, target, line of sight, path). |
| `spit.rs` | Venom spit (earned from the Venom Mother): `Glob`s from the head along `Player::aim`, slight drop, `SpitHit { target, dir }` to the first boss/mob on their path (applied with the slash in `boss::slash_bosses` / `mobs::slash_mobs`), Venom splash on walls/platforms. |
| `features.rs` | Feature behaviors beyond terrain rules: `Debris` junk shed while sliding down a crumbling feature. |

## Key design decisions

- **The world is a sketch** (`assets/world.aseprite`, 4096×2048 RGBA). Colors, not layer names, carry meaning:
  white = solid, dark gray (Multiply layer over white) = passable structure / interiors, black = air,
  **magenta cross = world origin (0,0)**, red = player spawn, yellow = Gutter King, green = Venom Mother.
  Slices are prefixed: **"region: …"** = areas (art `Style` by keyword, boss lair) and **"feature: …"** = special
  spots described in words, handled by keywords (`terrain.features`): "slippery"/"sliding"/"one way down" → slide-only
  slopes; "fall apart"/"crumbl" → junk debris while sliding (`features.rs`); "dash" gaps → no bridge, the player's dash
  (Shift/K) crosses them. Pins (`level::PINS`) are origin-relative sketch px, not slice-relative (slices get renamed). 1 px = 8 units; 1 px rise = ramp, 2 px = stair
  (auto-climbed), 3–10 px = a jump (max jump ≈ 17 px high, ≈ 22 px across). The user edits by color and expects Claude to
  interpret the shapes (art, parallax, where to put moving pieces). The user annotates intent in slice
  names: "sliding" / "one way down" makes that area's slopes (> `SLIDE_SLOPE`) slide-down-only — don't add
  ways back up there. Area colors (camouflage: `terrain_art::ground_color`) are Claude's to choose; sketch
  colors are only for markers, unknown colors are ignored. A boss marker moving on reload restarts the fight. After a sketch change: run the tour,
  check pins still make sense (a buried pin logs a warning), adjust `PINS` / `Style` keywords.

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
- Z layers: sky −100 … skylines −95/−90/−70 … smoke −55 … terrain tiles −1 … platforms 0 … player 10 … ash −10/30 …
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
7. World from the Aseprite sketch: terrain grid collision, outline-traced textured terrain per area style,
   hot reload, embedded for release, area-aware parallax, pinned moving platforms, wasp flow-field nav.
   Procedural level removed.
8. Mobs (`mobs.rs`, `mob_art.rs`): each region gets its boss's ecosystem (roaches, fly swarms, fleas, rat packs /
   drones, larvae, venom-spitting soldiers, drone nests, silk spiders), arenas and the spawn kept clear.
   Sketch slices are now prefixed `region: …` (areas) or `feature: …` (special spots, no mobs).

## Ideas / next steps

- Collectible fragments of lost colors (unlock camouflage + raise world saturation).
- Arena walls / boss intro; real checkpoints (respawn is always the red marker).
- Vector props along terrain surfaces per area (rebar, tires, pipes, benches, lamp posts).
- Check the building's parkour (white slivers inside the gray) is actually climbable; add pins where not.
- Adding a boss: new module with `pub fn spawn(.., &Arena)` that inserts `Boss::new(&SPEC, ..)` + its AI component, its plugin (react to `BossHit`, reset on `Respawned`), and a `BossKind` entry in `boss.rs`.
- More enemies (camouflage as stealth). Maybe blood-earned colors unlock camouflage.
- Hand-made level sections, checkpoints, a real ending past the final wall.
- Not yet visually verified in screenshots: lightning bolt, earthquake, a slab mid-fall.
- Git repo initialized; nothing committed yet.

- F4 toggles flight: gravity and walking/dash controls are bypassed; Space/W/Up adds a 900-unit impulse toward the mouse in world space. Exponential drag (3.5/s) brings flight to rest, collisions remain active, and flight landings cause no fall damage. F1 shows flight state; simulated TAP supports f4.
