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
        help = "Directory path to save screenshots",
        value_parser
    )]
    save_screenshot_to: Option<String>,
    #[arg(
        long,
        help = "Directory path to save video",
        value_parser
    )]
    save_video_to: Option<String>,
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
        fps: Some(cli.fps),
        save_screenshot_to: cli.save_screenshot_to,
        save_video_to: cli.save_video_to,
        duration: Some(0),
        video_chunk_duration: Some(cli.video_chunk_duration),
        ..Default::default()
    };

    let _ = capture_with_stdout(config, cli.stdout).await;
    
    rt.shutdown_timeout(std::time::Duration::from_nanos(0));
}
