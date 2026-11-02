//! Touch controls (Horizon's own, for iOS), after Bedrock Edition's with split controls: a
//! floating joystick on the left moves, a drag anywhere else turns the view, a tap there uses or
//! places (or hits the mob under the crosshair) and a hold breaks. Buttons jump, sneak, open the
//! inventory, change the perspective and pause, and Use (shown while the crosshair is on an
//! entity, Bedrock's Interact) trades, feeds or rides; the hotbar's slots are tapped, and held to
//! drop.
//!
//! The controls are a virtual controller: each frame gives a [`PadState`] the game handles as a
//! pad's (analog movement, toggle sneak, held and tapped triggers), plus the view's turn and the
//! hotbar slot chosen. Positions are GUI pixels, as the HUD draws them.
use crate::gamepad::{Button, PadState};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A held finger turns into breaking after this long without moving far.
pub const HOLD_TO_BREAK: Duration = Duration::from_millis(300);
/// A finger lifted sooner than this, without moving far, is a tap.
pub const TAP: Duration = Duration::from_millis(250);
/// The pause button held this long shows the debug screen (F3) instead of pausing.
pub const HOLD_FOR_DEBUG: Duration = Duration::from_millis(500);
/// A hotbar slot held this long drops its item.
pub const HOLD_TO_DROP: Duration = Duration::from_millis(600);
/// How far (GUI pixels) a finger may move and still tap or hold.
pub const SLOP: f32 = 6.0;
/// The joystick's reach from where the thumb landed (GUI pixels).
pub const STICK_RADIUS: f32 = 26.0;
/// Past this tilt, mostly forward, the joystick sprints.
const SPRINT_TILT: f32 = 0.9;

/// A rectangle in GUI pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn contains(&self, [x, y]: [f32; 2]) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// The on-screen buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TouchButton {
    Jump,
    Sneak,
    Sprint,
    Inventory,
    Chat,
    Perspective,
    Pause,
    Use,
    Swap,
}

impl TouchButton {
    pub const ALL: [TouchButton; 9] = [
        TouchButton::Jump,
        TouchButton::Sneak,
        TouchButton::Sprint,
        TouchButton::Inventory,
        TouchButton::Chat,
        TouchButton::Perspective,
        TouchButton::Pause,
        TouchButton::Use,
        TouchButton::Swap,
    ];
    /// What it presses on the controller (docs/controllers.md's Bedrock layout).
    pub fn press(self, pad: &mut PadState) {
        match self {
            TouchButton::Jump => pad.set(Button::South, true),
            TouchButton::Sneak => pad.set(Button::RightStick, true),
            TouchButton::Sprint => pad.set(Button::LeftStick, true),
            TouchButton::Chat => pad.set(Button::DPadRight, true),
            TouchButton::Inventory => pad.set(Button::North, true),
            TouchButton::Perspective => pad.set(Button::DPadUp, true),
            TouchButton::Pause => pad.set(Button::Start, true),
            TouchButton::Use => pad.triggers[0] = 1.0,
            TouchButton::Swap => pad.set(Button::West, true),
        }
    }
}

/// Where the controls are on a GUI `width` x `height` screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TouchLayout {
    pub width: f32,
    pub height: f32,
    /// Kept clear at the left and right edges: a phone's notch or rounded corners.
    pub left: f32,
    pub right: f32,
    /// Kept clear at the top (the status bar's area in portrait, a corner's curve).
    pub top: f32,
    /// The buttons' size, 1 being Bedrock's.
    pub scale: f32,
    /// The player's own placement of each control (see [`CUSTOM_SLOTS`]): x and y offsets in GUI
    /// pixels, then a size multiplier.
    pub custom: [[f32; 3]; CUSTOM_SLOTS],
}

/// Controls the player can move and resize: the nine buttons (in [`TouchButton::ALL`]'s order),
/// then close, Shift, hide keyboard, the joystick and the F3 combinations button.
pub const CUSTOM_SLOTS: usize = 16;
/// The slot of the close button, Shift toggle, keyboard button and joystick.
pub const SLOT_CLOSE: usize = 9;
pub const SLOT_SHIFT: usize = 10;
pub const SLOT_KEYBOARD: usize = 11;
pub const SLOT_STICK: usize = 12;
/// How near (GUI pixels) a control must be to another's edge to line up with it.
pub const ALIGN_REACH: f32 = 6.0;
/// The F3 button that opens the debug key combinations.
pub const SLOT_COMBOS: usize = 13;
/// The frame rate and coordinates readout.
pub const SLOT_INFO: usize = 14;
/// The zoom button.
pub const SLOT_ZOOM: usize = 15;
/// The zoom levels the zoom button can have, from [`TouchSettings::zoom_level`].
pub const ZOOM_FACTORS: [f32; 6] = [2.0, 3.0, 4.0, 6.0, 8.0, 10.0];
/// A control left alone: no offset, its normal size.
pub const NO_CUSTOM: [f32; 3] = [0.0, 0.0, 1.0];
/// The size multiplier a control may be given.
pub const CUSTOM_SIZE: (f32, f32) = (0.5, 2.5);

impl TouchLayout {
    /// A phone's layout without its real safe area: an iPhone's notch is about 36 GUI pixels.
    pub fn new(width: f32, height: f32) -> Self {
        let side = if width >= 480.0 { 36.0 } else { 8.0 };
        Self::with(width, height, [side, 0.0, side], 1.0)
    }
    /// `insets`: the safe area's left, top and right margins in GUI pixels; `scale`: the
    /// buttons' size setting.
    pub fn with(width: f32, height: f32, [left, top, right]: [f32; 3], scale: f32) -> Self {
        Self {
            width,
            height,
            left: left.max(8.0),
            right: right.max(8.0),
            top: top.max(0.0) + 4.0,
            scale,
            custom: [NO_CUSTOM; CUSTOM_SLOTS],
        }
    }
    /// The same layout with the player's placements.
    pub fn with_custom(mut self, custom: [[f32; 3]; CUSTOM_SLOTS]) -> Self {
        self.custom = custom;
        self
    }
    /// The slot of a button in [`Self::custom`].
    pub fn slot_of(button: TouchButton) -> usize {
        TouchButton::ALL
            .iter()
            .position(|b| *b == button)
            .unwrap_or(0)
    }
    /// Where slot `slot` is without the player's placement.
    pub fn slot_base(&self, slot: usize) -> Rect {
        match slot {
            0..=8 => self.base_button(TouchButton::ALL[slot]),
            SLOT_CLOSE => self.base_close(),
            SLOT_SHIFT => {
                let close = self.base_close();
                Rect::new(close.x, close.y + close.h + 5.0, 20.0, 20.0)
            }
            SLOT_KEYBOARD => {
                let close = self.base_close();
                Rect::new(close.x - close.w - 5.0, close.y, 20.0, 20.0)
            }
            SLOT_ZOOM => {
                let small = (18.0 * self.scale).round();
                let gap = (5.0 * self.scale).round();
                Rect::new(
                    self.width - self.right - small - 4.0 * (small + gap),
                    self.top,
                    small,
                    small,
                )
            }
            SLOT_INFO => Rect::new(self.left, self.top + 22.0, 64.0, 62.0),
            SLOT_COMBOS => {
                let small = (18.0 * self.scale).round();
                let gap = (5.0 * self.scale).round();
                Rect::new(
                    self.width - self.right - small - 3.0 * (small + gap),
                    self.top,
                    small,
                    small,
                )
            }
            _ => {
                let r = STICK_RADIUS * self.scale + 4.0;
                let c = self.base_rest();
                Rect::new(c[0] - r, c[1] - r, 2.0 * r, 2.0 * r)
            }
        }
    }
    /// Slot `slot` with the player's placement, not yet kept on the screen.
    pub fn slot_raw(&self, slot: usize) -> Rect {
        let base = self.slot_base(slot);
        let [dx, dy, k] = self.custom.get(slot).copied().unwrap_or(NO_CUSTOM);
        let (w, h) = (base.w * k, base.h * k);
        Rect::new(
            base.x + (base.w - w) / 2.0 + dx,
            base.y + (base.h - h) / 2.0 + dy,
            w,
            h,
        )
    }
    /// Slot `slot` as it is drawn and touched: placed by the player, kept on the screen.
    pub fn slot_rect(&self, slot: usize) -> Rect {
        let r = self.slot_raw(slot);
        Rect::new(
            r.x.clamp(0.0, (self.width - r.w).max(0.0)),
            r.y.clamp(0.0, (self.height - r.h).max(0.0)),
            r.w,
            r.h,
        )
    }
    /// Lines slot `slot` up with the controls near it: its top, middle or bottom edge with
    /// theirs, and the same sideways, when they are within [`ALIGN_REACH`] GUI pixels. Menu
    /// buttons only line up with each other, and the joystick with nothing.
    pub fn align(&mut self, slot: usize) {
        if slot >= CUSTOM_SLOTS || slot == SLOT_STICK {
            return;
        }
        let menu = |n: usize| (SLOT_CLOSE..=SLOT_KEYBOARD).contains(&n);
        let me = self.slot_rect(slot);
        let mut best_x: Option<f32> = None;
        let mut best_y: Option<f32> = None;
        let closer = |best: Option<f32>, d: f32| {
            d.abs() <= ALIGN_REACH && best.is_none_or(|b| d.abs() < b.abs())
        };
        for other in 0..CUSTOM_SLOTS {
            if other == slot || other == SLOT_STICK || menu(other) != menu(slot) {
                continue;
            }
            let o = self.slot_rect(other);
            for (mine, theirs) in [
                (me.y, o.y),
                (me.y + me.h / 2.0, o.y + o.h / 2.0),
                (me.y + me.h, o.y + o.h),
            ] {
                if closer(best_y, theirs - mine) {
                    best_y = Some(theirs - mine);
                }
            }
            for (mine, theirs) in [
                (me.x, o.x),
                (me.x + me.w / 2.0, o.x + o.w / 2.0),
                (me.x + me.w, o.x + o.w),
            ] {
                if closer(best_x, theirs - mine) {
                    best_x = Some(theirs - mine);
                }
            }
        }
        self.custom[slot][0] += best_x.unwrap_or(0.0);
        self.custom[slot][1] += best_y.unwrap_or(0.0);
        self.keep_on_screen(slot);
    }
    /// Pulls slot `slot`'s offsets back so it stays on the screen.
    pub fn keep_on_screen(&mut self, slot: usize) {
        if slot < CUSTOM_SLOTS {
            let (raw, kept) = (self.slot_raw(slot), self.slot_rect(slot));
            self.custom[slot][0] += kept.x - raw.x;
            self.custom[slot][1] += kept.y - raw.y;
        }
    }
    /// Hotbar slot `n` (0..9), as vanilla draws the bar: 182x22 centred at the bottom.
    pub fn hotbar_slot(&self, n: usize) -> Rect {
        let left = (self.width / 2.0).floor() - 91.0;
        Rect::new(left + n as f32 * 20.0 + 1.0, self.height - 22.0, 20.0, 22.0)
    }
    /// Bedrock's layout: jump on the right with sneak under it and sprint to its left, the
    /// small chat, perspective and pause buttons along the top, "..." at the hotbar's end.
    pub fn button(&self, button: TouchButton) -> Rect {
        self.slot_rect(Self::slot_of(button))
    }
    fn base_button(&self, button: TouchButton) -> Rect {
        let (w, h, k) = (self.width, self.height, self.scale);
        let big = (30.0 * k).round();
        let small = (18.0 * k).round();
        let gap = (5.0 * k).round();
        let x = w - self.right - big;
        // Sneak under jump, both above the hotbar's height.
        let y = ((h * 0.55).floor()).min(h - 2.0 * big - gap - 8.0);
        let jump = Rect::new(x, y, big, big);
        let top = |n: f32| {
            Rect::new(
                w - self.right - small - n * (small + gap),
                self.top,
                small,
                small,
            )
        };
        match button {
            TouchButton::Jump => jump,
            TouchButton::Sneak => Rect::new(jump.x, jump.y + big + gap, big, big),
            TouchButton::Sprint => Rect::new(jump.x - big - gap, jump.y, big, big),
            TouchButton::Inventory => {
                let slot = self.hotbar_slot(8);
                Rect::new(slot.x + slot.w + 3.0, h - 22.0, 22.0, 22.0)
            }
            TouchButton::Pause => top(0.0),
            TouchButton::Chat => top(1.0),
            TouchButton::Perspective => top(2.0),
            TouchButton::Use => Rect::new(jump.x, jump.y - big - gap, big, big),
            TouchButton::Swap => Rect::new(jump.x - big - gap, jump.y - big - gap, big, big),
        }
    }
    /// The joystick's reach from where the thumb landed.
    pub fn stick_radius(&self) -> f32 {
        STICK_RADIUS * self.scale * self.custom[SLOT_STICK][2]
    }
    /// Where a thumb starts the joystick: the left part of the screen, below the top.
    pub fn joystick_zone(&self) -> Rect {
        let top = self.top + 24.0;
        Rect::new(0.0, top, self.width * 0.4, self.height - top)
    }
    /// The joystick's centre while no thumb is on it.
    pub fn joystick_rest(&self) -> [f32; 2] {
        let r = self.slot_rect(SLOT_STICK);
        [r.x + r.w / 2.0, r.y + r.h / 2.0]
    }
    fn base_rest(&self) -> [f32; 2] {
        let r = STICK_RADIUS * self.scale;
        [self.left + r + 14.0, self.height - r - 30.0]
    }
    /// The close button over menus (Escape), top right.
    pub fn close(&self) -> Rect {
        self.slot_rect(SLOT_CLOSE)
    }
    fn base_close(&self) -> Rect {
        Rect::new(self.width - self.right - 20.0, self.top, 20.0, 20.0)
    }
    /// The F3 button, left of the perspective button: opens the debug key combinations.
    pub fn combos(&self) -> Rect {
        self.slot_rect(SLOT_COMBOS)
    }
    /// The button that hides the system keyboard, left of the close button: the iPhone's
    /// keyboard has no key of its own for it.
    pub fn zoom_button(&self) -> Rect {
        self.slot_rect(SLOT_ZOOM)
    }
    pub fn keyboard_hide(&self) -> Rect {
        self.slot_rect(SLOT_KEYBOARD)
    }
    /// The Shift toggle under it in inventories: taps quick-move while it is on.
    pub fn shift(&self) -> Rect {
        self.slot_rect(SLOT_SHIFT)
    }
}

/// What a finger does, from where it went down.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Role {
    Hotbar {
        slot: usize,
        since: Instant,
        dropped: bool,
    },
    /// A button, and whether a frame has seen it held.
    Button(TouchButton, bool),
    Joystick {
        origin: [f32; 2],
        at: [f32; 2],
    },
    Look {
        since: Instant,
        start: [f32; 2],
        last: [f32; 2],
        breaking: bool,
    },
    /// Down while a screen opened: ignored until lifted.
    Ignored,
}

/// One frame of the controls.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TouchFrame {
    /// Movement, the buttons held, and breaking (the right trigger pulled).
    pub pad: PadState,
    /// The view's turn since the last frame, in GUI pixels.
    pub look: [f32; 2],
    /// A tap on the view: use the held item or place, or attack the mob under the crosshair.
    pub tap: bool,
    /// The hotbar slot tapped.
    pub hotbar: Option<usize>,
    /// A hotbar slot was held: drop the selected item.
    pub drop: bool,
    /// Sneak is held in descend mode: sneak (fly or swim down) while it is.
    pub sneak_held: bool,
    /// The pause button was held: show or hide the debug screen (F3).
    pub debug: bool,
}

/// The fingers on the screen and what each does.
#[derive(Debug)]
pub struct TouchControls {
    pub layout: TouchLayout,
    /// The crosshair is on an entity: the Use button shows.
    pub use_shown: bool,
    /// Flying or in water: Sneak descends while held instead of toggling (Bedrock's fly down).
    pub descend: bool,
    fingers: HashMap<u64, Role>,
    look: [f32; 2],
    tap: bool,
    hotbar: Option<usize>,
    drop: bool,
    /// Buttons lifted before any frame saw them: iOS often delivers a touch near the screen's
    /// edge only when it ends, so its start and end come between two frames.
    quick: Vec<TouchButton>,
    /// The finger on the pause button, since when, and whether its hold showed the debug screen:
    /// pause acts when lifted, so a hold can do something else.
    pause: Option<(u64, Instant, bool)>,
    debug: bool,
}

impl Default for TouchControls {
    fn default() -> Self {
        Self {
            layout: TouchLayout::new(0.0, 0.0),
            use_shown: false,
            descend: false,
            fingers: HashMap::new(),
            look: [0.0; 2],
            tap: false,
            hotbar: None,
            drop: false,
            quick: Vec::new(),
            pause: None,
            debug: false,
        }
    }
}

fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

impl TouchControls {
    /// The finger is one of the controls' (down since the game was playing).
    pub fn owns(&self, id: u64) -> bool {
        self.fingers.contains_key(&id)
    }

    pub fn start(&mut self, id: u64, at: [f32; 2], now: Instant) {
        let layout = self.layout;
        let role = if let Some(slot) = (0..9).find(|&n| layout.hotbar_slot(n).contains(at)) {
            self.hotbar = Some(slot);
            Role::Hotbar {
                slot,
                since: now,
                dropped: false,
            }
        } else if let Some(button) = TouchButton::ALL
            .into_iter()
            .find(|&b| self.shown(b) && layout.button(b).contains(at))
        {
            if button == TouchButton::Pause {
                self.pause = Some((id, now, false));
            }
            Role::Button(button, false)
        } else if layout.joystick_zone().contains(at) && !self.has_joystick() {
            Role::Joystick { origin: at, at }
        } else {
            Role::Look {
                since: now,
                start: at,
                last: at,
                breaking: false,
            }
        };
        self.fingers.insert(id, role);
    }

    pub fn moved(&mut self, id: u64, to: [f32; 2]) {
        match self.fingers.get_mut(&id) {
            Some(Role::Joystick { at, .. }) => *at = to,
            Some(Role::Look { last, .. }) => {
                self.look[0] += to[0] - last[0];
                self.look[1] += to[1] - last[1];
                *last = to;
            }
            _ => {}
        }
    }

    pub fn end(&mut self, id: u64, at: [f32; 2], now: Instant) {
        self.moved(id, at);
        match self.fingers.remove(&id) {
            Some(Role::Look {
                since,
                start,
                breaking: false,
                ..
            }) if now.duration_since(since) < TAP && distance(start, at) <= SLOP => {
                self.tap = true;
            }
            Some(Role::Button(TouchButton::Pause, _)) => {
                if let Some((_, _, false)) = self.pause.take() {
                    self.quick.push(TouchButton::Pause);
                }
            }
            Some(Role::Button(button, false)) => self.quick.push(button),
            _ => {}
        }
    }

    /// A screen opened: every finger down stops acting until it is lifted, and nothing pending
    /// carries over.
    pub fn release_all(&mut self) {
        for role in self.fingers.values_mut() {
            *role = Role::Ignored;
        }
        self.look = [0.0; 2];
        self.tap = false;
        self.hotbar = None;
        self.drop = false;
        self.quick.clear();
        self.pause = None;
        self.debug = false;
    }

    /// The button is on screen.
    pub fn shown(&self, button: TouchButton) -> bool {
        button != TouchButton::Use || self.use_shown
    }

    fn has_joystick(&self) -> bool {
        self.fingers
            .values()
            .any(|r| matches!(r, Role::Joystick { .. }))
    }

    /// The joystick's tilt, right and up positive, at most 1 long.
    pub fn stick(&self) -> Option<([f32; 2], [f32; 2])> {
        self.fingers.values().find_map(|r| match *r {
            Role::Joystick { origin, at } => {
                let r = self.layout.stick_radius();
                let mut v = [(at[0] - origin[0]) / r, (origin[1] - at[1]) / r];
                let len = (v[0] * v[0] + v[1] * v[1]).sqrt();
                if len > 1.0 {
                    v = [v[0] / len, v[1] / len];
                }
                Some((origin, v))
            }
            _ => None,
        })
    }

    /// A button is held (drawn highlighted).
    pub fn held(&self, button: TouchButton) -> bool {
        self.fingers
            .values()
            .any(|r| matches!(r, Role::Button(b, _) if *b == button))
    }

    /// This frame's controls; the turn, taps and slot changes since the last are taken.
    pub fn frame(&mut self, now: Instant) -> TouchFrame {
        let mut pad = PadState::default();
        let mut sneak_held = false;
        let descend = self.descend;
        let mut press = |button: TouchButton, pad: &mut PadState| {
            if descend && button == TouchButton::Sneak {
                sneak_held = true;
            } else {
                button.press(pad);
            }
        };
        for button in std::mem::take(&mut self.quick) {
            press(button, &mut pad);
        }
        for role in self.fingers.values_mut() {
            match role {
                // Pause presses when lifted (`end`).
                Role::Button(TouchButton::Pause, _) => {}
                Role::Button(button, seen) => {
                    press(*button, &mut pad);
                    *seen = true;
                }
                Role::Look {
                    since,
                    start,
                    last,
                    breaking,
                } => {
                    if !*breaking
                        && now.duration_since(*since) >= HOLD_TO_BREAK
                        && distance(*start, *last) <= SLOP
                    {
                        *breaking = true;
                    }
                    if *breaking {
                        pad.triggers[1] = 1.0;
                    }
                }
                Role::Hotbar { since, dropped, .. } => {
                    if !*dropped && now.duration_since(*since) >= HOLD_TO_DROP {
                        *dropped = true;
                        self.drop = true;
                    }
                }
                Role::Joystick { .. } | Role::Ignored => {}
            }
        }
        if let Some((_, since, fired)) = self.pause.as_mut()
            && !*fired
            && now.duration_since(*since) >= HOLD_FOR_DEBUG
        {
            *fired = true;
            self.debug = true;
        }
        if let Some((_, v)) = self.stick() {
            pad.left = v;
            let tilt = (v[0] * v[0] + v[1] * v[1]).sqrt();
            if tilt >= SPRINT_TILT && v[1] > v[0].abs() {
                pad.set(Button::LeftStick, true);
            }
        }
        TouchFrame {
            pad,
            look: std::mem::take(&mut self.look),
            tap: std::mem::take(&mut self.tap),
            hotbar: self.hotbar.take(),
            drop: std::mem::take(&mut self.drop),
            sneak_held,
            debug: std::mem::take(&mut self.debug),
        }
    }

    /// The fingers on the hotbar, for drawing; and whether the slot `n` is held.
    pub fn hotbar_held(&self, n: usize) -> bool {
        self.fingers
            .values()
            .any(|r| matches!(r, Role::Hotbar { slot, .. } if *slot == n))
    }
}

/// The touch controls' settings file, next to options.txt.
pub const FILE: &str = "horizon-touch.txt";

/// The player's touch control settings (Options › Controls › Touch Controls).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TouchSettings {
    /// How far a drag turns the view, 1 being a screen pixel for a mouse pixel.
    pub look_sensitivity: f32,
    /// The buttons' opacity, 1 being Bedrock's.
    pub opacity: f32,
    /// The buttons' size, 1 being Bedrock's.
    pub button_scale: f32,
    /// A tap of the haptic engine when a block breaks or the player is hurt.
    pub haptics: bool,
    /// The frame rate and the player's coordinates drawn over the HUD.
    pub show_fps: bool,
    pub show_coords: bool,
    /// The coordinates also show where the player would be in the other world (Nether and
    /// Overworld), switched by a tap on them.
    pub coords_other: bool,
    /// Moving a control in the layout editor lines it up with the ones near it.
    pub align: bool,
    /// The zoom button's magnification: an index of [`ZOOM_FACTORS`].
    pub zoom_level: u8,
    /// The zoom button toggles instead of zooming while it is held.
    pub zoom_toggle: bool,
    /// Where the player put each control and how big (TouchLayout::custom).
    pub custom: [[f32; 3]; CUSTOM_SLOTS],
}

impl Default for TouchSettings {
    fn default() -> Self {
        Self {
            look_sensitivity: 1.0,
            opacity: 1.0,
            button_scale: 1.0,
            haptics: true,
            show_fps: false,
            show_coords: false,
            coords_other: false,
            align: false,
            zoom_level: 2,
            zoom_toggle: false,
            custom: [NO_CUSTOM; CUSTOM_SLOTS],
        }
    }
}

impl TouchSettings {
    pub const LOOK: (f32, f32) = (0.25, 3.0);
    pub const OPACITY: (f32, f32) = (0.25, 1.5);
    pub const SCALE: (f32, f32) = (0.75, 1.5);

    pub fn load(dir: &std::path::Path) -> Self {
        std::fs::read_to_string(dir.join(FILE))
            .map(|t| Self::parse(&t))
            .unwrap_or_default()
    }
    /// Written once something differs from the defaults.
    pub fn save(&self, dir: &std::path::Path) {
        let path = dir.join(FILE);
        if *self != Self::default() || path.exists() {
            let _ = std::fs::write(path, self.serialize());
        }
    }
    pub fn parse(text: &str) -> Self {
        let mut s = Self::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim();
            let number = |d: f32, (lo, hi): (f32, f32)| {
                value
                    .parse::<f32>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .map_or(d, |v| v.clamp(lo, hi))
            };
            if let Some(n) = key.trim().strip_prefix("slot")
                && let Ok(n) = n.parse::<usize>()
                && n < CUSTOM_SLOTS
            {
                let v: Vec<f32> = value
                    .split(',')
                    .filter_map(|p| p.trim().parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .collect();
                if v.len() == 3 {
                    s.custom[n] = [
                        v[0].clamp(-4000.0, 4000.0),
                        v[1].clamp(-4000.0, 4000.0),
                        v[2].clamp(CUSTOM_SIZE.0, CUSTOM_SIZE.1),
                    ];
                }
                continue;
            }
            match key.trim() {
                "lookSensitivity" => s.look_sensitivity = number(s.look_sensitivity, Self::LOOK),
                "opacity" => s.opacity = number(s.opacity, Self::OPACITY),
                "buttonScale" => s.button_scale = number(s.button_scale, Self::SCALE),
                "showFps" => s.show_fps = value == "true",
                "showCoords" => s.show_coords = value == "true",
                "coordsOther" => s.coords_other = value == "true",
                "align" => s.align = value == "true",
                "zoomToggle" => s.zoom_toggle = value == "true",
                "zoomLevel" => {
                    s.zoom_level = value
                        .parse::<u8>()
                        .map_or(s.zoom_level, |v| v.min(ZOOM_FACTORS.len() as u8 - 1))
                }
                "haptics" => {
                    s.haptics = match value {
                        "true" => true,
                        "false" => false,
                        _ => s.haptics,
                    }
                }
                _ => {}
            }
        }
        s
    }
    pub fn serialize(&self) -> String {
        let mut text = format!(
            "lookSensitivity:{}\nopacity:{}\nbuttonScale:{}\nhaptics:{}\nshowFps:{}\nshowCoords:{}\ncoordsOther:{}\nalign:{}\n",
            self.look_sensitivity,
            self.opacity,
            self.button_scale,
            self.haptics,
            self.show_fps,
            self.show_coords,
            self.coords_other,
            self.align
        );
        text.push_str(&format!(
            "zoomLevel:{}\nzoomToggle:{}\n",
            self.zoom_level, self.zoom_toggle
        ));
        for (n, c) in self.custom.iter().enumerate() {
            if *c != NO_CUSTOM {
                text.push_str(&format!("slot{n}:{},{},{}\n", c[0], c[1], c[2]));
            }
        }
        text
    }
}

/// A menu finger held this long without moving presses where it is: the right button over an
/// inventory's slots (split a stack, place one), else the left (drag a slider or scrollbar).
pub const MENU_HOLD: Duration = Duration::from_millis(350);

/// What a finger in a menu does as the mouse, in GUI pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuAction {
    Move([f32; 2]),
    Press {
        right: bool,
    },
    Release {
        right: bool,
    },
    /// The list under the finger follows it: positive when the finger moved down.
    Scroll(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum MenuMode {
    /// Down, not yet a tap, a scroll or a drag.
    Undecided,
    Scroll,
    Drag {
        right: bool,
    },
}

#[derive(Clone, Copy, Debug)]
struct MenuFinger {
    id: u64,
    start: [f32; 2],
    last: [f32; 2],
    since: Instant,
    mode: MenuMode,
}

/// The first finger in a menu, as the mouse. Buttons in menus act when pressed, so the press
/// waits until the finger shows what it does: lifted, it was a tap (a click); moved up or down,
/// it scrolls the list (no click); moved sideways, or over an inventory's slots, it drags with
/// the button down from where it started; held still, it presses there (see [`MENU_HOLD`]).
#[derive(Debug, Default)]
pub struct MenuTouch {
    finger: Option<MenuFinger>,
}

impl MenuTouch {
    pub fn owns(&self, id: u64) -> bool {
        self.finger.is_some_and(|f| f.id == id)
    }

    pub fn start(&mut self, id: u64, at: [f32; 2], now: Instant) -> Vec<MenuAction> {
        if self.finger.is_some() {
            return Vec::new();
        }
        self.finger = Some(MenuFinger {
            id,
            start: at,
            last: at,
            since: now,
            mode: MenuMode::Undecided,
        });
        vec![MenuAction::Move(at)]
    }

    /// `slots`: the menu is an inventory, where a drag moves items instead of scrolling.
    pub fn moved(&mut self, id: u64, at: [f32; 2], slots: bool) -> Vec<MenuAction> {
        let Some(f) = self.finger.as_mut().filter(|f| f.id == id) else {
            return Vec::new();
        };
        let actions = match f.mode {
            MenuMode::Undecided if distance(f.start, at) <= SLOP => Vec::new(),
            MenuMode::Undecided => {
                let (dx, dy) = (at[0] - f.start[0], at[1] - f.start[1]);
                if !slots && dy.abs() > dx.abs() {
                    f.mode = MenuMode::Scroll;
                    vec![MenuAction::Scroll(dy)]
                } else {
                    f.mode = MenuMode::Drag { right: false };
                    vec![
                        MenuAction::Move(f.start),
                        MenuAction::Press { right: false },
                        MenuAction::Move(at),
                    ]
                }
            }
            MenuMode::Scroll => vec![MenuAction::Scroll(at[1] - f.last[1])],
            MenuMode::Drag { .. } => vec![MenuAction::Move(at)],
        };
        f.last = at;
        actions
    }

    pub fn end(&mut self, id: u64, at: [f32; 2]) -> Vec<MenuAction> {
        if !self.owns(id) {
            return Vec::new();
        }
        let Some(f) = self.finger.take() else {
            return Vec::new();
        };
        match f.mode {
            MenuMode::Undecided => vec![
                MenuAction::Move(f.start),
                MenuAction::Press { right: false },
                MenuAction::Release { right: false },
            ],
            MenuMode::Scroll => Vec::new(),
            MenuMode::Drag { right } => vec![MenuAction::Move(at), MenuAction::Release { right }],
        }
    }

    /// Once per frame: a finger held still long enough presses.
    pub fn poll(&mut self, now: Instant, slots: bool) -> Vec<MenuAction> {
        match self.finger.as_mut() {
            Some(f)
                if f.mode == MenuMode::Undecided && now.duration_since(f.since) >= MENU_HOLD =>
            {
                f.mode = MenuMode::Drag { right: slots };
                vec![MenuAction::Move(f.last), MenuAction::Press { right: slots }]
            }
            _ => Vec::new(),
        }
    }

    /// The menu closed or changed: forget the finger (its lift is then ignored).
    pub fn cancel(&mut self) {
        self.finger = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controls() -> TouchControls {
        TouchControls {
            // An iPhone 12 at auto GUI scale 4.
            layout: TouchLayout::new(633.0, 293.0),
            ..Default::default()
        }
    }

    fn centre(r: Rect) -> [f32; 2] {
        [r.x + r.w / 2.0, r.y + r.h / 2.0]
    }

    #[test]
    fn menu_tap_clicks_on_lift() {
        let t0 = Instant::now();
        let mut m = MenuTouch::default();
        assert_eq!(
            m.start(1, [100.0, 50.0], t0),
            [MenuAction::Move([100.0, 50.0])]
        );
        assert!(
            m.moved(1, [102.0, 51.0], false).is_empty(),
            "within the slop"
        );
        assert!(m.poll(t0 + Duration::from_millis(100), false).is_empty());
        assert_eq!(
            m.end(1, [102.0, 51.0]),
            [
                MenuAction::Move([100.0, 50.0]),
                MenuAction::Press { right: false },
                MenuAction::Release { right: false },
            ]
        );
        assert!(!m.owns(1));
    }

    #[test]
    fn menu_vertical_drag_scrolls_without_clicking() {
        let t0 = Instant::now();
        let mut m = MenuTouch::default();
        m.start(1, [100.0, 50.0], t0);
        assert_eq!(m.moved(1, [101.0, 70.0], false), [MenuAction::Scroll(20.0)]);
        assert_eq!(
            m.moved(1, [101.0, 60.0], false),
            [MenuAction::Scroll(-10.0)]
        );
        assert!(
            m.poll(t0 + MENU_HOLD * 2, false).is_empty(),
            "scrolling never presses"
        );
        assert!(m.end(1, [101.0, 60.0]).is_empty());
    }

    #[test]
    fn menu_sideways_drag_presses_where_it_started() {
        let t0 = Instant::now();
        let mut m = MenuTouch::default();
        m.start(1, [100.0, 50.0], t0);
        assert_eq!(
            m.moved(1, [120.0, 52.0], false),
            [
                MenuAction::Move([100.0, 50.0]),
                MenuAction::Press { right: false },
                MenuAction::Move([120.0, 52.0]),
            ]
        );
        assert_eq!(
            m.end(1, [130.0, 52.0]),
            [
                MenuAction::Move([130.0, 52.0]),
                MenuAction::Release { right: false }
            ]
        );
    }

    #[test]
    fn inventory_drags_and_long_presses() {
        let t0 = Instant::now();
        let mut m = MenuTouch::default();
        // Up and down over slots drags an item instead of scrolling.
        m.start(1, [100.0, 50.0], t0);
        assert_eq!(
            m.moved(1, [100.0, 80.0], true)[1],
            MenuAction::Press { right: false }
        );
        m.end(1, [100.0, 80.0]);
        // Held still: the right button.
        m.start(2, [100.0, 50.0], t0);
        assert!(m.poll(t0 + MENU_HOLD / 2, true).is_empty());
        assert_eq!(
            m.poll(t0 + MENU_HOLD, true),
            [
                MenuAction::Move([100.0, 50.0]),
                MenuAction::Press { right: true }
            ]
        );
        assert!(m.poll(t0 + MENU_HOLD * 2, true).is_empty(), "once");
        assert_eq!(
            m.end(2, [100.0, 50.0])[1],
            MenuAction::Release { right: true }
        );
        // Elsewhere a hold presses the left button (a slider).
        m.start(3, [100.0, 50.0], t0);
        assert_eq!(
            m.poll(t0 + MENU_HOLD, false)[1],
            MenuAction::Press { right: false }
        );
    }

    #[test]
    fn menu_second_finger_is_ignored() {
        let t0 = Instant::now();
        let mut m = MenuTouch::default();
        m.start(1, [100.0, 50.0], t0);
        assert!(m.start(2, [200.0, 50.0], t0).is_empty());
        assert!(m.end(2, [200.0, 50.0]).is_empty());
        assert!(m.owns(1));
    }

    #[test]
    fn pause_on_lift_and_debug_on_hold() {
        let t0 = Instant::now();
        let mut c = controls();
        let pause = centre(c.layout.button(TouchButton::Pause));
        c.start(1, pause, t0);
        assert!(!c.frame(t0).pad.held(Button::Start), "nothing while held");
        c.end(1, pause, t0 + Duration::from_millis(100));
        assert!(
            c.frame(t0).pad.held(Button::Start),
            "a tap pauses when lifted"
        );
        c.start(2, pause, t0);
        let f = c.frame(t0 + HOLD_FOR_DEBUG);
        assert!(f.debug && !f.pad.held(Button::Start));
        assert!(!c.frame(t0 + HOLD_FOR_DEBUG * 2).debug, "once per hold");
        c.end(2, pause, t0 + HOLD_FOR_DEBUG * 2);
        assert!(!c.frame(t0).pad.held(Button::Start), "a hold doesn't pause");
    }

    #[test]
    fn touch_settings_round_trip_and_clamp() {
        let s = TouchSettings {
            look_sensitivity: 2.0,
            opacity: 0.5,
            button_scale: 1.25,
            haptics: false,
            show_fps: true,
            show_coords: true,
            coords_other: true,
            align: true,
            zoom_level: 4,
            zoom_toggle: true,
            custom: {
                let mut c = [NO_CUSTOM; CUSTOM_SLOTS];
                c[3] = [12.0, -7.5, 1.5];
                c
            },
        };
        assert_eq!(TouchSettings::parse(&s.serialize()), s);
        let wild = TouchSettings::parse(
            "lookSensitivity:99
opacity:x
buttonScale:0.1
",
        );
        assert_eq!(wild.look_sensitivity, 3.0);
        assert_eq!(wild.opacity, 1.0);
        assert_eq!(wild.button_scale, 0.75);
    }

    #[test]
    fn align_snaps_to_a_nearby_control() {
        let mut l = TouchLayout::new(633.0, 293.0);
        let pause = l.button(TouchButton::Pause);
        let chat = TouchLayout::slot_of(TouchButton::Chat);
        l.custom[chat][1] = 3.0;
        assert_ne!(l.button(TouchButton::Chat).y, pause.y);
        l.align(chat);
        assert_eq!(l.button(TouchButton::Chat).y, pause.y);
        // Far from everything: it stays where it is.
        let jump = TouchLayout::slot_of(TouchButton::Jump);
        l.custom[jump][1] = 40.0;
        let before = l.custom[jump];
        l.align(jump);
        assert!(l.custom[jump][1] <= before[1] + ALIGN_REACH);
    }

    #[test]
    fn sneak_descends_while_held_when_flying() {
        let now = Instant::now();
        let mut c = controls();
        let sneak = centre(c.layout.button(TouchButton::Sneak));
        c.descend = true;
        c.start(1, sneak, now);
        let f = c.frame(now);
        assert!(f.sneak_held && !f.pad.held(Button::RightStick));
        c.end(1, sneak, now);
        assert!(!c.frame(now).sneak_held);
        c.descend = false;
        c.start(2, sneak, now);
        let f = c.frame(now);
        assert!(!f.sneak_held && f.pad.held(Button::RightStick));
    }

    #[test]
    fn layouts_fit_with_insets_and_sizes() {
        for scale in [0.75, 1.0, 1.5] {
            for (w, h) in [(633.0, 293.0), (512.0, 384.0), (427.0, 240.0)] {
                let l = TouchLayout::with(w, h, [36.0, 0.0, 36.0], scale);
                let mut rects: Vec<Rect> = TouchButton::ALL.iter().map(|&b| l.button(b)).collect();
                rects.extend((0..9).map(|n| l.hotbar_slot(n)));
                for (i, a) in rects.iter().enumerate() {
                    assert!(
                        a.x >= 0.0 && a.y >= 0.0 && a.x + a.w <= w && a.y + a.h <= h,
                        "{a:?} off a {w}x{h} screen at {scale}"
                    );
                    for b in &rects[i + 1..] {
                        let apart = a.x + a.w <= b.x
                            || b.x + b.w <= a.x
                            || a.y + a.h <= b.y
                            || b.y + b.h <= a.y;
                        assert!(apart, "{a:?} overlaps {b:?} on {w}x{h} at {scale}");
                    }
                }
            }
        }
    }

    #[test]
    fn layout_fits_and_does_not_overlap() {
        let l = TouchLayout::new(633.0, 293.0);
        let mut rects: Vec<Rect> = TouchButton::ALL.iter().map(|&b| l.button(b)).collect();
        rects.extend((0..9).map(|n| l.hotbar_slot(n)));
        for (i, a) in rects.iter().enumerate() {
            assert!(a.x >= 0.0 && a.y >= 0.0 && a.x + a.w <= 633.0 && a.y + a.h <= 293.0);
            for b in &rects[i + 1..] {
                let apart =
                    a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
                assert!(apart, "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn buttons_hold_their_pad_buttons() {
        let now = Instant::now();
        let mut c = controls();
        c.start(1, centre(c.layout.button(TouchButton::Jump)), now);
        c.start(2, centre(c.layout.button(TouchButton::Sneak)), now);
        let f = c.frame(now);
        assert!(f.pad.held(Button::South));
        assert!(f.pad.held(Button::RightStick));
        c.end(1, centre(c.layout.button(TouchButton::Jump)), now);
        assert!(!c.frame(now).pad.held(Button::South));
    }

    #[test]
    fn a_tap_between_frames_still_presses_once() {
        let now = Instant::now();
        let mut c = controls();
        let jump = centre(c.layout.button(TouchButton::Jump));
        c.start(1, jump, now);
        c.end(1, jump, now);
        assert!(c.frame(now).pad.held(Button::South));
        assert!(!c.frame(now).pad.held(Button::South), "once");
        // Seen held by a frame: released when lifted.
        c.start(2, jump, now);
        assert!(c.frame(now).pad.held(Button::South));
        c.end(2, jump, now);
        assert!(!c.frame(now).pad.held(Button::South));
    }

    #[test]
    fn use_button_only_while_shown() {
        let now = Instant::now();
        let mut c = controls();
        let at = centre(c.layout.button(TouchButton::Use));
        c.start(1, at, now);
        assert_eq!(
            c.frame(now).pad.triggers[0],
            0.0,
            "hidden: the view takes it"
        );
        c.end(1, at, now);
        assert!(c.frame(now).tap);
        c.use_shown = true;
        c.start(2, at, now);
        let f = c.frame(now);
        assert_eq!(f.pad.triggers[0], 1.0);
        assert!(!f.tap);
    }

    #[test]
    fn joystick_floats_clamps_and_sprints() {
        let now = Instant::now();
        let mut c = controls();
        c.start(1, [100.0, 200.0], now);
        c.moved(1, [100.0 + STICK_RADIUS / 2.0, 200.0]);
        let f = c.frame(now);
        assert!((f.pad.left[0] - 0.5).abs() < 1e-5 && f.pad.left[1].abs() < 1e-5);
        assert!(!f.pad.held(Button::LeftStick));
        // Far forward: clamped to 1, and sprinting.
        c.moved(1, [100.0, 200.0 - 3.0 * STICK_RADIUS]);
        let f = c.frame(now);
        assert!((f.pad.left[1] - 1.0).abs() < 1e-5);
        assert!(f.pad.held(Button::LeftStick));
        // Fully sideways doesn't sprint.
        c.moved(1, [100.0 + 3.0 * STICK_RADIUS, 200.0]);
        assert!(!c.frame(now).pad.held(Button::LeftStick));
        // A second thumb on the left looks instead of taking the joystick over.
        c.start(2, [50.0, 150.0], now);
        c.moved(2, [60.0, 150.0]);
        assert_eq!(c.frame(now).look, [10.0, 0.0]);
    }

    #[test]
    fn tap_hold_and_drag_on_the_view() {
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        let at = [450.0, 120.0];
        let mut c = controls();
        // A tap.
        c.start(1, at, t0);
        c.end(1, at, ms(100));
        let f = c.frame(ms(100));
        assert!(f.tap && f.pad.triggers[1] == 0.0);
        assert!(!c.frame(ms(120)).tap, "a tap is taken once");
        // A hold breaks until lifted, and is no tap.
        c.start(2, at, ms(200));
        assert!(c.frame(ms(300)).pad.triggers[1] == 0.0);
        assert!(c.frame(ms(520)).pad.triggers[1] > 0.0);
        c.moved(2, [480.0, 120.0]);
        let f = c.frame(ms(600));
        assert!(f.pad.triggers[1] > 0.0, "breaking survives moving");
        assert_eq!(f.look, [30.0, 0.0]);
        c.end(2, [480.0, 120.0], ms(700));
        let f = c.frame(ms(700));
        assert!(!f.tap && f.pad.triggers[1] == 0.0);
        // A drag turns and is neither.
        c.start(3, at, ms(800));
        c.moved(3, [460.0, 100.0]);
        c.end(3, [470.0, 90.0], ms(850));
        let f = c.frame(ms(1200));
        assert_eq!(f.look, [20.0, -30.0]);
        assert!(!f.tap && f.pad.triggers[1] == 0.0);
    }

    #[test]
    fn hotbar_selects_and_drops_when_held() {
        let t0 = Instant::now();
        let mut c = controls();
        c.start(1, centre(c.layout.hotbar_slot(4)), t0);
        let f = c.frame(t0);
        assert_eq!(f.hotbar, Some(4));
        assert!(!f.drop);
        assert_eq!(c.frame(t0).hotbar, None, "a slot is chosen once");
        let f = c.frame(t0 + HOLD_TO_DROP);
        assert!(f.drop);
        assert!(!c.frame(t0 + HOLD_TO_DROP * 2).drop, "one drop per hold");
    }

    #[test]
    fn release_all_ignores_fingers_until_lifted() {
        let t0 = Instant::now();
        let mut c = controls();
        c.start(1, centre(c.layout.button(TouchButton::Inventory)), t0);
        c.start(2, [100.0, 200.0], t0);
        c.moved(2, [120.0, 200.0]);
        c.release_all();
        assert_eq!(c.frame(t0), TouchFrame::default());
        assert!(c.owns(1));
        c.end(1, [0.0, 0.0], t0);
        assert!(!c.owns(1));
    }
}
