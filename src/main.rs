use clap::Parser;
use screenshots::Screen;
use std::{thread, time::Duration};

#[derive(Parser)]
#[command(version, about = "A CLI tool to handle screen refresh rates")]
struct Cli {
    #[arg(long, help = "Screen refresh rate in fps (frames per second)", 
          value_parser = clap::value_parser!(f64),
          default_value_t = 0.2)]
    fps: f64,
}

fn main() {
    let cli: Cli = Cli::parse();
    println!("Starting capture at {} fps", cli.fps);

    // Get all screens
    let screens: Vec<Screen> = Screen::all().unwrap();
    
    // Use the primary screen (first one)
    if let Some(screen) = screens.first() {
        let interval: Duration = Duration::from_secs_f64(1.0 / cli.fps as f64);
        
        // Capture screen in a loop
        loop {
            let start: std::time::Instant = std::time::Instant::now();
            let image: image::ImageBuffer<image::Rgba<u8>, Vec<u8>> = screen.capture().unwrap();
            // let image_path = format!("screenshot_{}.png", start.elapsed().as_millis());
            let duration: Duration = start.elapsed();
            println!(
                "Captured image {}x{} in {:.2?}",
                image.width(),
                image.height(),
                duration
            );
            
            // Wait for next frame
            thread::sleep(interval);
        }
    } else {
        eprintln!("No screen found!");
    }
}
