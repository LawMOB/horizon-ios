//! The calls themselves. UIKit's are main-thread only: each takes the main thread's marker and does
//! nothing without it (the game calls them from its event loop, on the main thread).
use horizon_client_input::gamepad::{Button, GlyphStyle, PadState};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{MainThreadMarker, NSDictionary, NSProcessInfo, NSString, NSURL};
use objc2_game_controller::{
    GCController, GCControllerButtonInput, GCControllerDirectionPad, GCDevice, GCKeyCodeBackslash,
    GCKeyCodeCapsLock, GCKeyCodeCloseBracket, GCKeyCodeComma, GCKeyCodeDeleteForward,
    GCKeyCodeDeleteOrBackspace, GCKeyCodeDownArrow, GCKeyCodeEight, GCKeyCodeEnd,
    GCKeyCodeEqualSign, GCKeyCodeEscape, GCKeyCodeF1, GCKeyCodeF2, GCKeyCodeF3, GCKeyCodeF4,
    GCKeyCodeF5, GCKeyCodeF6, GCKeyCodeF7, GCKeyCodeF8, GCKeyCodeF9, GCKeyCodeF10, GCKeyCodeF11,
    GCKeyCodeF12, GCKeyCodeFive, GCKeyCodeFour, GCKeyCodeGraveAccentAndTilde, GCKeyCodeHome,
    GCKeyCodeHyphen, GCKeyCodeInsert, GCKeyCodeKeyA, GCKeyCodeKeyB, GCKeyCodeKeyC, GCKeyCodeKeyD,
    GCKeyCodeKeyE, GCKeyCodeKeyF, GCKeyCodeKeyG, GCKeyCodeKeyH, GCKeyCodeKeyI, GCKeyCodeKeyJ,
    GCKeyCodeKeyK, GCKeyCodeKeyL, GCKeyCodeKeyM, GCKeyCodeKeyN, GCKeyCodeKeyO, GCKeyCodeKeyP,
    GCKeyCodeKeyQ, GCKeyCodeKeyR, GCKeyCodeKeyS, GCKeyCodeKeyT, GCKeyCodeKeyU, GCKeyCodeKeyV,
    GCKeyCodeKeyW, GCKeyCodeKeyX, GCKeyCodeKeyY, GCKeyCodeKeyZ, GCKeyCodeLeftAlt,
    GCKeyCodeLeftArrow, GCKeyCodeLeftControl, GCKeyCodeLeftShift, GCKeyCodeNine, GCKeyCodeOne,
    GCKeyCodeOpenBracket, GCKeyCodePageDown, GCKeyCodePageUp, GCKeyCodePeriod, GCKeyCodeQuote,
    GCKeyCodeReturnOrEnter, GCKeyCodeRightAlt, GCKeyCodeRightArrow, GCKeyCodeRightControl,
    GCKeyCodeRightShift, GCKeyCodeSemicolon, GCKeyCodeSeven, GCKeyCodeSix, GCKeyCodeSlash,
    GCKeyCodeSpacebar, GCKeyCodeTab, GCKeyCodeThree, GCKeyCodeTwo, GCKeyCodeUpArrow, GCKeyCodeZero,
    GCKeyboard,
};
use objc2_ui_kit::{
    UIAlertAction, UIAlertActionStyle, UIAlertController, UIAlertControllerStyle, UIApplication,
    UIImpactFeedbackGenerator, UIImpactFeedbackStyle, UIPasteboard, UIViewController,
};
use std::cell::RefCell;

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

/// How hot the device says it is: 0 nominal, 1 fair, 2 serious, 3 critical
/// (`NSProcessInfo.thermalState`).
pub fn thermal_state() -> u8 {
    // Safety: processInfo is a shared object and thermalState a plain getter, safe from any thread.
    unsafe {
        let info = NSProcessInfo::processInfo();
        info.thermalState().0.clamp(0, 3) as u8
    }
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
