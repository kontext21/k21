# K21 SDK

The K21 SDK has everything you need to capture end-user data, contextualize it 
and deliver real user value with AI.

More information in the [docs.kontext21.com](https://docs.kontext21.com/) and on [kontext21.com](https://kontext21.com/)

Components:
- `k21` - Core library that can be consumed by external applications
- `k21-screen` - A CLI tool to handle screen captures
- `k21-processor` - A CLI tool to handle processing of screen captures
- `k21-server` - A server that provides HTTP endpoints for screen capture and processing

## Library Usage

The `k21` library can be used as a dependency in your Rust projects:

```toml
[dependencies]
k21 = { git = "https://github.com/kontext21/k21" }
```

## CLI Tools Compilation

```bash
cargo build
```

## Usage

```bash
# Screen capture
./k21-screen
./k21-screen --fps 1 --output captures/

# Processing
./k21-processor --mp4 file.mp4
./k21-processor --image file.png
./k21-screen --stdout | ./k21-processor --stdin

# Server
./k21-server
```

## Options

### k21-screen
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

## Ideas
- processor could just be able to handle inputs without having to specify the input type
- maybe processor could handle multiple images / inputs at once
- how do we preserve the original time stamp
- add websocket support to server for real-time updates
