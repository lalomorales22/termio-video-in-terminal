# TermIO - Terminal Video Chat with ASCII Art

TermIO is a terminal-based video chat application that converts live webcam feeds into ASCII art and displays them alongside real-time chat. It builds on the video-to-ASCII conversion technology from the ASCIIVision project, creating a unique terminal-based video chat experience.

## Features

- **Live ASCII Video Streaming**: Converts webcam feeds to ASCII art in real-time
- **WebSocket Server**: Multi-user chat server handling concurrent connections
- **Real-time Chat**: Send and receive messages while viewing ASCII video streams
- **Cross-platform Support**: Works on macOS, Linux, and Windows
- **True Color Support**: Preserves RGB color information in ASCII characters
- **Async Architecture**: Built with Tokio for high-performance concurrent I/O

## Architecture

### Core Components

1. **ASCII Module** (`ascii.rs`)
   - Luminance-based character mapping (Rec. 601 standard)
   - 68-character palette for fine granularity
   - RGB color preservation with ASCII characters

2. **Webcam Module** (`webcam.rs`)
   - FFmpeg-based webcam capture
   - Cross-platform device detection
   - Background thread-based frame buffering
   - Bilinear scaling for optimal quality

3. **WebSocket Server** (`server.rs`)
   - Tokio async runtime
   - Multi-client broadcasting
   - User connection lifecycle management
   - Frame and chat message routing

4. **User Management** (`user.rs`)
   - User registration and tracking
   - Frame storage per user
   - Connection state management

5. **Client** (`client.rs`)
   - WebSocket connection handling
   - Webcam feed capture and transmission
   - Message receiving and display
   - User list management

6. **Protocol** (`message.rs`)
   - Serde-based JSON serialization
   - Message types:
     - `Join`: User registration
     - `Frame`: ASCII video frames
     - `Chat`: Text messages
     - `UserList`: Connected users
     - `UserJoined`/`UserLeft`: Connection events
     - `Ping`/`Pong`: Keep-alive

## Installation

### Option 1: Using the DMG Installer (macOS)

The easiest way to install TermIO on macOS is using the pre-built DMG file:

1. **Download or locate** `termio.dmg` in the project directory
2. **Double-click** `termio.dmg` to mount the disk image
3. **Copy** `termio-server` binary to your desired location:
   ```bash
   cp /Volumes/TermIO/termio-server /usr/local/bin/
   # or
   cp /Volumes/TermIO/termio-server ~/bin/
   ```
4. **Make it executable** (if needed):
   ```bash
   chmod +x /usr/local/bin/termio-server
   ```
5. **Eject** the DMG by dragging it to Trash or running:
   ```bash
   hdiutil detach /Volumes/TermIO
   ```

Now you can run `termio-server` from any terminal!

### Option 2: Building from Source

```bash
cd termio
cargo build --release
```

The binary will be created at `target/release/termio-server`.

## Usage

### Multi-Machine Setup (Server on One Mac, Client on Another)

This is perfect for testing live streaming between two machines on the same network!

#### Step 1: Find Your Server Mac's IP Address

On the **server Mac** (the one hosting the server), open Terminal and run:

```bash
ifconfig | grep "inet " | grep -v 127.0.0.1
```

You'll see output like:
```
inet 192.168.1.100 netmask 0xffffff00 broadcast 192.168.1.255
```

Your IP address is `192.168.1.100` (yours will be different - look for one starting with `192.168.x.x` or `10.0.x.x`).

#### Step 2: Start the Server (on Server Mac)

On your **server Mac**, run:

```bash
termio-server 0.0.0.0:8080
```

You should see output indicating the server is listening. Leave this running.

#### Step 3: Connect Client from Another Mac

On your **client Mac** (the other one), run:

```bash
termio-server client YourUsername ws://192.168.1.100:8080
```

Replace:
- `YourUsername` with any name (e.g., `Alice`)
- `192.168.1.100` with your **server Mac's actual IP** from Step 1
- `8080` with the port if you used a different one

#### Step 4: Handle Different Cameras (If Needed)

If the **client Mac** has a different camera than the server, you need to edit `src/webcam.rs` before running:

1. First, list available cameras:
   ```bash
   cargo run --release --example list_devices
   ```

2. Find your camera's device index (usually 0, 1, or 2)

3. Edit `src/webcam.rs` and change:
   ```rust
   impl Default for WebcamConfig {
       fn default() -> Self {
           Self {
               device: "0".to_string(),  // <- Change this to your camera index
               // ...
           }
       }
   }
   ```

4. Rebuild the client:
   ```bash
   cargo build --release
   ```

5. Then run the client:
   ```bash
   termio-server client YourUsername ws://SERVER_IP:8080
   ```

#### Troubleshooting Multi-Machine Connection

- **"Connection refused"**: Make sure the server is running with `0.0.0.0:8080`, not just `127.0.0.1`
- **Can't find server**: Verify the IP address with `ping 192.168.1.100`
- **Firewall blocking**: Check your Mac's firewall settings (System Preferences > Security & Privacy > Firewall)
- **Wrong camera on client**: Run the `list_devices` example to find the correct device index

### Finding Your Webcam Device

On macOS, you may have multiple video devices (cameras, displays, phone mics, etc.). To identify which device is your actual webcam:

```bash
# List all available AVFoundation devices
cargo run --release --example list_devices

# Output shows video devices with indices:
# [AVFoundation indev] AVFoundation video devices:
# [0] Brio 500
# [1] PenguinFather Camera
# [2] Capture screen 0
```

The device index (0, 1, 2, etc.) determines which camera is used.

### Start the Server

```bash
# Listen on default address (127.0.0.1:8080)
cargo run --release

# Listen on custom address
cargo run --release -- 0.0.0.0:9000
```

### Connect a Client

```bash
# Connect with username to default server (uses device 0)
cargo run --release -- client MyUsername

# Connect to custom server
cargo run --release -- client MyUsername ws://remote.host:8080

# Connect using a specific webcam device
cargo run --release -- client MyUsername ws://127.0.0.1:8080
# Then edit the device in src/webcam.rs if needed
```

### Selecting a Specific Webcam Device

Edit `src/webcam.rs` and modify the default device in `WebcamConfig::default()`:

```rust
impl Default for WebcamConfig {
    fn default() -> Self {
        Self {
            device: "0".to_string(),  // Change "0" to your device index
            // ...
        }
    }
}
```

Then rebuild:
```bash
cargo build --release
```

## Protocol Overview

All WebSocket messages are JSON-formatted with the following structure:

```json
{
  "type": "MessageType",
  "data": { /* variant-specific data */ }
}
```

### Message Types

#### Join
```json
{
  "type": "Join",
  "data": { "username": "Alice" }
}
```

#### Frame
```json
{
  "type": "Frame",
  "data": {
    "user_id": "uuid",
    "username": "Alice",
    "frame": {
      "width": 80,
      "height": 24,
      "data": [/* 4 bytes per cell: char, r, g, b */]
    }
  }
}
```

#### Chat
```json
{
  "type": "Chat",
  "data": {
    "user_id": "uuid",
    "username": "Alice",
    "content": "Hello world!"
  }
}
```

#### UserList
```json
{
  "type": "UserList",
  "data": [
    {
      "user_id": "uuid",
      "username": "Alice",
      "connected_at": "2025-10-29T12:00:00Z"
    }
  ]
}
```

## ASCII Conversion Algorithm

The video-to-ASCII conversion uses the following process:

1. **Luminance Calculation** (Rec. 601 standard):
   ```
   Y = 0.299*R + 0.587*G + 0.114*B
   ```
   This matches human eye sensitivity to different colors.

2. **Character Palette**:
   ```
   " .'`^\",:;Il!i><~+_-?][}{1)(|\\tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$"
   ```
   68 characters ordered from light to dark density.

3. **Mapping**:
   ```
   char_index = (luminance * 67) / 255
   ascii_char = PALETTE[char_index]
   ```

4. **Color Preservation**:
   Each ASCII character retains the original RGB values, enabling 24-bit true color in terminals.

## Performance Characteristics

- **Frame Processing**: Single-pass O(w × h) scan
- **Frame Buffer**: 2-8 frames (configurable)
- **Target FPS**: 30fps (configurable)
- **Memory**: ~115KB for 120×30 resolution
- **Aspect Ratio Correction**: Terminal cells are ~2:1 height/width

## Dependencies

- **tokio**: Async runtime
- **tokio-tungstenite**: WebSocket support
- **ffmpeg-next**: Video codec and scaling
- **ratatui**: Terminal UI (for future client)
- **serde/serde_json**: Serialization
- **crossbeam-channel**: Thread-safe messaging
- **parking_lot**: Synchronization primitives

## Future Enhancements

- [ ] Full terminal UI client with ratatui
- [ ] Multiple video layout modes (grid, picture-in-picture)
- [ ] Voice chat integration
- [ ] User authentication
- [ ] Persistent chat history
- [ ] Recording/playback capabilities
- [ ] Monochrome mode toggle
- [ ] Video effects (glitch, color drift)
- [ ] Docker containerization

## Platform Support

| Platform | Webcam Input | Status |
|----------|-------------|--------|
| macOS    | avfoundation | Tested |
| Linux    | v4l2 | Works |
| Windows  | dshow | Supported |

## Building from Parent Project

This module is part of the larger asciivision project. To build alongside other components:

```bash
cd /path/to/asciivision
cargo build --release --package termio
```

## Troubleshooting

### "Failed to open device" Error

**Problem**: `Failed to open device '0': No such file or directory` or similar

**Solution**:
1. List available devices: `cargo run --release --example list_devices`
2. Verify your camera appears in the list
3. Update the device index in `src/webcam.rs` if needed
4. Rebuild: `cargo build --release`

### Wrong Camera Opening

**Problem**: Client opens microphone instead of camera, or opens wrong camera

**Solution**:
1. Run the device listing utility to identify correct indices
2. Update `WebcamConfig::default()` in `src/webcam.rs` with the correct device index
3. Rebuild and test

### AVFoundation Configuration Warnings

**Message**: `Configuration of video device failed, falling back to default.`

**Explanation**: Your camera doesn't support the requested framerate or pixel format. FFmpeg automatically falls back to default settings, which is safe and usually works fine. The camera will still function normally.

### Bus Error or Connection Drops

If the client crashes with a bus error or the connection drops immediately after starting:
1. Verify the camera device is correct
2. Try with a different camera if available
3. Check that your camera isn't in use by another application

## Debugging

Enable verbose logging:

```bash
RUST_LOG=debug cargo run --release
```

Trace-level logging:

```bash
RUST_LOG=trace cargo run --release
```

Log only webcam module:

```bash
RUST_LOG=termio_server::webcam=debug cargo run --release
```

## License

Part of the ASCIIVision project. See parent directory for license information.

## Related Projects

- **ASCIIVision**: Terminal video player that inspired this project
- **MEGA-CLI**: Multi-AI chat interface using similar architecture
- **MEGA-Analytics**: Conversation analytics dashboard

## Contributing

This project demonstrates:
- Cross-platform video capture and processing
- Real-time async I/O with Tokio
- WebSocket protocol implementation
- Terminal-based UI concepts
- Video-to-ASCII conversion algorithms

Contributions welcome! Check the parent project repository for contribution guidelines.
