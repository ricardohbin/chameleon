//! Power pills: iridescent vermin crawling slowly around a spot of wall (the
//! pure blue `#0000ff` marker of the sketch). Touch one to collect it (the icons
//! over the health), press `Z` to swallow one: about `DOSE` seconds of being
//! harder, better, faster, stronger (speed, jump, and above all strength: the
//! meter that wall walking, swimming, dashing and spitting spend). Pills stack
//! (as many doses at once as you dare: more of everything with diminishing returns, and from the second dose on, a more and more chaotic body), but once it runs out
//! comes the crash: as much worse as the peak was better, for a fifth of the time.
//! While it lasts the health pips turn prismatic, fewer of them as it times out.

use bevy::{prelude::*, sprite_render::AlphaMode2d};

use crate::{
    camera::{self, Shake},
    hud::ScreenFlash,
    level::WorldReloaded,
    save::AutoSave,
    player::{self, Player},
    shapes::{ellipse_pts, fill, smooth_open, stroke, Rng},
    terrain::Terrain,
};

/// Seconds of high one pill gives.
pub const DOSE: f32 = 30.0;
/// Most doses in the blood at once.
pub const MAX_STACKS: f32 = 3.0;
/// Most pills carried.
pub const MAX_STOCK: u32 = 12;
/// The last seconds of a dose fade out instead of dropping.
const FADE: f32 = 4.0;
/// The crash lasts this share of the time spent high (times the square root of the peak level, so it
/// grows faster than the doses: 1 dose ~6 s, 2 ~17 s, 3 ~31 s, 5 ~67 s, 8 ~136 s), within these bounds (seconds).
const CRASH_SHARE: f32 = 0.2;
const CRASH_MIN: f32 = 4.0;
const CRASH_MAX: f32 = 150.0;
/// The crash eases off over its last share.
const CRASH_EASE: f32 = 0.35;
/// How close the chameleon has to be to a pill to pick it up.
const PICKUP: f32 = 46.0;
/// Two spots this close (sketch pixels) are the same pill.
const SAME_SPOT: i32 = 3;

/// A pill's identity: where its marker is on the sketch, in pixels from the origin marker (so
/// it survives edits elsewhere in the world, and the canvas being resized or recropped).
fn spot(world: Vec2) -> IVec2 {
    (world / crate::terrain::PX).round().as_ivec2()
}

/// The chameleon's pill state (`Player::pill`).
#[derive(Default, Clone)]
pub struct Pill {
    /// Pills carried.
    pub stock: u32,
    /// Which creatures of the map were already picked up (their spot in sketch pixels from the origin
    /// marker, see `spot`): they stay gone.
    pub taken: Vec<IVec2>,
    /// Seconds of high left (stacks add up: 2 pills = 60 s, at double strength for the first 30).
    pub high: f32,
    /// Seconds spent high so far, and the strongest it got: they size the crash.
    spent: f32,
    peak: f32,
    /// The body twitching: seconds left, and the input it forces meanwhile.
    glitch_time: f32,
    glitch_dir: f32,
    glitch_v: f32,
    phantom_jump: bool,
    rng: u32,
    /// Seconds of crash left, its length and how bad it was at the start.
    crash: f32,
    crash_len: f32,
    crash_peak: f32,
}

impl Pill {
    /// How high it is: 0 (clean), 1 for a dose, one more per dose with no limit, falling off in the last seconds of each dose.
    pub fn level(&self) -> f32 {
        if self.high <= 0.0 {
            return 0.0;
        }
        let doses = (self.high / DOSE).ceil();
        let into_top = self.high - (doses - 1.0) * DOSE;
        let fade = (into_top / FADE).clamp(0.0, 1.0);
        (doses - 1.0) + fade
    }

    /// How bad the crash is: 0 (none) up to the peak level it follows.
    pub fn crash_level(&self) -> f32 {
        if self.crash <= 0.0 {
            return 0.0;
        }
        let left = self.crash / self.crash_len.max(0.01);
        self.crash_peak * (left / CRASH_EASE).clamp(0.0, 1.0)
    }

    /// Share of the crash still to go (how many pips stay drained).
    pub fn crash_bar(&self) -> f32 {
        (self.crash / self.crash_len.max(0.01)).clamp(0.0, 1.0)
    }

    /// Share of the first dose still to go (how much of the health bar stays prismatic).
    pub fn bar(&self) -> f32 {
        (self.high / DOSE).clamp(0.0, 1.0)
    }

    /// `1 + k * level` while high, its inverse while crashing.
    fn mag(&self, k: f32) -> f32 {
        let (l, c) = (self.level(), self.crash_level());
        if l > 0.0 {
            1.0 + k * Self::worth(l)
        } else if c > 0.0 {
            1.0 / (1.0 + k * Self::worth(c))
        } else {
            1.0
        }
    }

    /// What a level is worth: diminishing returns (1 dose = 1, 2 = 1.5, 4 = 2.3, 8 = 3.5).
    fn worth(level: f32) -> f32 {
        if level <= 1.0 { level } else { level.powf(0.6) }
    }

    /// How out of control the body is: 0 up to one dose, then more and more, unplayable around 6.
    pub fn chaos(&self) -> f32 {
        ((self.level() - 1.0) / 5.0).clamp(0.0, 1.0)
    }

    /// The input forced on the controls right now, if the body is glitching:
    /// (horizontal, vertical; 0 = let go of that axis).
    pub fn glitch(&self) -> Option<(f32, f32)> {
        (self.glitch_time > 0.0).then_some((self.glitch_dir, self.glitch_v))
    }

    /// A phantom jump press, once.
    pub fn take_jump(&mut self) -> bool {
        std::mem::take(&mut self.phantom_jump)
    }

    fn rand(&mut self) -> f32 {
        if self.rng == 0 {
            self.rng = 0x2545_f491;
        }
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Rolls the next twitch: how often and how long grows with the chaos. It flips what
    /// is being pressed (or presses something while nothing is), now and then jumps.
    fn twitch(&mut self, dt: f32, input: Vec2) {
        let c = self.chaos();
        self.glitch_time = (self.glitch_time - dt).max(0.0);
        if c <= 0.0 || self.glitch_time > 0.0 {
            return;
        }
        let rate = 0.2 + 3.8 * c * c;
        if self.rand() >= rate * dt {
            return;
        }
        self.glitch_time = 0.12 + (0.15 + 0.3 * c) * self.rand();
        let flip = |r: &mut Self, v: f32| if v != 0.0 { if r.rand() < 0.75 { -v } else { 0.0 } } else if r.rand() < 0.5 { 1.0 } else { -1.0 };
        self.glitch_dir = flip(self, input.x);
        self.glitch_v = if self.rand() < 0.2 + 0.5 * c { flip(self, input.y) } else { 0.0 };
        self.phantom_jump = self.rand() < 0.3 * c;
    }

    /// Walking, climbing and swimming speed.
    pub fn speed(&self) -> f32 {
        self.mag(0.7)
    }

    pub fn jump(&self) -> f32 {
        self.mag(0.2)
    }

    /// What the strength meter's spending is multiplied by.
    pub fn cost(&self) -> f32 {
        1.0 / self.mag(1.0)
    }

    /// How fast the strength meter fills back.
    pub fn recover(&self) -> f32 {
        self.mag(1.0)
    }

    /// Strength a dash needs: the meter must be full, unless the blood is up.
    pub fn dash_min(&self) -> f32 {
        if self.level() > 0.5 { 0.3 } else { 1.0 }
    }

    /// The high and the crash, for a save: seconds left, spent and peak of the high; seconds left,
    /// length and peak of the crash.
    pub fn state(&self) -> [f32; 6] {
        [self.high, self.spent, self.peak, self.crash, self.crash_len, self.crash_peak]
    }

    pub fn set_state(&mut self, s: [f32; 6]) {
        let ok = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
        let s = s.map(ok);
        (self.high, self.spent, self.peak, self.crash, self.crash_len, self.crash_peak) = (s[0], s[1], s[2], s[3], s[4], s[5]);
        // Never both at once.
        if self.high > 0.0 {
            self.crash = 0.0;
        }
        self.glitch_time = 0.0;
        self.phantom_jump = false;
    }

    /// Whether the creature at `spot` was taken. Redrawing a marker a few pixels away still counts as the same one.
    pub fn was_taken(&self, spot: IVec2) -> bool {
        self.taken.iter().any(|t| (*t - spot).abs().max_element() <= SAME_SPOT)
    }

    fn can_take(&self) -> bool {
        self.stock > 0 && self.crash <= 0.0
    }

    fn take(&mut self) {
        self.stock -= 1;
        self.high += DOSE;
    }

    fn tick(&mut self, dt: f32) {
        if self.high > 0.0 {
            self.spent += dt;
            self.high -= dt;
            self.peak = self.peak.max(self.level());
            if self.high <= 0.0 {
                self.high = 0.0;
                self.crash_len = (self.spent * CRASH_SHARE * self.peak.max(1.0).sqrt()).clamp(CRASH_MIN, CRASH_MAX);
                self.crash = self.crash_len;
                self.crash_peak = self.peak;
                self.spent = 0.0;
                self.peak = 0.0;
            }
        } else if self.crash > 0.0 {
            self.crash = (self.crash - dt).max(0.0);
        }
    }

    /// Back to clean (dying, a new life).
    fn clear(&mut self) {
        *self = Pill { stock: self.stock, taken: std::mem::take(&mut self.taken), ..default() };
    }
}

/// The prismatic color of a thing at `hue` degrees.
pub fn prism(hue: f32, light: f32) -> Color {
    Color::hsl(hue.rem_euclid(360.0), 1.0, light)
}

pub struct PillPlugin;

impl Plugin for PillPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load)
            .add_systems(Update, (rebuild, crawl, sparkle, use_pill.before(player::move_player), collect.after(player::move_player)));
    }
}

#[derive(Resource)]
struct PillAssets {
    body: Handle<Mesh>,
    shell: Handle<Mesh>,
    head: Handle<Mesh>,
    halo: Handle<Mesh>,
    leg: Handle<Mesh>,
    feeler: Handle<Mesh>,
    spark: Handle<Mesh>,
}

fn load(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    let leg = smooth_open(&[Vec2::ZERO, Vec2::new(3.0, 6.0), Vec2::new(8.0, 10.0)], 4);
    let feeler = smooth_open(&[Vec2::new(0.0, 0.0), Vec2::new(7.0, 3.0), Vec2::new(12.0, 9.0)], 4);
    commands.insert_resource(PillAssets {
        body: meshes.add(fill(&ellipse_pts(15.0, 9.0, 20), |p| 1.0 - 0.3 * (p.y / 9.0).abs().powi(2) - 0.1 * (p.x / 15.0).max(0.0))),
        shell: meshes.add(fill(&ellipse_pts(11.0, 6.0, 20), |p| 0.75 + 0.25 * (p.y / 6.0).clamp(-1.0, 1.0))),
        head: meshes.add(fill(&ellipse_pts(6.0, 5.0, 14), |p| 1.0 - 0.15 * (p.x / 6.0).abs())),
        halo: meshes.add(fill(&ellipse_pts(1.0, 1.0, 24), |p| 1.0 - p.length().min(1.0))),
        leg: meshes.add(stroke(&leg, |t| 2.2 - 1.2 * t, 0.8, 0.8, |_| 1.0)),
        feeler: meshes.add(stroke(&feeler, |t| 1.6 - 0.8 * t, 0.9, 0.9, |_| 1.0)),
        spark: meshes.add(fill(&ellipse_pts(3.0, 3.0, 8), |_| 1.0)),
    });
}

/// A pill creature, wandering around `home`.
#[derive(Component)]
struct Bug {
    spot: IVec2,
    home: Vec2,
    /// A health bug (pulsating red, heals one tick) instead of a power pill.
    health: bool,
    phase: f32,
    mats: [Handle<ColorMaterial>; 4],
    halo: Handle<ColorMaterial>,
    legs: Vec<(Entity, f32)>,
}

/// A bit of light thrown out when a pill is picked up.
#[derive(Component)]
struct Spark {
    vel: Vec2,
    age: f32,
    hue: f32,
    mat: Handle<ColorMaterial>,
}

const SPARK_LIFE: f32 = 0.9;
/// `Spark::hue` of a spark that stays red (a health bug).
const RED_SPARK: f32 = -1.0;

/// Spawns the creatures at the start, and again when the world is rebuilt.
fn rebuild(
    mut commands: Commands,
    assets: Res<PillAssets>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    terrain: Res<Terrain>,
    mut reloaded: MessageReader<WorldReloaded>,
    old: Query<Entity, With<Bug>>,
    mut started: Local<bool>,
    mut pending: Local<bool>,
) {
    if reloaded.read().count() > 0 {
        *pending = true;
        return;
    }
    if *started && !*pending {
        return;
    }
    *started = true;
    *pending = false;
    for e in &old {
        commands.entity(e).despawn();
    }
    let mut rng = Rng(0x9e3779b9);
    let bugs = terrain.pills.iter().map(|&h| (h, false)).chain(terrain.health_pills.iter().map(|&h| (h, true)));
    for (home, health) in bugs {
        let mut mat = |a: f32| materials.add(ColorMaterial { color: prism(a, 0.55), ..default() });
        let mats = [mat(0.0), mat(120.0), mat(60.0), mat(240.0)];
        let halo = materials.add(ColorMaterial { color: prism(0.0, 0.6).with_alpha(0.25), alpha_mode: AlphaMode2d::Blend, ..default() });
        let phase = rng.range(0.0, std::f32::consts::TAU);
        let layer = camera::color_layer;
        let mut legs = Vec::new();
        let root = commands.spawn((Transform::from_translation(home.extend(5.0)), Visibility::default())).id();
        let part = |commands: &mut Commands, mesh: &Handle<Mesh>, mat: &Handle<ColorMaterial>, at: Vec3, scale: Vec3| {
            commands.spawn((Mesh2d(mesh.clone()), MeshMaterial2d(mat.clone()), Transform::from_translation(at).with_scale(scale), layer(), ChildOf(root))).id()
        };
        part(&mut commands, &assets.halo, &halo, Vec3::new(0.0, 0.0, -0.1), Vec3::new(30.0, 24.0, 1.0));
        // Six legs in two tripods, a feeler pair, the shell on the back.
        for (i, x) in [9.0f32, 0.0, -9.0].into_iter().enumerate() {
            for side in [1.0f32, -1.0] {
                let group = if (i % 2 == 0) == (side > 0.0) { 0.0 } else { 1.0 };
                let leg = part(&mut commands, &assets.leg, &mats[3], Vec3::new(x, side * 6.0, 0.0), Vec3::new(1.0, side, 1.0));
                legs.push((leg, group));
            }
        }
        part(&mut commands, &assets.body, &mats[0], Vec3::new(0.0, 0.0, 0.1), Vec3::ONE);
        part(&mut commands, &assets.shell, &mats[1], Vec3::new(-2.0, 0.0, 0.2), Vec3::ONE);
        part(&mut commands, &assets.head, &mats[2], Vec3::new(17.0, 0.0, 0.1), Vec3::ONE);
        for side in [1.0f32, -1.0] {
            part(&mut commands, &assets.feeler, &mats[3], Vec3::new(20.0, side * 2.5, 0.0), Vec3::new(1.0, side, 1.0));
        }
        commands.entity(root).insert(Bug { spot: spot(home), home, health, phase, mats, halo, legs });
    }
}

/// Wanders slowly around its spot, legs walking, colors sliding around the wheel.
fn crawl(
    time: Res<Time>,
    mut bugs: Query<(&Bug, &mut Transform), Without<Player>>,
    mut parts: Query<&mut Transform, (Without<Bug>, Without<Player>)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let t = time.elapsed_secs();
    for (bug, mut tf) in &mut bugs {
        let at = |t: f32| bug.home + Vec2::new(38.0 * (0.42 * t + bug.phase).cos(), 22.0 * (0.29 * t + bug.phase * 1.7).sin());
        let (p, ahead) = (at(t), at(t + 0.1));
        let dir = ahead - p;
        tf.translation = p.extend(5.0);
        tf.rotation = Quat::from_rotation_z(dir.y.atan2(dir.x));
        for (leg, group) in &bug.legs {
            if let Ok(mut l) = parts.get_mut(*leg) {
                let side = l.scale.y;
                l.rotation = Quat::from_rotation_z(side * 0.4 * (t * 9.0 + group * std::f32::consts::PI).sin());
            }
        }
        // Iridescence: each part sits somewhere else on the wheel, and slides along it.
        if bug.health {
            // A heartbeat in red: the whole body swells and dims together.
            let beat = (0.5 + 0.5 * (t * 4.0 + bug.phase).sin()).powi(2);
            for (m, light) in bug.mats.iter().zip([0.0, 0.08, 0.04, -0.1]) {
                if let Some(mut mat) = materials.get_mut(m) {
                    mat.color = Color::hsl(0.0, 1.0, (0.3 + 0.25 * beat + light).clamp(0.1, 0.9));
                }
            }
            if let Some(mut mat) = materials.get_mut(&bug.halo) {
                mat.color = Color::hsl(0.0, 1.0, 0.5).with_alpha(0.1 + 0.25 * beat);
            }
            tf.scale = Vec3::splat(1.0 + 0.12 * beat);
            continue;
        }
        let hue = t * 140.0 + bug.phase * 60.0;
        for (m, (off, light)) in bug.mats.iter().zip([(0.0, 0.5), (110.0, 0.62), (210.0, 0.55), (300.0, 0.4)]) {
            if let Some(mut mat) = materials.get_mut(m) {
                let shimmer = 0.07 * (t * 11.0 + off).sin();
                mat.color = prism(hue + off, light + shimmer);
            }
        }
        if let Some(mut mat) = materials.get_mut(&bug.halo) {
            mat.color = prism(hue + 180.0, 0.6).with_alpha(0.16 + 0.1 * (t * 5.0 + bug.phase).sin());
        }
    }
}

fn spark_burst(commands: &mut Commands, assets: &PillAssets, materials: &mut Assets<ColorMaterial>, at: Vec2) {
    let mut rng = Rng(((at.x * 31.0 + at.y * 17.0) as i32 as u32) | 1);
    for i in 0..18 {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let hue = i as f32 * 20.0;
        let mat = materials.add(ColorMaterial { color: prism(hue, 0.6), alpha_mode: AlphaMode2d::Blend, ..default() });
        commands.spawn((
            Mesh2d(assets.spark.clone()),
            MeshMaterial2d(mat.clone()),
            Transform::from_translation(at.extend(14.0)),
            camera::color_layer(),
            Spark { vel: Vec2::from_angle(a) * rng.range(80.0, 320.0), age: 0.0, hue, mat },
        ));
    }
}

fn sparkle(
    mut commands: Commands,
    time: Res<Time>,
    mut sparks: Query<(Entity, &mut Spark, &mut Transform)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let dt = time.delta_secs();
    for (e, mut s, mut tf) in &mut sparks {
        s.age += dt;
        if s.age >= SPARK_LIFE {
            commands.entity(e).despawn();
            continue;
        }
        let v = s.vel;
        tf.translation += (v * dt).extend(0.0);
        s.vel *= (-3.0 * dt).exp();
        let k = 1.0 - s.age / SPARK_LIFE;
        tf.scale = Vec3::splat(0.5 + 1.5 * k);
        if let Some(mut m) = materials.get_mut(&s.mat) {
            m.color = if s.hue == RED_SPARK { Color::hsl(0.0, 1.0, 0.55) } else { prism(s.hue + s.age * 400.0, 0.6) }.with_alpha(k);
        }
    }
}

/// Touching a creature takes it into the pocket.
fn collect(
    mut commands: Commands,
    assets: Res<PillAssets>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut flash: ResMut<ScreenFlash>,
    mut autosave: MessageWriter<AutoSave>,
    player: Single<(&mut Player, &Transform)>,
    bugs: Query<(Entity, &Bug, &Transform), Without<Player>>,
) {
    let (mut p, tf) = player.into_inner();
    for (e, b, _) in &bugs {
        // Picked up before (a loaded game): gone from the map.
        if p.pill.was_taken(b.spot) {
            commands.entity(e).despawn();
        }
    }
    if p.dead.is_some() {
        return;
    }
    for (e, b, bug) in &bugs {
        if p.pill.was_taken(b.spot) || bug.translation.truncate().distance(tf.translation.truncate()) >= PICKUP {
            continue;
        }
        if b.health {
            // Eaten on the spot, only if it is needed: otherwise it waits.
            if p.hp >= p.max_hp {
                continue;
            }
            p.hp += 1;
            p.shadow = p.shadow.min((p.max_hp - p.hp) as f32);
            p.pill.taken.push(b.spot);
            autosave.write(AutoSave);
            commands.entity(e).despawn();
            let at = bug.translation.truncate();
            for i in 0..10 {
                let a = i as f32 * std::f32::consts::TAU / 10.0;
                let mat = materials.add(ColorMaterial { color: Color::hsl(0.0, 1.0, 0.55), alpha_mode: AlphaMode2d::Blend, ..default() });
                commands.spawn((
                    Mesh2d(assets.spark.clone()),
                    MeshMaterial2d(mat.clone()),
                    Transform::from_translation(at.extend(14.0)),
                    camera::color_layer(),
                    Spark { vel: Vec2::from_angle(a) * 160.0, age: 0.0, hue: RED_SPARK, mat },
                ));
            }
            flash.flash(Color::srgb(1.0, 0.3, 0.3), 0.2);
            continue;
        }
        if p.pill.stock < MAX_STOCK {
            p.pill.stock += 1;
            p.pill.taken.push(b.spot);
            // Very important items: a checkpoint.
            autosave.write(AutoSave);
            commands.entity(e).despawn();
            spark_burst(&mut commands, &assets, &mut materials, bug.translation.truncate());
            flash.flash(Color::srgb(0.7, 0.8, 1.0), 0.2);
        }
    }
}

/// `Z`: swallows a pill, and runs the clock.
fn use_pill(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut shake: ResMut<Shake>,
    mut flash: ResMut<ScreenFlash>,
    mut autosave: MessageWriter<AutoSave>,
    mut player: Single<&mut Player>,
) {
    let input = Vec2::new(
        keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) as i32 as f32 - keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) as i32 as f32,
        keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) as i32 as f32 - keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) as i32 as f32,
    );
    player.pill.tick(time.delta_secs());
    player.pill.twitch(time.delta_secs(), input);
    if player.dead.is_some() {
        player.pill.clear();
        return;
    }
    if keys.just_pressed(KeyCode::KeyZ) && player.pill.can_take() {
        player.pill.take();
        autosave.write(AutoSave);
        shake.add(0.25);
        flash.flash(Color::srgb(0.9, 0.5, 1.0), 0.6);
    }
}
