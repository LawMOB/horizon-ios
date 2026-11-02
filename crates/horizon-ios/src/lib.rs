//! The iOS system calls Horizon makes (UIKit and GameController through objc2): opening links,
//! the pasteboard, haptics, an alert, and game controllers. The game and its libraries forbid
//! unsafe code; these calls are unsafe in objc2's bindings, so they live here, behind safe
//! functions. Each does nothing on other platforms and off the main thread.
use horizon_client_input::gamepad::{GlyphStyle, PadState};

#[cfg(target_os = "ios")]
mod ios;

/// Open a link (https://, or `shareddocuments://` for a folder in the Files app). False when it
/// couldn't be asked.
pub fn open_url(url: &str) -> bool {
    #[cfg(target_os = "ios")]
    return ios::open_url(url);
    #[cfg(not(target_os = "ios"))]
    {
        let _ = url;
        false
    }
}

/// Open a folder of the app's Documents in the Files app.
pub fn open_folder(path: &std::path::Path) -> bool {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    // The path as a URL: every byte but unreserved ones and slashes percent-encoded.
    let mut url = String::from("shareddocuments://");
    for b in path.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            url.push(b as char);
        } else {
            url.push_str(&format!("%{b:02X}"));
        }
    }
    open_url(&url)
}

/// Put text on the system pasteboard.
pub fn copy_text(text: &str) {
    #[cfg(target_os = "ios")]
    ios::copy_text(text);
    #[cfg(not(target_os = "ios"))]
    let _ = text;
}

/// The text on the system pasteboard, if any.
pub fn pasted_text() -> Option<String> {
    #[cfg(target_os = "ios")]
    return ios::pasted_text();
    #[cfg(not(target_os = "ios"))]
    None
}

/// A tap of the haptic engine, 0..1 strong.
pub fn haptic(strength: f32) {
    #[cfg(target_os = "ios")]
    ios::haptic(strength);
    #[cfg(not(target_os = "ios"))]
    let _ = strength;
}

/// A message with an OK button over the app, from its window's view controller (winit's
/// `UiKitWindowHandle::ui_view_controller`).
pub fn alert(
    view_controller: Option<std::ptr::NonNull<std::ffi::c_void>>,
    title: &str,
    message: &str,
) {
    #[cfg(target_os = "ios")]
    ios::alert(view_controller, title, message);
    #[cfg(not(target_os = "ios"))]
    let _ = (view_controller, title, message);
}

/// The game controller last connected (Bluetooth or MFi), as the pad state the game reads, and
/// its button glyphs.
pub fn controller() -> Option<(PadState, GlyphStyle)> {
    #[cfg(target_os = "ios")]
    return ios::controller();
    #[cfg(not(target_os = "ios"))]
    None
}

/// A hardware mouse's state since the last poll.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MouseState {
    /// How far it moved (points, x right, y down).
    pub moved: [f32; 2],
    /// How far the wheel scrolled (positive is up).
    pub scroll: f32,
    /// Left, right and middle buttons held.
    pub buttons: [bool; 3],
}

/// The hardware mouse (USB or Bluetooth), if one is connected.
pub fn mouse() -> Option<MouseState> {
    #[cfg(target_os = "ios")]
    return ios::mouse();
    #[cfg(not(target_os = "ios"))]
    None
}

/// The keys held on a hardware keyboard (USB or Bluetooth), by the names the game's key bindings
/// use (`key.keyboard.w`); `None` when no keyboard is connected.
pub fn keyboard() -> Option<Vec<&'static str>> {
    #[cfg(target_os = "ios")]
    return ios::keyboard();
    #[cfg(not(target_os = "ios"))]
    None
}
