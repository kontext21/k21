use clap::Parser;
use mylib::logger::utils::init_logger_exe;
use mylib::screen_capture::utils::{run_screen_capture, ScreenCaptureConfig};

#[derive(Parser)]
#[command(version, about = "A CLI tool to handle screen refresh rates", long_about = None)]
struct Cli {
    #[arg(
        long,
        help = "Screen refresh rate in fps (frames per second)",
        default_value_t = 1.0
    )]
    fps: f32,
    #[arg(
        long,
        help = "Duration of each video chunk in seconds",
        default_value_t = 60
    )]
    video_chunk_duration: u64,
    #[arg(
        long,
        help = "Dump image to stdout (for processor)",
        default_value_t = false
    )]
    stdout: bool,
    #[arg(
        long,
        help = "Save screenshot to disk", 
        default_value_t = false
    )]
    save_screenshot: bool,
    #[arg(
        long, 
        help = "Save video to disk", 
        default_value_t = false
    )]
    save_video: bool,
}

#[tokio::main]
async fn main() {
    init_logger_exe();

    let cli = Cli::parse();
    
    // init tokio runtime
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let _ = rt.enter();

    let config = ScreenCaptureConfig {
        fps: cli.fps,
        video_chunk_duration: cli.video_chunk_duration,
        stdout: cli.stdout,
        save_screenshot: cli.save_screenshot,
        save_video: cli.save_video,
        max_frames: Some(10),
    };

    run_screen_capture(config).await;
    
    rt.shutdown_timeout(std::time::Duration::from_nanos(0));
}
