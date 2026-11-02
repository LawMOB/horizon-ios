//! The calls themselves. UIKit's are main-thread only: each takes the main thread's marker and does
//! nothing without it (the game calls them from its event loop, on the main thread).
use block2::RcBlock;
use horizon_client_input::gamepad::{Button, GlyphStyle, PadState};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{MainThreadMarker, NSDictionary, NSString, NSURL};
use objc2_game_controller::{
    GCController, GCControllerAxisInput, GCControllerButtonInput, GCControllerDirectionPad,
    GCDevice, GCKeyCodeBackslash, GCKeyCodeCapsLock, GCKeyCodeCloseBracket, GCKeyCodeComma,
    GCKeyCodeDeleteForward, GCKeyCodeDeleteOrBackspace, GCKeyCodeDownArrow, GCKeyCodeEight,
    GCKeyCodeEnd, GCKeyCodeEqualSign, GCKeyCodeEscape, GCKeyCodeF1, GCKeyCodeF2, GCKeyCodeF3,
    GCKeyCodeF4, GCKeyCodeF5, GCKeyCodeF6, GCKeyCodeF7, GCKeyCodeF8, GCKeyCodeF9, GCKeyCodeF10,
    GCKeyCodeF11, GCKeyCodeF12, GCKeyCodeFive, GCKeyCodeFour, GCKeyCodeGraveAccentAndTilde,
    GCKeyCodeHome, GCKeyCodeHyphen, GCKeyCodeInsert, GCKeyCodeKeyA, GCKeyCodeKeyB, GCKeyCodeKeyC,
    GCKeyCodeKeyD, GCKeyCodeKeyE, GCKeyCodeKeyF, GCKeyCodeKeyG, GCKeyCodeKeyH, GCKeyCodeKeyI,
    GCKeyCodeKeyJ, GCKeyCodeKeyK, GCKeyCodeKeyL, GCKeyCodeKeyM, GCKeyCodeKeyN, GCKeyCodeKeyO,
    GCKeyCodeKeyP, GCKeyCodeKeyQ, GCKeyCodeKeyR, GCKeyCodeKeyS, GCKeyCodeKeyT, GCKeyCodeKeyU,
    GCKeyCodeKeyV, GCKeyCodeKeyW, GCKeyCodeKeyX, GCKeyCodeKeyY, GCKeyCodeKeyZ, GCKeyCodeLeftAlt,
    GCKeyCodeLeftArrow, GCKeyCodeLeftControl, GCKeyCodeLeftShift, GCKeyCodeNine, GCKeyCodeOne,
    GCKeyCodeOpenBracket, GCKeyCodePageDown, GCKeyCodePageUp, GCKeyCodePeriod, GCKeyCodeQuote,
    GCKeyCodeReturnOrEnter, GCKeyCodeRightAlt, GCKeyCodeRightArrow, GCKeyCodeRightControl,
    GCKeyCodeRightShift, GCKeyCodeSemicolon, GCKeyCodeSeven, GCKeyCodeSix, GCKeyCodeSlash,
    GCKeyCodeSpacebar, GCKeyCodeTab, GCKeyCodeThree, GCKeyCodeTwo, GCKeyCodeUpArrow, GCKeyCodeZero,
    GCKeyboard, GCMouse, GCMouseInput,
};
use objc2_ui_kit::{
    UIAlertAction, UIAlertActionStyle, UIAlertController, UIAlertControllerStyle, UIApplication,
    UIImpactFeedbackGenerator, UIImpactFeedbackStyle, UIPasteboard, UIViewController,
};
use std::cell::RefCell;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

pub fn open_url(url: &str) -> bool {
    let Some(mtm) = MainThreadMarker::new() else {
        return false;
    };
    // Safety: a valid NSString; URLWithString returns nil for a malformed URL.
    let Some(url) = (unsafe { NSURL::URLWithString(&NSString::from_str(url)) }) else {
        return false;
    };
    let app = UIApplication::sharedApplication(mtm);
    let options = NSDictionary::<NSString, AnyObject>::new();
    // Safety: the main thread, a valid URL and an empty options dictionary; no completion block.
    unsafe { app.openURL_options_completionHandler(&url, &options, None) };
    true
}

pub fn copy_text(text: &str) {
    // Safety: the general pasteboard is thread-safe; setString copies the string.
    unsafe { UIPasteboard::generalPasteboard().setString(Some(&NSString::from_str(text))) };
}

pub fn pasted_text() -> Option<String> {
    // Safety: as above; string returns nil when the pasteboard holds no text.
    unsafe { UIPasteboard::generalPasteboard().string() }.map(|s| s.to_string())
}

thread_local! {
    /// One generator, kept so the engine stays prepared between taps.
    static HAPTICS: RefCell<Option<Retained<UIImpactFeedbackGenerator>>> = const { RefCell::new(None) };
}

#[allow(deprecated)]
pub fn haptic(strength: f32) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    HAPTICS.with_borrow_mut(|generator| {
        let generator = generator.get_or_insert_with(|| {
            // Safety: the main thread; initWithStyle is deprecated in name only (iOS 17 adds a
            // view-bound initializer), and the generator works unattached.
            unsafe {
                UIImpactFeedbackGenerator::initWithStyle(mtm.alloc(), UIImpactFeedbackStyle::Medium)
            }
        });
        // Safety: the main thread and a live generator.
        unsafe {
            generator.impactOccurredWithIntensity(f64::from(strength.clamp(0.0, 1.0)) as _);
            generator.prepare();
        }
    });
}

/// The keys held on a hardware keyboard (USB or Bluetooth), by the names `key.keyboard.*` the
/// game's key bindings use; none when no keyboard is connected.
pub fn keyboard() -> Option<Vec<&'static str>> {
    // Safety: GameController's polling getters are safe to call from the main thread, which the
    // game's loop is; the key code constants are read-only statics.
    unsafe {
        let keyboard = GCKeyboard::coalescedKeyboard()?;
        let input = keyboard.keyboardInput()?;
        let table = [
            (GCKeyCodeKeyA, "key.keyboard.a"),
            (GCKeyCodeKeyB, "key.keyboard.b"),
            (GCKeyCodeKeyC, "key.keyboard.c"),
            (GCKeyCodeKeyD, "key.keyboard.d"),
            (GCKeyCodeKeyE, "key.keyboard.e"),
            (GCKeyCodeKeyF, "key.keyboard.f"),
            (GCKeyCodeKeyG, "key.keyboard.g"),
            (GCKeyCodeKeyH, "key.keyboard.h"),
            (GCKeyCodeKeyI, "key.keyboard.i"),
            (GCKeyCodeKeyJ, "key.keyboard.j"),
            (GCKeyCodeKeyK, "key.keyboard.k"),
            (GCKeyCodeKeyL, "key.keyboard.l"),
            (GCKeyCodeKeyM, "key.keyboard.m"),
            (GCKeyCodeKeyN, "key.keyboard.n"),
            (GCKeyCodeKeyO, "key.keyboard.o"),
            (GCKeyCodeKeyP, "key.keyboard.p"),
            (GCKeyCodeKeyQ, "key.keyboard.q"),
            (GCKeyCodeKeyR, "key.keyboard.r"),
            (GCKeyCodeKeyS, "key.keyboard.s"),
            (GCKeyCodeKeyT, "key.keyboard.t"),
            (GCKeyCodeKeyU, "key.keyboard.u"),
            (GCKeyCodeKeyV, "key.keyboard.v"),
            (GCKeyCodeKeyW, "key.keyboard.w"),
            (GCKeyCodeKeyX, "key.keyboard.x"),
            (GCKeyCodeKeyY, "key.keyboard.y"),
            (GCKeyCodeKeyZ, "key.keyboard.z"),
            (GCKeyCodeOne, "key.keyboard.1"),
            (GCKeyCodeTwo, "key.keyboard.2"),
            (GCKeyCodeThree, "key.keyboard.3"),
            (GCKeyCodeFour, "key.keyboard.4"),
            (GCKeyCodeFive, "key.keyboard.5"),
            (GCKeyCodeSix, "key.keyboard.6"),
            (GCKeyCodeSeven, "key.keyboard.7"),
            (GCKeyCodeEight, "key.keyboard.8"),
            (GCKeyCodeNine, "key.keyboard.9"),
            (GCKeyCodeZero, "key.keyboard.0"),
            (GCKeyCodeReturnOrEnter, "key.keyboard.enter"),
            (GCKeyCodeEscape, "key.keyboard.escape"),
            (GCKeyCodeDeleteOrBackspace, "key.keyboard.backspace"),
            (GCKeyCodeTab, "key.keyboard.tab"),
            (GCKeyCodeSpacebar, "key.keyboard.space"),
            (GCKeyCodeHyphen, "key.keyboard.minus"),
            (GCKeyCodeEqualSign, "key.keyboard.equal"),
            (GCKeyCodeOpenBracket, "key.keyboard.left.bracket"),
            (GCKeyCodeCloseBracket, "key.keyboard.right.bracket"),
            (GCKeyCodeBackslash, "key.keyboard.backslash"),
            (GCKeyCodeSemicolon, "key.keyboard.semicolon"),
            (GCKeyCodeQuote, "key.keyboard.apostrophe"),
            (GCKeyCodeGraveAccentAndTilde, "key.keyboard.grave.accent"),
            (GCKeyCodeComma, "key.keyboard.comma"),
            (GCKeyCodePeriod, "key.keyboard.period"),
            (GCKeyCodeSlash, "key.keyboard.slash"),
            (GCKeyCodeCapsLock, "key.keyboard.caps.lock"),
            (GCKeyCodeHome, "key.keyboard.home"),
            (GCKeyCodeEnd, "key.keyboard.end"),
            (GCKeyCodePageUp, "key.keyboard.page.up"),
            (GCKeyCodePageDown, "key.keyboard.page.down"),
            (GCKeyCodeDeleteForward, "key.keyboard.delete"),
            (GCKeyCodeInsert, "key.keyboard.insert"),
            (GCKeyCodeUpArrow, "key.keyboard.up"),
            (GCKeyCodeDownArrow, "key.keyboard.down"),
            (GCKeyCodeLeftArrow, "key.keyboard.left"),
            (GCKeyCodeRightArrow, "key.keyboard.right"),
            (GCKeyCodeLeftControl, "key.keyboard.left.control"),
            (GCKeyCodeRightControl, "key.keyboard.right.control"),
            (GCKeyCodeLeftShift, "key.keyboard.left.shift"),
            (GCKeyCodeRightShift, "key.keyboard.right.shift"),
            (GCKeyCodeLeftAlt, "key.keyboard.left.alt"),
            (GCKeyCodeRightAlt, "key.keyboard.right.alt"),
            (GCKeyCodeF1, "key.keyboard.f1"),
            (GCKeyCodeF2, "key.keyboard.f2"),
            (GCKeyCodeF3, "key.keyboard.f3"),
            (GCKeyCodeF4, "key.keyboard.f4"),
            (GCKeyCodeF5, "key.keyboard.f5"),
            (GCKeyCodeF6, "key.keyboard.f6"),
            (GCKeyCodeF7, "key.keyboard.f7"),
            (GCKeyCodeF8, "key.keyboard.f8"),
            (GCKeyCodeF9, "key.keyboard.f9"),
            (GCKeyCodeF10, "key.keyboard.f10"),
            (GCKeyCodeF11, "key.keyboard.f11"),
            (GCKeyCodeF12, "key.keyboard.f12"),
        ];
        Some(
            table
                .into_iter()
                .filter(|(code, _)| {
                    input
                        .buttonForKeyCode(*code)
                        .is_some_and(|button| button.isPressed())
                })
                .map(|(_, name)| name)
                .collect(),
        )
    }
}

/// Mouse movement and scrolling since the last poll, in thousandths of a point: GameController
/// reports them to handlers, which add them here.
static MOUSE_DX: AtomicI32 = AtomicI32::new(0);
static MOUSE_DY: AtomicI32 = AtomicI32::new(0);
static MOUSE_SCROLL: AtomicI32 = AtomicI32::new(0);
/// How many times the handlers have been called, to tell whether the mouse reports at all.
static MOUSE_EVENTS: AtomicU32 = AtomicU32::new(0);

type MovedBlock = RcBlock<dyn Fn(NonNull<GCMouseInput>, f32, f32)>;
type ScrollBlock = RcBlock<dyn Fn(NonNull<GCControllerAxisInput>, f32)>;

thread_local! {
    /// The mouse whose handlers are installed, and the handlers (kept alive here).
    static MOUSE: RefCell<Option<(Retained<GCMouseInput>, MovedBlock, ScrollBlock)>> =
        const { RefCell::new(None) };
}

/// A hardware mouse (USB or Bluetooth): what it moved and scrolled since the last call, and
/// which buttons (left, right, middle) are down; none when no mouse is connected.
pub fn mouse() -> Option<crate::MouseState> {
    // Safety: GameController's getters and handler setters are safe to call from the main
    // thread, which the game's loop is; the handlers only add to atomics.
    unsafe {
        let mouse = GCMouse::current().or_else(|| GCMouse::mice().firstObject())?;
        let input = mouse.mouseInput()?;
        MOUSE.with(|installed| {
            let mut installed = installed.borrow_mut();
            let same = installed
                .as_ref()
                .is_some_and(|(i, _, _)| Retained::as_ptr(i) == Retained::as_ptr(&input));
            if !same {
                let moved: MovedBlock =
                    RcBlock::new(|_: NonNull<GCMouseInput>, dx: f32, dy: f32| {
                        MOUSE_DX.fetch_add((dx * 1000.0) as i32, Ordering::Relaxed);
                        MOUSE_DY.fetch_add((dy * 1000.0) as i32, Ordering::Relaxed);
                        MOUSE_EVENTS.fetch_add(1, Ordering::Relaxed);
                    });
                let scrolled: ScrollBlock =
                    RcBlock::new(|_: NonNull<GCControllerAxisInput>, value: f32| {
                        MOUSE_SCROLL.fetch_add((value * 1000.0) as i32, Ordering::Relaxed);
                        MOUSE_EVENTS.fetch_add(1, Ordering::Relaxed);
                    });
                input.setMouseMovedHandler(&*moved as *const _ as *mut _);
                input
                    .scroll()
                    .yAxis()
                    .setValueChangedHandler(&*scrolled as *const _ as *mut _);
                *installed = Some((input.clone(), moved, scrolled));
            }
        });
        let down = |button: Option<Retained<GCControllerButtonInput>>| {
            button.is_some_and(|b| b.isPressed())
        };
        Some(crate::MouseState {
            moved: [
                MOUSE_DX.swap(0, Ordering::Relaxed) as f32 / 1000.0,
                MOUSE_DY.swap(0, Ordering::Relaxed) as f32 / 1000.0,
            ],
            scroll: MOUSE_SCROLL.swap(0, Ordering::Relaxed) as f32 / 1000.0,
            events: MOUSE_EVENTS.load(Ordering::Relaxed),
            buttons: [
                down(Some(input.leftButton())),
                down(input.rightButton()),
                down(input.middleButton()),
            ],
        })
    }
}

pub fn alert(
    view_controller: Option<std::ptr::NonNull<std::ffi::c_void>>,
    title: &str,
    message: &str,
) {
    let (Some(mtm), Some(view_controller)) = (MainThreadMarker::new(), view_controller) else {
        return;
    };
    // Safety: winit's handle points at the window's live root UIViewController.
    let view_controller: &UIViewController = unsafe { view_controller.cast().as_ref() };
    // Safety: the main thread; the strings are copied; no handler block.
    unsafe {
        let alert = UIAlertController::alertControllerWithTitle_message_preferredStyle(
            Some(&NSString::from_str(title)),
            Some(&NSString::from_str(message)),
            UIAlertControllerStyle::Alert,
            mtm,
        );
        let ok = UIAlertAction::actionWithTitle_style_handler(
            Some(&NSString::from_str("OK")),
            UIAlertActionStyle::Default,
            None,
            mtm,
        );
        alert.addAction(&ok);
        view_controller.presentViewController_animated_completion(&alert, true, None);
    }
}

pub fn controller() -> Option<(PadState, GlyphStyle)> {
    // Safety: GameController's polling getters are safe to call from the main thread, which
    // the game's loop is; every object returned is retained.
    unsafe {
        let controller =
            GCController::current().or_else(|| GCController::controllers().firstObject())?;
        let pad = controller.extendedGamepad()?;
        let mut state = PadState::default();
        let mut button = |b: Button, input: &GCControllerButtonInput| {
            state.set(b, input.isPressed());
        };
        button(Button::South, &pad.buttonA());
        button(Button::East, &pad.buttonB());
        button(Button::West, &pad.buttonX());
        button(Button::North, &pad.buttonY());
        button(Button::LeftBumper, &pad.leftShoulder());
        button(Button::RightBumper, &pad.rightShoulder());
        button(Button::Start, &pad.buttonMenu());
        if let Some(options) = pad.buttonOptions() {
            button(Button::Select, &options);
        }
        if let Some(stick) = pad.leftThumbstickButton() {
            button(Button::LeftStick, &stick);
        }
        if let Some(stick) = pad.rightThumbstickButton() {
            button(Button::RightStick, &stick);
        }
        let dpad = pad.dpad();
        button(Button::DPadUp, &dpad.up());
        button(Button::DPadDown, &dpad.down());
        button(Button::DPadLeft, &dpad.left());
        button(Button::DPadRight, &dpad.right());
        let stick = |s: &GCControllerDirectionPad| [s.xAxis().value(), s.yAxis().value()];
        state.left = stick(&pad.leftThumbstick());
        state.right = stick(&pad.rightThumbstick());
        state.triggers = [pad.leftTrigger().value(), pad.rightTrigger().value()];
        let category = controller.productCategory().to_string();
        let style = if category.contains("DualSense") || category.contains("DualShock") {
            GlyphStyle::PlayStation
        } else if category.contains("Switch") || category.contains("Joy-Con") {
            GlyphStyle::Nintendo
        } else {
            GlyphStyle::Xbox
        };
        Some((state, style))
    }
}
