# LensCast

A Linux desktop application (Fedora GNOME theme) to stream and control your Android phone camera via network connection.

## Features

- **Device Management**: Add, edit, delete, and connect to Android devices
- **Connection Types**: IP/Port connection or QR code pairing
- **Camera Controls**: Live preview, rotate, flip, resolution change, camera switching
- **Media Controls**: Enable/disable camera and audio streaming
- **Reconnection**: Auto-reconnect to previously used devices

## UI Layout

### Header (Top Left)
- "New Connection" button

### Sidebar (Left - Telegram-style)
- List of saved device connections
- 3-dot menu per device → Edit / Delete

### Main Area (Center)
- **New Device**: Connection wizard (IP/Port or QR pairing)
- **Connecting State**: Loading animation with device name
- **Connected State**:
  - Camera preview (top)
  - Controls bar: Rotate | Flip
  - Camera selector (dropdown)
  - Resolution selector (dropdown)
  - Separator
  - Toggle: Enable Camera / Enable Audio
  - Disconnect/Stop button (top right)

### Connection Modal
- **Step 1**: Device name input
- **Step 2**: Connection type selection (IP/Port or QR Code)
  - IP/Port: Input fields for IP and Port + Pairing code
  - QR: Display QR code for pairing

### Edit Modal
- Edit device name
- Change IP/Port
- Re-pair device (shows connection wizard)

## Architecture

- **Frontend**: Vanilla HTML/CSS/JS
- **Backend**: Rust (Tauri)
- **Protocol**: TCP/IP via ADB over network

## Prerequisites

- Rust (latest stable)
- Node.js 18+
- pnpm
- ADB (Android SDK Platform Tools)
- webkit2gtk (Linux)

## Setup

```bash
pnpm install
pnpm tauri dev
```

## License

MIT