
k21-screen - A CLI tool to handle screen captures
k21-processor - A CLI tool to handle processing of screen captures

## Compilation

```bash
cargo build
```

## Usage

```bash
./k21-screen
./k21-processor --mp4 file.mp4
./k21-processor --image file.png
./k21-screen --stdout | ./k21-processor --stdin
```
Will run and capture until process is stopp

## Options

- `--fps`: Screen refresh rate in fps (frames per second) - default: 1

## TODO k21-screen

- [ ] `--output`: Output directory for screenshots
- [ ] `--help`: Show help information
- [ ] `--version`: Show version information
- [ ] `--format`: Output format (png, jpg, etc.)
- [ ] `--overwrite`: Overwrite existing files
- [ ] `--duration`: How long to capture the screen. Eg. `10s`
- [ ] `--interval`: Interval in seconds between captures
- [ ] `--count`: Number of captures to take 
- [ ] `--quality`: Output quality (0-100)

## ideas
- processor could just be able to handle inputs without having to specify the input type
- maybe processor could handle multiple images / inputs at once
- how do we preserve the original time stamp