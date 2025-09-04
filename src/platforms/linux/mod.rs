// Import from the local scap library
use scap::Target;

pub fn list_windows() -> Result<(), Box<dyn std::error::Error>> {
    if scap::is_supported() {
        let targets = scap::get_all_targets();

        println!("Available windows:");
        println!("==================");

        let mut window_index = 0;
        for target in targets.iter() {
            if let Target::Window(window) = target {
                println!("Window {}: ID {}, Title: {}", window_index, window.id, window.title);
                window_index += 1;
            }
        }
    } else {
        println!("Screen capture not supported");
    }

    Ok(())
}