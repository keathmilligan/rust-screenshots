use std::collections::HashMap;

// Import macOS Core Graphics APIs
use core_graphics_helmer_fork::window::{CGWindowListCopyWindowInfo, kCGWindowListOptionAll};
use core_foundation::{array::CFArray, dictionary::CFDictionary, string::CFString, number::CFNumber, base::{TCFType, ToVoid}};

// Import from the local scap library
use scap::Target;

pub fn list_windows() -> Result<(), Box<dyn std::error::Error>> {
    // First, get windows from scap with their indices
    let mut scap_indices: HashMap<u32, usize> = HashMap::new();
    if scap::is_supported() {
        let targets = scap::get_all_targets();
        let mut window_index = 0;
        for target in targets.iter() {
            if let Target::Window(window) = target {
                scap_indices.insert(window.id, window_index);
                window_index += 1;
            }
        }
    }

    // Then get detailed window info from macOS APIs
    unsafe {
        let window_list = CGWindowListCopyWindowInfo(kCGWindowListOptionAll, 0);
        let windows_array: CFArray<CFDictionary> = CFArray::wrap_under_create_rule(window_list);
        let count = windows_array.len();
        let mut shown_count = 0;

        for i in 0..count {
            if let Some(window_dict) = windows_array.get(i) {
                // Extract window information
                let window_id = get_cf_number_value(&window_dict, "kCGWindowNumber").unwrap_or(0) as u32;
                let owner_pid = get_cf_number_value(&window_dict, "kCGWindowOwnerPID").unwrap_or(0);
                let window_layer = get_cf_number_value(&window_dict, "kCGWindowLayer").unwrap_or(0);

                let window_name = get_cf_string_value(&window_dict, "kCGWindowName").unwrap_or("".to_string());
                let owner_name = get_cf_string_value(&window_dict, "kCGWindowOwnerName").unwrap_or("Unknown".to_string());

                // Get bounds information
                let bounds = get_window_bounds(&window_dict);

                // Get alpha/transparency
                let alpha = get_cf_number_value(&window_dict, "kCGWindowAlpha").unwrap_or(1);

                // Check if window is on screen
                let on_screen = get_cf_number_value(&window_dict, "kCGWindowIsOnscreen").unwrap_or(1) == 1;

                // Filter to show meaningful windows
                let has_meaningful_info = !window_name.is_empty() ||
                                          (!owner_name.is_empty() && owner_name != "Unknown" &&
                                           (bounds.2 > 50 || bounds.3 > 50));

                if has_meaningful_info {
                    // Check if this window has a scap index
                    let index_str = if let Some(idx) = scap_indices.get(&window_id) {
                        format!("{:4}", idx)
                    } else {
                        "   -".to_string()
                    };

                    println!("Idx:{} | ID:{:6} | PID:{:6} | Layer:{:12} | {:>8} | {:>1.2} | {:>4},{:<4} | {:>4}x{:<4} | {:<20} | {}",
                        index_str,
                        window_id,
                        owner_pid,
                        window_layer,
                        if on_screen { "OnScreen" } else { "OffScren" },
                        alpha as f32,
                        bounds.0, bounds.1,
                        bounds.2, bounds.3,  // width x height
                        truncate_string(&owner_name, 20),
                        if window_name.is_empty() {
                            if bounds.2 > 0 && bounds.3 > 0 {
                                format!("({})", truncate_string(&get_bounds_string(&bounds), 30))
                            } else {
                                "(untitled)".to_string()
                            }
                        } else {
                            truncate_string(&window_name, 50)
                        }
                    );
                    shown_count += 1;
                }
            }
        }

        println!("\nShowing {} of {} total windows ({} capturable via scap)",
                shown_count, count, scap_indices.len());
    }

    Ok(())
}

fn get_cf_string_value(dict: &CFDictionary, key: &str) -> Option<String> {
    let cf_key = CFString::new(key);
    dict.find(cf_key.to_void()).and_then(|value| {
        let cf_string = unsafe { CFString::wrap_under_get_rule((*value).cast()) };
        Some(cf_string.to_string())
    })
}

fn get_cf_number_value(dict: &CFDictionary, key: &str) -> Option<i64> {
    let cf_key = CFString::new(key);
    dict.find(cf_key.to_void()).and_then(|value| {
        let cf_number = unsafe { CFNumber::wrap_under_get_rule((*value).cast()) };
        cf_number.to_i64()
    })
}

fn get_window_bounds(dict: &CFDictionary) -> (i32, i32, i32, i32) {
    let bounds_key = CFString::new("kCGWindowBounds");
    if let Some(bounds_value) = dict.find(bounds_key.to_void()) {
        let bounds_dict = unsafe { CFDictionary::wrap_under_get_rule((*bounds_value).cast()) };

        let x = get_cf_number_value(&bounds_dict, "X").unwrap_or(0) as i32;
        let y = get_cf_number_value(&bounds_dict, "Y").unwrap_or(0) as i32;
        let width = get_cf_number_value(&bounds_dict, "Width").unwrap_or(0) as i32;
        let height = get_cf_number_value(&bounds_dict, "Height").unwrap_or(0) as i32;

        (x, y, width, height)
    } else {
        (0, 0, 0, 0)
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len.saturating_sub(1)])
    }
}

fn get_bounds_string(bounds: &(i32, i32, i32, i32)) -> String {
    format!("{}x{} at ({},{})", bounds.2, bounds.3, bounds.0, bounds.1)
}