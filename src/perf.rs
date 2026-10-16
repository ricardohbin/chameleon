//! Dev-build frame-drop watchdog (compiled out of release builds).
//!
//! Prints a `WARN` whenever a frame takes much longer than the recent average, with a quick
//! breakdown of where the time went and a few counters that usually explain it:
//!
//! ```text
//! frame drop: 48.2ms (avg 8.1ms) | render-wait/First 2.1 PreUpdate 0.4 FixedLoop 0.0 Update 41.0 PostUpdate 4.5 Last 0.2
//!   entities 5231 (+3980 since last drop) sprites 4100 meshes 220 | images 312 (+1) meshes-assets 88 (+0)
//! ```
//!
//! How to read it:
//! * `render-wait/First` large  → the main thread waited for the render thread / GPU / vsync
//!   (too many draw calls, huge overdraw, shader/pipeline compilation, asset upload).
//! * `Update` / `PostUpdate` large → game logic or transform/visibility propagation (look at the
//!   entity counters: a jump in entities, or in images/meshes assets, points at per-frame spawning
//!   or regenerated textures).
//! * `FixedLoop` large → FixedUpdate catching up (physics).
//!
//! A one-line summary every few seconds (only when something was slow) gives the average fps and
//! the worst frame. For per-system detail run once with `cargo run --features bevy/trace_chrome`
//! (writes `trace-*.json`, open in ui.perfetto.dev) or `bevy/trace_tracy`.
//!
//! Env: `PERF_MS=40` sets the absolute drop threshold in ms (default 25); `PERF=0` disables.

use std::time::{Duration, Instant};

use bevy::{
    app::MainScheduleOrder,
    ecs::{entity::Entities, schedule::ScheduleLabel},
    prelude::*,
};

const PHASES: [&str; 6] = ["render-wait/First", "PreUpdate", "FixedLoop", "Update", "PostUpdate", "Last"];
const WARMUP_FRAMES: u32 = 60;
const SUMMARY_EVERY: Duration = Duration::from_secs(5);

/// Marker schedule run right after the main schedule with this index (see `PHASES`).
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct Mark(usize);

pub struct PerfPlugin;

impl Plugin for PerfPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("PERF").is_ok_and(|v| v == "0") {
            return;
        }
        let threshold_ms = std::env::var("PERF_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(25.0);
        let now = Instant::now();
        app.insert_resource(Perf {
            threshold_ms,
            stamps: [now; 6],
            prev_end: now,
            avg_ms: 16.0,
            frames: 0,
            window_start: now,
            window_frames: 0,
            window_drops: 0,
            window_worst_ms: 0.0,
            last_counts: Counts::default(),
        });
        let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
        order.insert_after(First, Mark(0));
        order.insert_after(PreUpdate, Mark(1));
        order.insert_after(RunFixedMainLoop, Mark(2));
        order.insert_after(Update, Mark(3));
        order.insert_after(PostUpdate, Mark(4));
        order.insert_after(Last, Mark(5));
        for i in 0..5 {
            app.add_systems(Mark(i), move |mut p: ResMut<Perf>| p.stamps[i] = Instant::now());
        }
        app.add_systems(Mark(5), end_of_frame);
    }
}

#[derive(Default, Clone, Copy)]
struct Counts {
    entities: u32,
    images: usize,
    meshes: usize,
}

#[derive(Resource)]
struct Perf {
    threshold_ms: f32,
    stamps: [Instant; 6],
    prev_end: Instant,
    avg_ms: f32,
    frames: u32,
    window_start: Instant,
    window_frames: u32,
    window_drops: u32,
    window_worst_ms: f32,
    last_counts: Counts,
}

fn end_of_frame(
    mut p: ResMut<Perf>,
    entities: &Entities,
    sprites: Query<(), With<Sprite>>,
    meshes2d: Query<(), With<Mesh2d>>,
    images: Res<Assets<Image>>,
    meshes: Res<Assets<Mesh>>,
) {
    let end = Instant::now();
    let ms = |a: Instant, b: Instant| b.saturating_duration_since(a).as_secs_f32() * 1000.0;
    let frame_ms = ms(p.prev_end, end);
    let mut phases = [0.0f32; 6];
    phases[0] = ms(p.prev_end, p.stamps[0]);
    for i in 1..5 {
        phases[i] = ms(p.stamps[i - 1], p.stamps[i]);
    }
    phases[5] = ms(p.stamps[4], end);
    p.prev_end = end;
    p.frames += 1;
    if p.frames <= WARMUP_FRAMES {
        return;
    }

    let avg = p.avg_ms;
    let is_drop = frame_ms > p.threshold_ms && frame_ms > avg * 1.8;
    // Keep the average honest: spikes only nudge it.
    p.avg_ms = avg + (frame_ms.min(avg * 2.0) - avg) * 0.05;
    p.window_frames += 1;
    p.window_worst_ms = p.window_worst_ms.max(frame_ms);

    if is_drop {
        p.window_drops += 1;
        let now = Counts { entities: entities.count_spawned(), images: images.len(), meshes: meshes.len() };
        let last = p.last_counts;
        let breakdown: Vec<String> = PHASES.iter().zip(phases).map(|(n, v)| format!("{n} {v:.1}")).collect();
        warn!(
            "frame drop: {frame_ms:.1}ms (avg {avg:.1}ms) | {}\n  entities {} ({:+} since last drop) sprites {} mesh2d {} | image assets {} ({:+}) mesh assets {} ({:+})",
            breakdown.join(" "),
            now.entities,
            now.entities as i64 - last.entities as i64,
            sprites.iter().count(),
            meshes2d.iter().count(),
            now.images,
            now.images as i64 - last.images as i64,
            now.meshes,
            now.meshes as i64 - last.meshes as i64,
        );
        p.last_counts = now;
    }

    if end.duration_since(p.window_start) >= SUMMARY_EVERY {
        if p.window_drops > 0 {
            let secs = end.duration_since(p.window_start).as_secs_f32();
            warn!(
                "perf: {:.0} fps over last {secs:.0}s, {} drops, worst frame {:.1}ms",
                p.window_frames as f32 / secs,
                p.window_drops,
                p.window_worst_ms
            );
        }
        p.window_start = end;
        p.window_frames = 0;
        p.window_drops = 0;
        p.window_worst_ms = 0.0;
    }
}
