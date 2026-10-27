use tauri::{PhysicalPosition, Position, WebviewWindow};

// Top-center of a physical monitor rect. Used by hover-reveal, where the cursor may be
// on a different monitor than the (hidden) window.
pub fn apply_top_center_on_monitor(
    window: &WebviewWindow,
    rect: (i32, i32, i32, i32), // physical: x, y, width, height
    scale: f64,
) -> Result<(), String> {
    let (mon_x, mon_y, mon_width, _mon_height) = rect;
    let base_width = (480.0 * scale) as i32;
    let x = mon_x + (mon_width - base_width) / 2;
    let y = mon_y + (28.0 * scale) as i32;
    window
        .set_position(Position::Physical(PhysicalPosition { x, y }))
        .map_err(|e| e.to_string())
}

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
                return apply_top_center_on_monitor(
                    window,
                    (mon_pos.x, mon_pos.y, mon_width, mon_height),
                    scale,
                );
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
