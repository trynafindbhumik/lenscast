# LensCast

Stream and control your Android phone camera directly from your Linux desktop.

![LensCast](https://img.shields.io/badge/LensCast-v0.1.0-blue)

## Features

- 🔗 Connect to Android devices via IP/Port or QR code
- 📷 Live camera preview
- 🔄 Rotate and flip camera feed
- 📱 Switch between front and back cameras
- 🎥 Multiple resolution options
- 🎤 Audio streaming support
- 💾 Save and manage multiple devices

## Installation

### Prerequisites

- Linux (Fedora/GNOME recommended)
- [ADB](https://developer.android.com/studio/command-line/adb) installed
- Rust toolchain
- GTK 3 development libraries

### Fedora/RHEL

```bash
sudo dnf install gtk3-devel glib2-devel clutter-devel
```

### Ubuntu/Debian

```bash
sudo apt install libgtk-3-dev libglib2.0-dev
```

### Build

```bash
cargo build --release
```

## Usage

1. **Connect your Android device** via USB with USB ging enabled, or connect wirelessly
2. **Start LensCast** from your application menu or terminal
3. **Add a device** using "New Connection" button
4. **Enter device details** (IP and Port) or scan QR code
5. **Enjoy** camera streaming!

### Command Line

```bash
# Run with default settings
cargo run --release

# Development mode
cargo run
```

## Project Structure

```
lenscast/
├── src/
│   ├── main.rs       # Application entry point
│   └── lib.rs       # Main application logic
├── Cargo.toml       # Rust dependencies
├── lenscast.sh      # Utility script for setup/connection
├── README.md        # This file
├── LICENSE          # MIT License
└── DEVELOPER_README.md  # Development documentation
```

## Requirements

- Linux with GTK 3 compatible desktop environment
- Android device with USB ging or wireless ging enabled
- Network connection (same WiFi for wireless streaming)

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Supported Platforms

- Linux (Fedora, Ubuntu, Debian, and derivatives)
- Android devices (API 21+)