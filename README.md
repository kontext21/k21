# K21-Screen

A CLI tool to handle screen captures

## Compilation

```bash
cargo build
```

## Usage

```bash
k21-screen
```
Will run and capture until process is stopp

## Options

- `--fps`: Screen refresh rate in fps (frames per second) - default: 1

## TODO

- [ ] `--output`: Output directory for screenshots
- [ ] `--help`: Show help information
- [ ] `--version`: Show version information
- [ ] `--format`: Output format (png, jpg, etc.)
- [ ] `--overwrite`: Overwrite existing files
- [ ] `--duration`: How long to capture the screen. Eg. `10s`
- [ ] `--interval`: Interval in seconds between captures
- [ ] `--count`: Number of captures to take 
- [ ] `--quality`: Output quality (0-100)