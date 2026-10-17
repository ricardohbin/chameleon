# WASM performance plan

Profile (dev build, wasm): ~92% of the time is Bevy's per-entity systems over a huge number of
`Mesh2d` entities (asset-change checks, specialization, bounds, visibility, transform propagation,
render extract), single-threaded. Game logic is ~5%. The fix is fewer entities, for native and web.

## Done

- [x] Profile the wasm build and find the bottleneck (entity count, not game logic)
- [x] `shapes::concat`: merge several toolkit meshes into one
- [x] Props: parts of a prop with the same material, depth and layer are merged into one mesh
      (`Props::emit` / `emit_color` / `flush` in `terrain_props.rs`; moss, vines and the regional props all go through it)
- [x] Mobs: the black outline is folded into each part's own mesh (black vertices, behind the part)
      instead of a second entity per part (`mob_art::with_outline`; falls back to the old child entity if the mesh has no shade data)
- [x] Culling: props far from the view lose their `Mesh2d` / material (`cull_props`, `PropRoot` / `Asleep` / `Dormant`), and get them back when the camera nears. Hysteresis + a cap per pass
- [x] `cargo check` passes on native and `wasm32-unknown-unknown`

## To do

- [ ] Look at it in the game (native): props, vines/moss color reveal (beat a boss), mob outlines, culling pop-in at the screen edge
- [x] Re-profile the wasm build (trace 2): median frame 107 ms → 55 ms (≈9 → 18 fps; includes opt-level 3 and the canvas change). Prop placement was still running during the trace (see next item)
- [ ] Prop placement is slow on wasm (4 ms/frame budget): still ~7% of every frame 4–10 s in, and each new prop adds a material + bind group (~12%). Get a trace AFTER placement finishes; consider a bigger budget while the menu is open
- [ ] Mobs: merge static parts that share a bone and a material into one entity
- [ ] Dedupe materials (one `ColorMaterial` handle per distinct color instead of one per prop)
- [x] Wasm build profiles: `web/build.sh --fast` (opt-level 3, wasm-opt -O3); default stays size-optimized so traces stay comparable
- [x] Wasm: canvas at one pixel per CSS pixel (`scale_factor_override(1.0)` in `main.rs`; slightly softer on hi-DPI)
- [x] `web/build.sh --names`: keeps function names for DevTools traces (no wasm-opt)
- [ ] Update README / MEMORY.md if anything user-visible changed (nothing so far)

## Notes / risks

- The culling radius comes from the camera's projection area, so boss-arena zoom-outs are covered.
- A mesh shared by several mobs is outlined once (tracked by asset id).
- Nothing committed or pushed.

## Trace 3 (after placement finished): flat 55–60 ms/frame

- [x] Counts log: every 5 s, `counts: entities … | Mesh2d … (hidden …) | sprites … | prop roots … (asleep …), sleeping parts … | mobs …` (browser console / terminal)
- [x] Stop per-frame asset writes that change nothing (each write re-prepares the material / re-uploads the mesh and rescans every mesh entity):
  - `shapes::set_color` (writes a material color only if it differs): sky tint, fires, smoke, lightning flash, chameleon skin, fume puffs (also only near the camera), gore stains (32 steps)
  - chameleon tail mesh rebuilt only when curl/sway/rings change
  - water surfaces rebuilt only when the pool is near the view
- [ ] Re-profile: look at `prepare_bind_groups`, `ExtractedAssets<MeshMaterial2d>`, `mark_2d_meshes_as_changed…` (were 7 % / 6 % / 3.5 %)
- [ ] Read the counts line to see if culling works and what else holds the entities

## Counts (from the browser console): culling works for props, but mobs were the bulk

`entities 132672 | Mesh2d 32212 (hidden 352) | prop roots 14275 (asleep 13812), sleeping parts 24078 | mobs 1181`

- Props: 97 % asleep, so they are no longer the problem. The 32 k awake `Mesh2d` are almost all **mobs** (1181 mobs × ~25 parts, the whole world's, all alive from the start).
- [x] The cull is now generic (`terrain_props::cull::<R>`) and also runs for `Mob` roots (render parts only; the mob's logic keeps running). Newly spawned roots are skipped one frame (mob outlines are built then)
- [ ] Re-run: expect `Mesh2d` to drop from ~32 k to a few thousand
- [ ] Other entities: ~132 k in total, ~38 k are prop roots + sleeping parts (no Mesh2d, but still in transform propagation / visibility). Next step if still slow: despawn sleeping prop parts instead of keeping them

## Sleeping mobs, look-ahead, vanishing corpses and gore

- [x] `ViewReach` (`track_view`): the view's position and where it is heading (camera speed × 1 s, max 1500 units). Anything within the margin of that path counts as near, so a fast fall wakes what is below before it arrives
- [x] Mobs asleep (far beyond the view + 900 units, woken at +500) no longer run their logic: `mobs::think`, `nests`, `flash_mobs` and `mob_art::animate` skip them (`Without<Asleep>`); they stay put and resume where they were. Bosses are separate and unchanged (they already only act when awake)
- [x] Dead mobs far from the view are despawned (`clear_far_corpses`); blood splats, stains and drops far from the view are despawned (`gore::clear_far`)
- [x] Wake cap per pass 600, sleep cap 250, pass every 2 frames
- [ ] Not changed: bosses (wake rules are the existing ones: lair entry; the centipede already wakes on region entry), and mobs of other systems that query `Mob` directly (slash, electric, spit, feeding), which only matter near the player anyway
- [ ] Re-run and send the `counts:` line + a trace

## Sleep resets the mob; the other all-mobs systems skip sleepers

- [x] `reset_sleepers`: a mob that goes to sleep is rebuilt with `Mob::new` (health kept, never raised) and put back at its `home`, so it wakes in its idle pose, not mid-leap. Dead mobs are skipped (they get despawned instead)
- [x] `Without<Asleep>` also on the other systems that loop over every mob: slash, spit, webs, electric, bunker balls, catacombs, cave, subway, sewer (belch, leech drag), treatment (drag, acid). Left alone: feeding (corpses), ending, hole_up
- Profile (trace 4): all of these together were ~2–3 % of the frame (`mobs::think` 0.7 %, `mob_art::animate` 0.7–1.6 %, `webs::stick` 0.3 %, `cave::feel` 0.2 %), so this is hygiene, not the main win

## Batch after traces 131608 / 131800 (55–60 ms → ~40 ms; Mesh2d 32 k → 9.7 k)

Findings: visibility check ~15 % (iterating all 132 k entities incl. ~47 k sleeping parts), render submit ~26 %, material preparation 10–15 %, `wasp::think` 4.6 %.

- [x] Sleepers shed `Visibility` / `InheritedVisibility` / `ViewVisibility` (kept in `Asleep` / `Dormant`, restored on wake), also on the intermediate entities of a hierarchy. A sleeping mob wakes `Inherited` (what hid it is over)
- [x] `wasp::think`: no path planning while the boss isn't awake and is at home (it searched the map every 0.4 s)
- [x] More per-frame material writes now skip identical values (`set_color` / `set_alpha`): regrowth sky/glow/shafts/backdrop, vine fade, pill bugs. Pill bugs far from the view don't animate
- [x] Opaque gray prop parts: color and depth baked into the vertices (`shapes::bake`), all drawn with ONE shared white material and merged into ONE mesh per prop (was: one per material and depth)
- [x] Vines / moss living copies: one shared material per boss and distance band (`Props::vine_mat`, bands of `BLUE_BAND`), their colors baked in the meshes (was: 1–2 materials per plant). `Vine.base` is white; the lightning-flash tint now multiplies instead of mixing (subtle)
- [ ] Not changed: per-mob materials (each mob flashes on hit with its own), boss effect materials, curtains' silk (animated per curtain)
- [ ] Look at in game: prop colors (baked), moss/vine reveal after a boss, sleeping mobs waking idle
- [ ] Next if still slow: merge props of a region into chunk meshes (fewer draws), the render submit cost is ~26 %

## Profiling in Chrome over CDP (scratchpad `prof.mjs` + `hook.js` + `an3.py`)

First build with all the changes above, 1280x720 and 1920x1080, real gameplay (Enter on the menu, hold D + jumps): ~25 ms/frame average (menu: 60 fps), with hitches (max 700 ms).
Per frame, from a WebGL call counter: **555 `createBuffer` + 555 `deleteBuffer` + 555 `bufferData` + 615 `bufferSubData`**, but only ~235 draws. So hundreds of materials are modified every frame (each = new uniform buffer + bind group); the wgpu tracker drops (15 % self) and `prepare_bind_groups`/`prepare_asset` (~18 %) are the consequence.

- [x] Suspect: the smoke plumes of `atmosphere.rs` (every plume: 2 fires + 14 puffs, each with its own material, all animated every frame, on screen or not; ~550 materials). They now share materials (fires flicker in 4 ways, puffs live 3 lives): ~50 materials
- [x] `log_churn`: every 5 s, `churn/frame: materials N (distinct) meshes N | busiest: …` with the components of the entities that own the most-modified materials
- [ ] Re-profile with the new build (19 min fat-LTO build) and read the churn line

## Run 6 (smoke plumes shared): ~19 ms/frame average (was 25), createBuffer/frame gone — but 54 380 live GL buffers

`createBuffer` total 226 k, `deleteBuffer` 172 k → **54 k live GPU buffers** (one per mesh asset + one per material). wgpu's tracker/usage-scope drops are ~18 % of the frame and grow with that count; `render_system` is ~34 %. The 650–730 ms hitches are `mobs::rebuild` after the chameleon dies (all 1181 mobs get their art built at once).

- [x] **Mobs build their body lazily** (`mob_art::spawn_asleep`, `art`, `mobs::cull_mobs`): a far mob is just the `Mob` (no entities, meshes, materials, visibility); the body is built when it is near the view (6 per pass) and thrown away when far. After the celebration, woken mobs are tinted. Outline now runs for `NeedsOutline` (built or woken mobs)
- [x] **Props are baked into chunks** (`terrain_props::bake_chunks`, 1536-unit chunks) once placement is done: plain props (no `Web`, no nested children) become one mesh per chunk, material, layer, vine band (and depth, for translucent ones); world position and depth in the vertices; opaque gray = one entity per chunk. The few special roots keep the old sleeping
- [ ] Re-profile: look at live buffers (`GL totals`), `churn`, tracker drops, frame times

## Run 9 (lazy mobs + baked props): 17 ms/frame flat at 1280x720, 1920x1080 and 2560x1440 (60 fps), max hitch 183 ms (the restart after dying)

Live GL buffers 54 k → 16 k, entities 132 k → 66 k, no per-frame buffer creation. At 1440p the main thread is ~95 % busy, so little headroom: the entity scans (`AssetChanged<Mesh2d>` 13 %, visibility 8 %) are what is left, and `Mesh2d` is 15 k (7 k of them hidden, baked vine groups).

- [x] Baked vine/moss groups exist for the engine only when their color's wave arrives (`wake_vines`): until then they are `Dormant` (no mesh, no visibility)
- [x] Translucent baked parts are grouped to a tenth of depth (was a hundredth): fewer entities
- [x] `log_archetypes` (once, at 30 s): the biggest archetypes, to find what the other ~50 k entities are
- [ ] Re-profile; read `archetype:` lines

## Run 11 / 12 (dormant vine groups, coarser translucent groups): flat 17 ms (60 fps, vsync) at 1080p with 19 % of the main thread idle; same in the catacombs (a save injected at -1200,-5600): no frame over 50 ms

- `counts: entities` is the slot high-water mark (`Entities::len`), not live entities: the "50 k unexplained entities" were freed slots. Live: ~9 k `Mesh2d` + ~4.5 k dormant vine groups
- Live GL buffers 14 k (was 54 k)
- Hitch left: ~184 ms when the chameleon dies and the world restarts
- [x] Release (`web/build.sh`, opt-level s) built for the final validation — see below
- [ ] Remove or gate the diagnostics (`log_counts`, `log_churn`, `log_archetypes`) once the numbers are settled

## Final validation: release build (`web/build.sh`, opt-level s, 42 MB wasm)

| case | ms/frame |
|---|---|
| 1920x1080, start area | 17 flat (60 fps), no frame over 50 ms |
| 1920x1080, catacombs | 18 |
| 2560x1440, start area | 23 (mix of 60 and 30 fps) |

The `--names` build (opt-level 3) held 17 ms at 2560x1440 too, so opt-level s costs about 6 ms/frame there. `web/build.sh --fast` (opt-level 3 + wasm-opt -O3) is the choice if 1440p+ or slower machines matter; it costs a bigger download.
- [ ] Decide: ship `--fast`, or switch `web-release` to opt-level 3
- [ ] Remove or gate the diagnostics (`log_counts`, `log_churn`, `log_archetypes`)
- [ ] Look at it in the game by eye: prop colors, vine/moss reveal after a boss, mobs waking, smoke

## Decisions applied

- [x] `web-release` is now opt-level 3 (`web/build.sh`); the size build is `web/build.sh --small` (`web-small`); `--names` inherits the default; `--fast` is gone. README updated
- [x] Diagnostics (`log_counts`, `log_churn`, `log_archetypes`) are behind the `diag` cargo feature (off by default): `FEATURES=diag web/build.sh --names`
- [ ] Rebuilt and profiled the new default once (below)

## Final web build (itch.io)

`web/build.sh` (opt-level 3) → `web/dist` (`index.html`, `ghore.js`, `ghore_bg.wasm.br` 9.6 MB from 60 MB, `GhoreSerif.ttf`) and `web/ghore-web.zip` (10 MB, `index.html` at the root). Test locally with `python web/serve.py`.

- Loading page: download progress (counted against the decompressed size written by the build), compile, "Building the world…" with a clock; hidden by the `ghore-ready` event the game sends when the props are baked (about 24 s after the download on this machine; 120 s fallback)
- Quiet machine, new default: **17 ms/frame flat at 1920x1080 and 2560x1440** (60 fps); the only slow frame is the restart after dying (~190 ms)
- [ ] Upload `web/ghore-web.zip` to itch.io as a draft and check the first load (how itch serves the `.br`)
- [ ] Look at it in the game by eye: prop colors, vine/moss reveal after a boss, mobs waking, smoke
