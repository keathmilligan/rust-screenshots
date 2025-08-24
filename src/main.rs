use clap::{Parser, Subcommand};
use std::process;

// Import from the local scap library
use scap::{capturer::{Capturer, Options}, frame::VideoFrame, Target};

#[derive(Parser)]
#[command(name = "captest")]
#[command(about = "A command-line screen capture tool using scap")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available screens
    List,
    /// List available windows
    ListWindows,
    /// Capture a screen by number
    Capture {
        /// Screen number to capture
        screen: usize,
        /// Output filename (optional, defaults to screenshot_<timestamp>.png)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Capture a window by number
    CaptureWindow {
        /// Window number to capture
        window: usize,
        /// Output filename (optional, defaults to screenshot_<timestamp>.png)
        #[arg(short, long)]
        output: Option<String>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::List => list_screens()?,
        Commands::ListWindows => list_windows()?,
        Commands::Capture { screen, output } => capture_screen(*screen, output.as_deref())?,
        Commands::CaptureWindow { window, output } => capture_window(*window, output.as_deref())?,
    }

    Ok(())
}

fn save_bgra_as_png(bgra_frame: &scap::frame::BGRAFrame, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    use image::{ImageBuffer, Rgba};
    
    // Create an image buffer from the BGRA data
    // Note: BGRA format needs to be converted to RGBA for the image crate
    let mut rgba_data = Vec::with_capacity(bgra_frame.data.len());
    
    // Convert BGRA to RGBA by swapping B and R channels
    for chunk in bgra_frame.data.chunks_exact(4) {
        rgba_data.push(chunk[2]); // R (was B)
        rgba_data.push(chunk[1]); // G
        rgba_data.push(chunk[0]); // B (was R)
        rgba_data.push(chunk[3]); // A
    }
    
    // Create image buffer
    let img_buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
        bgra_frame.width as u32,
        bgra_frame.height as u32,
        rgba_data,
    ).ok_or("Failed to create image buffer")?;
    
    // Save as PNG
    println!("Saving {filename}");
    img_buffer.save(filename)?;
    
    Ok(())
}

fn list_screens() -> Result<(), Box<dyn std::error::Error>> {
    // Check if screen capture is supported
    if !scap::is_supported() {
        println!("Screen capture not supported");
        return Ok(());
    }

    let targets = scap::get_all_targets();
    
    println!("Available screens:");
    println!("==================");
    
    let mut screen_index = 0;
    for target in targets.iter() {
        match target {
            Target::Display(display) => {
                println!("Screen {}: Display ID {}", 
                    screen_index, 
                    display.id
                );
                println!("          Title: {}", display.title);
                println!();
                screen_index += 1;
            }
            Target::Window(_) => {
                // Skip windows, only show displays/screens
                continue;
            }
        }
    }
    
    Ok(())
}

fn list_windows() -> Result<(), Box<dyn std::error::Error>> {
    // Check if screen capture is supported
    if !scap::is_supported() {
        println!("Screen capture not supported");
        return Ok(());
    }

    let targets = scap::get_all_targets();
    
    println!("Available windows:");
    println!("==================");
    
    let mut window_index = 0;
    for target in targets.iter() {
        match target {
            Target::Window(window) => {
                println!("Window {}: ID {}",
                    window_index,
                    window.id
                );
                println!("           Title: {}", window.title);
                println!();
                window_index += 1;
            }
            Target::Display(_) => {
                // Skip displays, only show windows
                continue;
            }
        }
    }
    
    if window_index == 0 {
        println!("No windows available for capture.");
    }
    
    Ok(())
}

fn capture_window(window_index: usize, output_filename: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    // Check if screen capture is supported
    if !scap::is_supported() {
        println!("Screen capture not supported");
        return Ok(());
    }

    // Request permission if not already granted
    if !scap::has_permission() {
        scap::request_permission();
        println!("Please grant screen recording permission and rerun.");
        return Ok(());
    }

    let targets = scap::get_all_targets();
    
    // Filter to only get windows
    let windows: Vec<_> = targets.iter()
        .filter_map(|target| {
            if let Target::Window(window) = target {
                Some(window)
            } else {
                None
            }
        })
        .collect();
    
    if window_index >= windows.len() {
        eprintln!("Error: Window {} not found. Available windows: 0-{}",
            window_index, windows.len().saturating_sub(1));
        std::process::exit(1);
    }
    
    let window = windows[window_index];
    let target = Target::Window(window.clone());
    
    println!("Capturing window {} (ID: {}) - '{}'...",
        window_index, window.id, window.title);
    
    // Check permissions first
    println!("Checking scap permissions...");
    if !scap::has_permission() {
        println!("No screen recording permission! Requesting...");
        scap::request_permission();
        return Err("Please grant screen recording permission in System Preferences and try again".into());
    }
    println!("Permissions OK");

    // Set up capturer options with the specific window target
    println!("Setting up capturer options for window target...");
    let options = Options {
        fps: 1,
        show_highlight: false,
        excluded_targets: None,
        output_type: scap::frame::FrameType::BGRAFrame,
        target: Some(target),
        ..Default::default()
    };

    // Initialize capturer
    println!("Building capturer...");
    let mut capturer = Capturer::build(options).unwrap_or_else(|err| {
        println!("Error building capturer: {err}");
        process::exit(1);
    });
    println!("Capturer built successfully");

    // Generate filename
    let filename = match output_filename {
        Some(name) => {
            if name.ends_with(".png") {
                name.to_string()
            } else {
                format!("{}.png", name)
            }
        }
        None => {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            format!("window_screenshot_{}.png", timestamp)
        }
    };

    capturer.start_capture();
    println!("Scap capturer initialized successfully");
    
    // Try to get a frame synchronously with debug output
    println!("Attempting to get next frame...");
    match capturer.get_next_frame() {
        Ok(frame) => {
            match frame {
                scap::frame::Frame::Video(video_frame) => {
                    match video_frame {
                        VideoFrame::YUVFrame(yuv_frame) => {
                            println!(
                                "Received YUV frame of width {} and height {} and pts {:?}",
                                yuv_frame.width, yuv_frame.height, yuv_frame.display_time
                            );
                        }
                        VideoFrame::BGR0(bgr_frame) => {
                            println!(
                                "Received BGR0 frame of width {} and height {}",
                                bgr_frame.width, bgr_frame.height
                            );
                        }
                        VideoFrame::RGB(rgb_frame) => {
                            println!(
                                "Received RGB frame of width {} and height {} and time {:?}",
                                rgb_frame.width, rgb_frame.height, rgb_frame.display_time
                            );
                        }
                        VideoFrame::RGBx(rgbx_frame) => {
                            println!(
                                "Received RGBx frame of width {} and height {}",
                                rgbx_frame.width, rgbx_frame.height
                            );
                        }
                        VideoFrame::XBGR(xbgr_frame) => {
                            println!(
                                "Received XBGR frame of width {} and height {}",
                                xbgr_frame.width, xbgr_frame.height
                            );
                        }
                        VideoFrame::BGRx(bgrx_frame) => {
                            println!(
                                "Received BGRx frame of width {} and height {}",
                                bgrx_frame.width, bgrx_frame.height
                            );
                        }
                        VideoFrame::BGRA(bgra_frame) => {
                            println!(
                                "Received BGRA frame of width {} and height {} and time {:?}",
                                bgra_frame.width, bgra_frame.height, bgra_frame.display_time
                            );
                            
                            // Convert BGRA frame to PNG and save
                            match save_bgra_as_png(&bgra_frame, &filename) {
                                Ok(_) => println!("Successfully saved window screenshot to: {}", filename),
                                Err(e) => println!("Failed to save window screenshot: {}", e),
                            }
                        }
                    }
                }
                scap::frame::Frame::Audio(_audio_frame) => {
                    println!("Received audio frame (unexpected for screen capture)");
                }
            }
            println!("Frame captured successfully!");
            Ok(())
        }
        Err(e) => {
            println!("Frame capture failed with error: {}", e);
            Err(format!("Frame capture failed: {}", e).into())
        }
    }
}

fn capture_screen(screen_index: usize, output_filename: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    // Check if screen capture is supported
    if !scap::is_supported() {
        println!("Screen capture not supported");
        return Ok(());
    }

    // Request permission if not already granted
    if !scap::has_permission() {
        scap::request_permission();
        println!("Please grant screen recording permission and rerun.");
        return Ok(());
    }

    let targets = scap::get_all_targets();
    
    // Filter to only get displays (screens)
    let displays: Vec<_> = targets.iter()
        .filter_map(|target| {
            if let Target::Display(display) = target {
                Some(display)
            } else {
                None
            }
        })
        .collect();
    
    if screen_index >= displays.len() {
        eprintln!("Error: Screen {} not found. Available screens: 0-{}", 
            screen_index, displays.len().saturating_sub(1));
        std::process::exit(1);
    }
    
    let display = displays[screen_index];
    let _target = Target::Display(display.clone());
    
    println!("Capturing screen {} (ID: {})...", 
        screen_index, display.id);
    
    // Check permissions first
    println!("Checking scap permissions...");
    if !scap::has_permission() {
        println!("No screen recording permission! Requesting...");
        scap::request_permission();
        return Err("Please grant screen recording permission in System Preferences and try again".into());
    }
    println!("Permissions OK");

    // Try with no specific target (should default to primary display)
    println!("Setting up capturer options with no target...");
    let options = Options {
        fps: 1,
        show_highlight: false,
        output_type: scap::frame::FrameType::BGRAFrame,
        ..Default::default()
    };

    // Initialize capturer
    println!("Building capturer...");
    let mut capturer = Capturer::build(options).unwrap_or_else(|err| {
        println!("Error building capturer: {err}");
        process::exit(1);
    });
    println!("Capturer built successfully");

    // Generate filename
    let filename = match output_filename {
        Some(name) => {
            if name.ends_with(".png") {
                name.to_string()
            } else {
                format!("{}.png", name)
            }
        }
        None => {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs();
            format!("screenshot_{}.png", timestamp)
        }
    };

    capturer.start_capture();
    println!("Scap capturer initialized successfully");
    
    // Try to get a frame synchronously with debug output
    println!("Attempting to get next frame...");
    match capturer.get_next_frame() {
        Ok(frame) => {
            match frame {
                scap::frame::Frame::Video(video_frame) => {
                    match video_frame {
                        VideoFrame::YUVFrame(yuv_frame) => {
                            println!(
                                "Received YUV frame of width {} and height {} and pts {:?}",
                                yuv_frame.width, yuv_frame.height, yuv_frame.display_time
                            );
                        }
                        VideoFrame::BGR0(bgr_frame) => {
                            println!(
                                "Received BGR0 frame of width {} and height {}",
                                bgr_frame.width, bgr_frame.height
                            );
                        }
                        VideoFrame::RGB(rgb_frame) => {
                            println!(
                                "Received RGB frame of width {} and height {} and time {:?}",
                                rgb_frame.width, rgb_frame.height, rgb_frame.display_time
                            );
                        }
                        VideoFrame::RGBx(rgbx_frame) => {
                            println!(
                                "Received RGBx frame of width {} and height {}",
                                rgbx_frame.width, rgbx_frame.height
                            );
                        }
                        VideoFrame::XBGR(xbgr_frame) => {
                            println!(
                                "Received XBGR frame of width {} and height {}",
                                xbgr_frame.width, xbgr_frame.height
                            );
                        }
                        VideoFrame::BGRx(bgrx_frame) => {
                            println!(
                                "Received BGRx frame of width {} and height {}",
                                bgrx_frame.width, bgrx_frame.height
                            );
                        }
                        VideoFrame::BGRA(bgra_frame) => {
                            println!(
                                "Received BGRA frame of width {} and height {} and time {:?}",
                                bgra_frame.width, bgra_frame.height, bgra_frame.display_time
                            );
                            
                            // Convert BGRA frame to PNG and save
                            match save_bgra_as_png(&bgra_frame, &filename) {
                                Ok(_) => println!("Successfully saved screenshot to: {}", filename),
                                Err(e) => println!("Failed to save screenshot: {}", e),
                            }
                        }
                    }
                }
                scap::frame::Frame::Audio(_audio_frame) => {
                    println!("Received audio frame (unexpected for screen capture)");
                }
            }
            println!("Frame captured successfully!");
            Ok(())
        }
        Err(e) => {
            println!("Frame capture failed with error: {}", e);
            Err(format!("Frame capture failed: {}", e).into())
        }
    }
}
