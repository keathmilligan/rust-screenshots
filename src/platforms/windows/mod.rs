// Import from the local scap library
use scap::Target;
use std::collections::HashMap;
use windows::Win32::Foundation::{HWND, RECT, BOOL, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowRect, GetWindowLongW, IsWindowVisible,
    GWL_STYLE, GetWindowThreadProcessId
};

struct WindowCallbackData {
    scap_indices: HashMap<u32, usize>,
    shown_count: usize,
    total_count: usize,
}

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

    // Then get detailed window info from Windows APIs
    println!("Idx | ID       | PID     | Style    | Visible | X    Y    | W    H    | Title");
    println!("----|----------|---------|----------|---------|-----------|-----------|------");

    let mut data = WindowCallbackData {
        scap_indices,
        shown_count: 0,
        total_count: 0,
    };

    unsafe {
        EnumWindows(Some(enum_window_proc), LPARAM(&mut data as *mut _ as isize))?;
    }

    println!("\nShowing {} of {} total windows ({} capturable via scap)",
             data.shown_count, data.total_count, data.scap_indices.len());

    Ok(())
}

unsafe extern "system" fn enum_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let data = unsafe { &mut *(lparam.0 as *mut WindowCallbackData) };

    data.total_count += 1;

    let window_id = hwnd.0 as u32;

    // Get window title
    let mut title = [0u16; 512];
    let title_len = unsafe { GetWindowTextW(hwnd, &mut title) };
    let title = String::from_utf16_lossy(&title[..title_len as usize]);

    // Skip windows without titles or very small
    if title.is_empty() {
        return BOOL(1); // Continue enumeration
    }

    // Get window rect
    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
        return BOOL(1);
    }

    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    // Skip very small windows
    if width < 10 || height < 10 {
        return BOOL(1);
    }

    // Get PID
    let mut pid = 0u32;
    let _thread_id = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    let pid = if pid != 0 { pid } else { 0 };

    // Get window style
    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;

    // Check if visible
    let visible = unsafe { IsWindowVisible(hwnd) }.as_bool();

    // Check if this window has a scap index
    let index_str = if let Some(idx) = data.scap_indices.get(&window_id) {
        format!("{:3}", idx)
    } else {
        "  -".to_string()
    };

    println!("{:3} | {:8} | {:7} | {:8X} | {:7} | {:3},{:3} | {:3}x{:3} | {}",
             index_str,
             window_id,
             pid,
             style,
             if visible { "Yes" } else { "No" },
             rect.left, rect.top,
             width, height,
             truncate_string(&title, 30)
    );

    data.shown_count += 1;

    BOOL(1) // Continue enumeration
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len.saturating_sub(1)])
    }
}