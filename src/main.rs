use clap::{Parser, Subcommand};
use screenshots::Screen;

#[derive(Parser)]
#[command(name = "captest")]
#[command(about = "A command-line screen capture tool using screenshots")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available screens
    List,
    /// Capture a screen by number
    Capture {
        /// Screen number to capture
        screen: usize,
        /// Output filename (optional, defaults to screenshot_<timestamp>.png)
        #[arg(short, long)]
        output: Option<String>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::List => list_screens()?,
        Commands::Capture { screen, output } => capture_screen(*screen, output.as_deref())?,
    }

    Ok(())
}

fn list_screens() -> Result<(), Box<dyn std::error::Error>> {
    let screens = Screen::all()?;
    
    println!("Available screens:");
    println!("==================");
    
    for (index, screen) in screens.iter().enumerate() {
        println!("Screen {}: {}x{}", 
            index, 
            screen.display_info.width, 
            screen.display_info.height
        );
        println!("          Scale Factor: {}", screen.display_info.scale_factor);
        println!("          ID: {}", screen.display_info.id);
        if screen.display_info.is_primary {
            println!("          Primary Display: Yes");
        }
        println!();
    }
    
    Ok(())
}

fn capture_screen(screen_index: usize, output_filename: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let screens = Screen::all()?;
    
    if screen_index >= screens.len() {
        eprintln!("Error: Screen {} not found. Available screens: 0-{}", 
            screen_index, screens.len().saturating_sub(1));
        std::process::exit(1);
    }
    
    let screen = &screens[screen_index];
    
    println!("Capturing screen {} ({}x{})...", 
        screen_index, 
        screen.display_info.width, 
        screen.display_info.height
    );
    
    // Capture the screen
    let image = screen.capture()?;
    
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
    
    // Save the image
    image.save(&filename)?;
    
    println!("Screenshot saved to: {}", filename);
    
    Ok(())
}
