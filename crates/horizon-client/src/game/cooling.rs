//! Cool Mode: keeps the phone from heating up. The device says how hot it is (iOS thermal
//! state); when it is warm the game draws fewer frames and fewer pixels, and relaxes only after
//! a minute of cooler readings so it does not flicker. In Eco the lower limits are always on.
//! The player's own options are never changed: these are caps over them.
use super::Game;
use std::cell::Cell;

/// What the cooling asks for: a frame cap (0 none) and a share of the resolution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Cap {
    pub fps: i32,
    pub scale: f32,
}
const NONE: Cap = Cap { fps: 0, scale: 1.0 };
/// Cooler readings must last this long (ms) before the limits ease one step.
const RELAX_MS: i64 = 60_000;
/// How often (ms) the device is asked.
const POLL_MS: i64 = 2_000;

/// The limits for a Cool Mode (0 off, 1 auto, 2 eco) and a thermal state (0..=3).
fn cap_for(mode: u8, thermal: u8) -> Cap {
    let hot = match thermal {
        0 => NONE,
        1 => Cap {
            fps: 45,
            scale: 0.85,
        },
        2 => Cap {
            fps: 30,
            scale: 0.7,
        },
        _ => Cap {
            fps: 20,
            scale: 0.5,
        },
    };
    match mode {
        0 => NONE,
        2 => {
            let eco = Cap {
                fps: 30,
                scale: 0.75,
            };
            Cap {
                fps: [eco.fps, hot.fps]
                    .into_iter()
                    .filter(|f| *f > 0)
                    .min()
                    .unwrap_or(0),
                scale: eco.scale.min(hot.scale),
            }
        }
        _ => hot,
    }
}

/// The thermal state held: it rises at once, and falls one step after `RELAX_MS` of lower
/// readings.
fn hold(state: &mut u8, lower_since: &mut Option<i64>, raw: u8, now_ms: i64) {
    if raw >= *state {
        *state = raw;
        *lower_since = None;
    } else {
        let since = *lower_since.get_or_insert(now_ms);
        if now_ms - since >= RELAX_MS {
            *state -= 1;
            *lower_since = None;
        }
    }
}

thread_local! {
    static HELD: Cell<u8> = const { Cell::new(0) };
    static LOWER_SINCE: Cell<Option<i64>> = const { Cell::new(None) };
    static LAST_POLL: Cell<i64> = const { Cell::new(i64::MIN) };
    static CAP: Cell<Cap> = const { Cell::new(NONE) };
}

/// The frame cap in force (0: none), applied in a world.
pub(super) fn fps_cap() -> i32 {
    CAP.get().fps
}

/// The share of the resolution in force.
pub(super) fn scale() -> f32 {
    CAP.get().scale
}

/// A line for the HUD: how hot the phone is and the frame cap in force.
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub(super) fn status() -> (String, u32) {
    let (name, color) = match HELD.get() {
        0 => ("OK", [110, 230, 110]),
        1 => ("Warm", [250, 220, 70]),
        2 => ("Hot", [255, 150, 50]),
        _ => ("Critical", [235, 70, 70]),
    };
    let cap = CAP.get().fps;
    let text = if cap > 0 {
        format!("{name} {cap}fps")
    } else {
        name.to_owned()
    };
    (
        text,
        horizon_client::hud::rgba(color[0], color[1], color[2], 240),
    )
}

impl Game<'_> {
    /// Each frame: now and then, ask how hot the device is and set the caps. A change of the
    /// resolution share reaches the renderer here.
    pub(super) fn cool_tick(&mut self) {
        let now = self.started.elapsed().as_millis() as i64;
        if now - LAST_POLL.get() < POLL_MS {
            return;
        }
        LAST_POLL.set(now);
        let mode = self.settings.touch.cool;
        // Only iOS says how hot it is (horizon_ios is an iOS-only dependency).
        #[cfg(target_os = "ios")]
        let raw = if mode == 0 {
            0
        } else {
            horizon_ios::thermal_state().unwrap_or(0)
        };
        #[cfg(not(target_os = "ios"))]
        let raw = 0;
        let (mut state, mut since) = (HELD.get(), LOWER_SINCE.get());
        hold(&mut state, &mut since, raw, now);
        HELD.set(state);
        LOWER_SINCE.set(since);
        let cap = cap_for(mode, state);
        if cap != CAP.get() {
            CAP.set(cap);
            let s = &self.settings.horizon;
            if let Some(r) = self.renderer.as_mut() {
                r.set_world_post(s.resolution_scale as f32 * cap.scale / 100.0, s.fxaa);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_follow_mode_and_heat() {
        assert_eq!(cap_for(0, 3), NONE);
        assert_eq!(cap_for(1, 0), NONE);
        assert_eq!(cap_for(1, 2).fps, 30);
        assert_eq!(cap_for(1, 3).fps, 20);
        // Eco is always on, and a hot phone lowers it further.
        assert_eq!(cap_for(2, 0).fps, 30);
        assert_eq!(cap_for(2, 1).fps, 30);
        assert_eq!(cap_for(2, 3).fps, 20);
        assert!(cap_for(2, 3).scale < cap_for(2, 0).scale);
    }

    #[test]
    fn heat_rises_at_once_and_falls_after_a_minute() {
        let (mut s, mut since) = (0, None);
        hold(&mut s, &mut since, 2, 0);
        assert_eq!(s, 2);
        hold(&mut s, &mut since, 0, 10_000);
        hold(&mut s, &mut since, 0, 69_000);
        assert_eq!(s, 2, "not yet a minute");
        hold(&mut s, &mut since, 0, 71_000);
        assert_eq!(s, 1);
        // A hotter reading restarts the wait.
        hold(&mut s, &mut since, 1, 72_000);
        assert_eq!((s, since), (1, None));
    }
}
