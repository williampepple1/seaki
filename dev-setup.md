# Development Setup

## Prerequisites

1. **Node.js** (v16 or higher)
   - Download from: https://nodejs.org/
   - Verify installation: `node --version`

2. **Rust** (latest stable)
   - Install from: https://rustup.rs/
   - Verify installation: `rustc --version`

3. **Tauri CLI**
   - Install with: `npm install -g @tauri-apps/cli`
   - Or use: `cargo install tauri-cli`

## Quick Start

1. **Install Dependencies**
   ```bash
   npm install
   ```

2. **Run Development Server**
   ```bash
   npm run tauri dev
   ```

3. **Build for Production**
   ```bash
   npm run tauri build
   ```

## Project Structure

```
seaki/
├── src/                    # React frontend
│   ├── App.tsx            # Main React component
│   ├── main.tsx           # React entry point
│   └── index.css          # Styles
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── main.rs        # Tauri main
│   │   ├── network.rs     # Network discovery
│   │   ├── server.rs      # HTTP server
│   │   └── file_handler.rs # File operations
│   └── Cargo.toml         # Rust dependencies
├── uploads/               # File storage directory
├── package.json          # Node.js dependencies
└── tauri.conf.json       # Tauri configuration
```

## Features Implemented

✅ **Device Discovery**: mDNS and network scanning
✅ **File Transfer**: HTTP-based file sharing
✅ **React UI**: Modern, responsive interface
✅ **Progress Tracking**: Real-time transfer progress
✅ **Error Handling**: Comprehensive error management
✅ **Security**: Local network only, no external servers

## Testing

1. Run the app on multiple devices on the same WiFi
2. Start the server on one device
3. Discover devices from another device
4. Send files between devices

## Troubleshooting

- **Build errors**: Ensure all dependencies are installed
- **Network issues**: Check firewall settings for ports 8080-8082
- **File transfer fails**: Verify both devices have sufficient storage
