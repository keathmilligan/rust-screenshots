use clap::{Parser, Subcommand};
use std::process;
use base64::{Engine as _, engine::general_purpose};

// Import from the local scap library
use scap::{capturer::{Capturer, Options}, frame::VideoFrame, Target};

// Import OCR libraries
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;

mod platforms;

#[cfg(target_os = "macos")]
use crate::platforms::mac::list_windows;

#[cfg(target_os = "windows")]
use crate::platforms::windows::list_windows;

#[cfg(target_os = "linux")]
use crate::platforms::linux::list_windows;

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
    /// List available windows with detailed info
    ListWindows,
    /// Capture a screen by number
    Capture {
        /// Screen number to capture
        screen: usize,
        /// Output filename (optional, defaults to screenshot_<timestamp>.jpg)
        #[arg(short, long)]
        output: Option<String>,
        /// Analyze the captured image with LLM (requires LMStudio running locally)
        #[arg(long)]
        analyze: bool,
        /// Custom prompt for LLM analysis
        #[arg(long)]
        prompt: Option<String>,
        /// Extract text from the captured image using OCR
        #[arg(long)]
        ocr: bool,
    },
    /// Capture a window by number
    CaptureWindow {
        /// Window number to capture
        window: usize,
        /// Output filename (optional, defaults to screenshot_<timestamp>.jpg)
        #[arg(short, long)]
        output: Option<String>,
        /// Analyze the captured image with LLM (requires LMStudio running locally)
        #[arg(long)]
        analyze: bool,
        /// Custom prompt for LLM analysis
        #[arg(long)]
        prompt: Option<String>,
        /// Extract text from the captured image using OCR
        #[arg(long)]
        ocr: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::List => list_screens()?,
        Commands::ListWindows => list_windows()?,
        Commands::Capture { screen, output, analyze, prompt, ocr } => {
            capture_screen(*screen, output.as_deref(), *analyze, prompt.as_deref(), *ocr).await?
        },
        Commands::CaptureWindow { window, output, analyze, prompt, ocr } => {
            capture_window(*window, output.as_deref(), *analyze, prompt.as_deref(), *ocr).await?
        },
    }

    Ok(())
}

fn save_jpeg_bytes(jpeg_bytes: &[u8], filename: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs::File;
    use std::io::Write;
    
    println!("Saving {filename}");
    let mut file = File::create(filename)?;
    file.write_all(jpeg_bytes)?;
    
    Ok(())
}

async fn analyze_image_with_llm_base64(base64_image: &str, custom_prompt: Option<&str>) -> Result<String, Box<dyn std::error::Error>> {
    use serde_json::json;
    
    let default_prompt = "Analyze this screenshot and describe all UI elements, text, images and other information. Analyze text carefully and include the full text recognized in each area.";
    let prompt = custom_prompt.unwrap_or(default_prompt);
    
    // Use reqwest directly to ensure proper vision API format
    let vision_payload = json!({
        "model": "gpt-4-vision-preview",
        "messages": [
            {
                "role": "user",
                "content": [
                    {
                        "type": "text",
                        "text": prompt
                    },
                    {
                        "type": "image_url",
                        "image_url": {
                            "url": format!("data:image/jpeg;base64,{}", base64_image)
                        }
                    }
                ]
            }
        ],
        "max_tokens": 1000
    });
    
    let response = reqwest::Client::new()
        .post("http://localhost:1234/v1/chat/completions")
        .header("Authorization", "Bearer lm-studio")
        .header("Content-Type", "application/json")
        .json(&vision_payload)
        .send()
        .await?;
    
    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await?;
        return Err(format!("LLM request failed with status {}: {}. Make sure LMStudio is running on localhost:1234 with a vision model loaded.", status, error_text).into());
    }
    
    let response_json: serde_json::Value = response.json().await?;
    
    if let Some(content) = response_json["choices"][0]["message"]["content"].as_str() {
        Ok(content.to_string())
    } else {
        Err("No content in LLM response".into())
    }
}

async fn extract_text_with_ocr(width: u32, height: u32, rgb_data: &[u8]) -> Result<String, Box<dyn std::error::Error>> {
    use std::path::PathBuf;

    println!("Extracting text with OCR");
    
    // Model paths - these should be downloaded using the download-models.sh script from ocrs examples
    let mut detection_model_path = PathBuf::from("../ocrs/ocrs/examples/text-detection.rten");
    let mut rec_model_path = PathBuf::from("../ocrs/ocrs/examples/text-recognition.rten");
    
    // If the models don't exist in the ocrs examples directory, try current directory
    if !detection_model_path.exists() {
        detection_model_path = PathBuf::from("text-detection.rten");
    }
    if !rec_model_path.exists() {
        rec_model_path = PathBuf::from("text-recognition.rten");
    }
    
    // Check if models exist
    if !detection_model_path.exists() || !rec_model_path.exists() {
        return Err(format!(
            "OCR models not found. Please download models using the download-models.sh script from the ocrs examples directory.\nLooked for:\n- {}\n- {}",
            detection_model_path.display(),
            rec_model_path.display()
        ).into());
    }
    
    // Load the models
    println!("Loading models");
    let detection_model = Model::load_file(detection_model_path)?;
    let recognition_model = Model::load_file(rec_model_path)?;
    
    // Create OCR engine
    let engine = OcrEngine::new(OcrEngineParams {
        detection_model: Some(detection_model),
        recognition_model: Some(recognition_model),
        ..Default::default()
    })?;

    println!("Preparing image for OCR");
    
    // Create image source directly from RGB8 data
    let img_source = ImageSource::from_bytes(rgb_data, (width, height))?;
    let ocr_input = engine.prepare_input(img_source)?;
    
    println!("Performing OCR analysis");
    // Perform OCR: detect words, find lines, recognize text
    let word_rects = engine.detect_words(&ocr_input)?;
    let line_rects = engine.find_text_lines(&ocr_input, &word_rects);
    let line_texts = engine.recognize_text(&ocr_input, &line_rects)?;
    
    // Collect all text lines into a single string
    let extracted_text: Vec<String> = line_texts
        .iter()
        .flatten()
        // Filter likely spurious detections
        .filter(|l| l.to_string().len() > 1)
        .map(|l| l.to_string())
        .collect();
    
    if extracted_text.is_empty() {
        Ok("No text detected in the image.".to_string())
    } else {
        Ok(extracted_text.join("\n"))
    }
}

fn bgra_to_rgb8(bgra_frame: &scap::frame::BGRAFrame) -> (u32, u32, Vec<u8>) {
    // Convert BGRA to RGB by swapping B and R channels and dropping alpha
    let mut rgb_data = Vec::with_capacity((bgra_frame.data.len() * 3) / 4);
    for chunk in bgra_frame.data.chunks_exact(4) {
        rgb_data.push(chunk[2]); // R (was B)
        rgb_data.push(chunk[1]); // G
        rgb_data.push(chunk[0]); // B (was R)
        // Drop alpha channel
    }
    
    (bgra_frame.width as u32, bgra_frame.height as u32, rgb_data)
}

fn rgb8_to_jpeg_bytes(width: u32, height: u32, rgb_data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use image::{ImageBuffer, Rgb};
    
    // Create image buffer from RGB8 data
    let img_buffer = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(
        width,
        height,
        rgb_data.to_vec(),
    ).ok_or("Failed to create image buffer")?;
    
    // Convert to JPEG bytes with high quality
    let mut jpeg_bytes = Vec::new();
    {
        use image::codecs::jpeg::JpegEncoder;
        use image::ImageEncoder;
        
        let encoder = JpegEncoder::new_with_quality(&mut jpeg_bytes, 75);
        encoder.write_image(
            &img_buffer,
            img_buffer.width(),
            img_buffer.height(),
            image::ColorType::Rgb8,
        )?;
    }
    
    Ok(jpeg_bytes)
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



async fn capture_window(window_index: usize, output_filename: Option<&str>, analyze: bool, prompt: Option<&str>, ocr: bool) -> Result<(), Box<dyn std::error::Error>> {
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
        output_resolution: scap::capturer::Resolution::_1080p,
        ..Default::default()
    };

    // Initialize capturer
    println!("Building capturer...");
    let mut capturer = Capturer::build(options).unwrap_or_else(|err| {
        println!("Error building capturer: {err}");
        process::exit(1);
    });
    println!("Capturer built successfully");

    // Generate filename only if output is specified
    let filename = output_filename;

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
                            
                            // Convert to JPEG for both saving and LLM analysis
                            let (width, height, rgb_data) = bgra_to_rgb8(&bgra_frame);
                            let jpeg_bytes = match rgb8_to_jpeg_bytes(width, height, &rgb_data) {
                                Ok(bytes) => bytes,
                                Err(e) => {
                                    println!("Failed to convert frame to JPEG: {}", e);
                                    return Err(e);
                                }
                            };
                            
                            // Save JPEG if output filename was specified
                            if let Some(ref filename) = filename {
                                match save_jpeg_bytes(&jpeg_bytes, filename) {
                                    Ok(_) => println!("Successfully saved window screenshot to: {}", filename),
                                    Err(e) => println!("Failed to save window screenshot: {}", e),
                                }
                            } else {
                                println!("Frame captured successfully (no output file specified, not saving)");
                            }
                            
                            // Analyze with LLM if requested
                            if analyze {
                                let base64_image = general_purpose::STANDARD.encode(&jpeg_bytes);
                                match analyze_image_with_llm_base64(&base64_image, prompt).await {
                                    Ok(analysis) => println!("LLM Analysis:\n{}", analysis),
                                    Err(e) => println!("LLM analysis failed: {}", e),
                                }
                            }
                            
                            // Extract text with OCR if requested
                            if ocr {
                                match extract_text_with_ocr(width, height, &rgb_data).await {
                                    Ok(text) => println!("OCR Text Extraction:\n{}", text),
                                    Err(e) => println!("OCR extraction failed: {}", e),
                                }
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

async fn capture_screen(screen_index: usize, output_filename: Option<&str>, analyze: bool, prompt: Option<&str>, ocr: bool) -> Result<(), Box<dyn std::error::Error>> {
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
    let target = Target::Display(display.clone());
    
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

    // Set up capturer options for the specified screen
    println!("Setting up capturer options for screen {}...", screen_index);
    let options = Options {
        fps: 1,
        show_highlight: false,
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

    // Generate filename only if output is specified
    let filename = output_filename.map(|name| {
        if name.ends_with(".png") {
            name.to_string()
        } else {
            format!("{}.png", name)
        }
    });

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
                            
                            // Convert to JPEG for both saving and LLM analysis
                            let (width, height, rgb_data) = bgra_to_rgb8(&bgra_frame);
                            let jpeg_bytes = match rgb8_to_jpeg_bytes(width, height, &rgb_data) {
                                Ok(bytes) => bytes,
                                Err(e) => {
                                    println!("Failed to convert frame to JPEG: {}", e);
                                    return Err(e);
                                }
                            };
                            
                            // Save JPEG if output filename was specified
                            if let Some(ref filename) = filename {
                                match save_jpeg_bytes(&jpeg_bytes, filename) {
                                    Ok(_) => println!("Successfully saved screenshot to: {}", filename),
                                    Err(e) => println!("Failed to save screenshot: {}", e),
                                }
                            } else {
                                println!("Frame captured successfully (no output file specified, not saving)");
                            }
                            
                            // Analyze with LLM if requested
                            if analyze {
                                let base64_image = general_purpose::STANDARD.encode(&jpeg_bytes);
                                match analyze_image_with_llm_base64(&base64_image, prompt).await {
                                    Ok(analysis) => println!("LLM Analysis:\n{}", analysis),
                                    Err(e) => println!("LLM analysis failed: {}", e),
                                }
                            }
                            
                            // Extract text with OCR if requested
                            if ocr {
                                match extract_text_with_ocr(width, height, &rgb_data).await {
                                    Ok(text) => println!("OCR Text Extraction:\n{}", text),
                                    Err(e) => println!("OCR extraction failed: {}", e),
                                }
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

