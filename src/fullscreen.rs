// Detects an exclusive (D3D) fullscreen app in the foreground — the case where an
// always-on-top overlay steals focus and minimizes the game. Borderless-windowed
// fullscreen does NOT trigger this, so the overlay keeps working normally there.
use windows::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUNS_RUNNING_D3D_FULL_SCREEN,
};

pub fn exclusive_fullscreen_active() -> bool {
    unsafe {
        matches!(
            SHQueryUserNotificationState(),
            Ok(QUNS_RUNNING_D3D_FULL_SCREEN)
        )
    }
}
