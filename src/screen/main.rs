use clap::Parser;
use k21::logger::init_logger_exe;
use k21::capture::{capture_with_stdout, ScreenCaptureConfig};

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
        video_chunk_duration_in_seconds: cli.video_chunk_duration,
        save_screenshot: cli.save_screenshot,
        save_video: cli.save_video,
        record_length_in_seconds: 0,
        ..Default::default()
    };

    let _ = capture_with_stdout(config, cli.stdout).await;
    
    rt.shutdown_timeout(std::time::Duration::from_nanos(0));
}
