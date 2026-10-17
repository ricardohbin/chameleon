//! Control schemes (`settings::Scheme`) and gamepads.
//!
//! The game reads the keyboard everywhere (`ButtonInput<KeyCode>`), so this runs
//! first in `PreUpdate` and adapts the input to it:
//! - a gamepad's sticks, d-pad and buttons are pressed as the keys they stand
//!   for (left stick = A/D/W/S, south = Space, west = F slash, east = Q spit,
//!   left trigger = G shield, north = E, left bumper = K dash);
//! - in the dual scheme the arrows only aim: they are released before the
//!   game sees them, so they don't move the chameleon any more.
//!
//! The aim is published in `AimInput` for `player::move_player`: the mouse
//! (`mouse`), or a direction (`dir`, zero = not chosen yet: the facing). The
//! directional scheme keeps the last direction pressed; the dual one the last
//! direction of the arrows / right stick.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::{
    menu::Menu,
    settings::{Scheme, Settings},
};

pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AimInput>().add_systems(
            PreUpdate,
            adapt.after(crate::menu::input).after(bevy::input::InputSystems).before(crate::level::restart),
        );
    }
}

/// Where the chameleon aims this frame.
#[derive(Resource)]
pub struct AimInput {
    /// Follow the mouse cursor.
    pub mouse: bool,
    /// Else this direction (unit; zero = none chosen yet).
    pub dir: Vec2,
}

impl Default for AimInput {
    fn default() -> Self {
        AimInput { mouse: true, dir: Vec2::ZERO }
    }
}

const STICK_MOVE: f32 = 0.35;
const STICK_AIM: f32 = 0.4;

const BUTTONS: [(GamepadButton, KeyCode); 8] = [
    (GamepadButton::South, KeyCode::Space),
    (GamepadButton::West, KeyCode::KeyF),
    (GamepadButton::RightTrigger2, KeyCode::KeyF),
    (GamepadButton::East, KeyCode::KeyQ),
    (GamepadButton::LeftTrigger2, KeyCode::KeyG),
    (GamepadButton::North, KeyCode::KeyE),
    (GamepadButton::RightTrigger, KeyCode::KeyC),
    (GamepadButton::LeftTrigger, KeyCode::KeyK),
];

const LEFT: [KeyCode; 2] = [KeyCode::KeyA, KeyCode::ArrowLeft];
const RIGHT: [KeyCode; 2] = [KeyCode::KeyD, KeyCode::ArrowRight];
const UP: [KeyCode; 2] = [KeyCode::KeyW, KeyCode::ArrowUp];
const DOWN: [KeyCode; 2] = [KeyCode::KeyS, KeyCode::ArrowDown];

/// The direction held on some keys (y up).
fn arrows(keys: &ButtonInput<KeyCode>, l: &[KeyCode], r: &[KeyCode], u: &[KeyCode], d: &[KeyCode]) -> Vec2 {
    let axis = |plus: &[KeyCode], minus: &[KeyCode]| keys.any_pressed(plus.iter().copied()) as i32 as f32 - keys.any_pressed(minus.iter().copied()) as i32 as f32;
    Vec2::new(axis(r, l), axis(u, d))
}

fn adapt(
    settings: Res<Settings>,
    menu: Res<Menu>,
    pads: Query<&Gamepad>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut aim: ResMut<AimInput>,
    mut synthetic: Local<HashSet<KeyCode>>,
    mut last: Local<Vec2>,
) {
    if menu.open {
        // The menu has the keys; let go of whatever the pad was holding.
        for k in synthetic.drain() {
            keys.release(k);
        }
        return;
    }

    // --- Gamepads, as keys ---
    let mut want: HashSet<KeyCode> = HashSet::new();
    let (mut left, mut right) = (Vec2::ZERO, Vec2::ZERO);
    for pad in &pads {
        let dpad = pad.dpad();
        let stick = pad.left_stick();
        let m = if stick.length() > dpad.length() { stick } else { dpad };
        left += m;
        right += pad.right_stick();
        for (button, key) in BUTTONS {
            if pad.pressed(button) {
                want.insert(key);
            }
        }
    }
    if left.x < -STICK_MOVE {
        want.insert(KeyCode::KeyA);
    } else if left.x > STICK_MOVE {
        want.insert(KeyCode::KeyD);
    }
    if left.y > STICK_MOVE + 0.15 {
        want.insert(KeyCode::KeyW);
    } else if left.y < -(STICK_MOVE + 0.15) {
        want.insert(KeyCode::KeyS);
    }
    for k in synthetic.difference(&want).copied().collect::<Vec<_>>() {
        keys.release(k);
    }
    for k in &want {
        keys.press(*k);
    }
    *synthetic = want;

    // --- The aim ---
    let stick_aim = (right.length() > STICK_AIM).then(|| right.normalize());
    match settings.scheme {
        Scheme::MouseKeyboard => {
            // A right stick takes over while it is pushed.
            aim.mouse = stick_aim.is_none();
            if let Some(d) = stick_aim {
                *last = d;
            }
            aim.dir = *last;
        }
        Scheme::Directional => {
            // The last direction pressed: any movement key (or the left stick).
            let pressed = arrows(&keys, &LEFT, &RIGHT, &UP, &DOWN);
            let v = if pressed != Vec2::ZERO { pressed } else { stick_aim.unwrap_or(left) };
            if v.length() > STICK_AIM {
                *last = v.normalize();
            }
            aim.mouse = false;
            aim.dir = *last;
        }
        Scheme::Dual => {
            // WASD moves, the arrows aim (and no longer walk).
            let pressed = arrows(&keys, &[KeyCode::ArrowLeft], &[KeyCode::ArrowRight], &[KeyCode::ArrowUp], &[KeyCode::ArrowDown]);
            let v = if pressed != Vec2::ZERO { pressed } else { stick_aim.unwrap_or(Vec2::ZERO) };
            if v != Vec2::ZERO {
                *last = v.normalize();
            }
            for k in [KeyCode::ArrowLeft, KeyCode::ArrowRight, KeyCode::ArrowUp, KeyCode::ArrowDown] {
                keys.reset(k);
            }
            aim.mouse = false;
            aim.dir = *last;
        }
    }
}
