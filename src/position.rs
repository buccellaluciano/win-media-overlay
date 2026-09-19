use tauri::{PhysicalPosition, Position, WebviewWindow};

pub fn apply_window_position(
    window: &WebviewWindow,
    preset: &str,
    custom_x: i32,
    custom_y: i32,
) -> Result<(), String> {
    if preset == "custom" {
        // ponytail: clamp to visible monitor, full multi-monitor tracking later if needed
        let (x, y) = window
            .current_monitor()
            .ok()
            .flatten()
            .or_else(|| window.primary_monitor().ok().flatten())
            .map(|m| {
                let s = m.size();
                let p = m.position();
                let x = custom_x.clamp(p.x, p.x + s.width as i32 - 80);
                let y = custom_y.clamp(p.y, p.y + s.height as i32 - 40);
                (x, y)
            })
            .unwrap_or((custom_x, custom_y));
        let _ = window.set_position(Position::Physical(PhysicalPosition { x, y }));
        return Ok(());
    }

    // Try current monitor first, fallback to primary monitor
    let monitor_opt = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());

    if let Some(monitor) = monitor_opt {
        let mon_size = monitor.size();
        let mon_pos = monitor.position();
        let scale = monitor.scale_factor();
        let mon_width = mon_size.width as i32;
        let mon_height = mon_size.height as i32;

        // Use standard base player dimensions for position calculations
        // to prevent height skew when the settings panel is expanded
        let base_width = (480.0 * scale) as i32;
        let base_height = (188.0 * scale) as i32;

        let (x, y) = match preset {
            "top-center" => {
                let x = mon_pos.x + (mon_width - base_width) / 2;
                let y = mon_pos.y + (28.0 * scale) as i32;
                (x, y)
            }
            "bottom-right" => {
                let x = mon_pos.x + mon_width - base_width - (24.0 * scale) as i32;
                let y = mon_pos.y + mon_height - base_height - (64.0 * scale) as i32;
                (x, y)
            }
            "center" => {
                let x = mon_pos.x + (mon_width - base_width) / 2;
                let y = mon_pos.y + (mon_height - base_height) / 2;
                (x, y)
            }
            _ => (custom_x, custom_y),
        };

        let _ = window.set_position(Position::Physical(PhysicalPosition { x, y }));
    }

    Ok(())
}
