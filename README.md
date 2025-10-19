# Seaki - Local File Sharing

A local file sharing application built with Tauri, Rust, and React that allows people on the same WiFi network to send files to each other without consuming internet data, similar to Xender.

## Features

- 🔍 **Device Discovery**: Automatically discover devices on the same WiFi network
- 📁 **File Sharing**: Send files of any size between devices
- 🚀 **Fast Transfer**: Direct peer-to-peer file transfer without internet
- 🔒 **Secure**: Files are transferred directly between devices on your local network
- 💻 **Cross-Platform**: Works on Windows, macOS, and Linux

## How it Works

1. **Start the Server**: Click "Start Server" to make your device discoverable
2. **Discover Devices**: The app automatically scans for other Seaki instances on your network
3. **Select File**: Choose any file from your device
4. **Send File**: Select a target device and send the file directly

## Technology Stack

- **Frontend**: React with TypeScript
- **Backend**: Rust with Tauri
- **Networking**: HTTP server with mDNS discovery
- **File Transfer**: Chunked upload/download for large files

## Getting Started

### Prerequisites

- Node.js (v16 or higher)
- Rust (latest stable)
- Tauri CLI

### Installation

1. Clone the repository:
```bash
git clone <repository-url>
cd seaki
```

2. Install dependencies:
```bash
npm install
```

3. Run the development server:
```bash
npm run tauri dev
```

### Building for Production

```bash
npm run tauri build
```

## Usage

1. **Start the Application**: Launch Seaki on all devices you want to share files between
2. **Enable Server**: Click "Start Server" to make your device discoverable
3. **Discover Devices**: The app will automatically find other Seaki instances on your network
4. **Share Files**: Select a file, choose a target device, and send!

## Network Requirements

- All devices must be connected to the same WiFi network
- No internet connection required for file transfers
- Firewall settings may need to be adjusted to allow local network communication

## Security

- Files are transferred directly between devices on your local network
- No data is sent to external servers
- All transfers are encrypted within your local network

## Troubleshooting

- **No devices found**: Ensure all devices are on the same WiFi and have Seaki running
- **Connection failed**: Check firewall settings and ensure ports 8080-8082 are not blocked
- **File transfer failed**: Verify both devices have sufficient storage space

## License

This project is open source and available under the MIT License.
