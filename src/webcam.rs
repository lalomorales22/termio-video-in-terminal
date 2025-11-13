use anyhow::{anyhow, Context, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use ffmpeg_next::codec;
use ffmpeg_next::format::Pixel;
use ffmpeg_next::media::Type;
use ffmpeg_next::software::scaling::Flags;
use ffmpeg_next::util::frame::Video;
use std::thread;
use std::ffi::{CStr, CString};

use crate::ascii;
use crate::message::AsciiFrame;

/// Configuration for webcam capture
#[derive(Debug, Clone)]
pub struct WebcamConfig {
    /// Device name (e.g., "/dev/video0" on Linux, "0" on macOS)
    pub device: String,
    /// Target width in cells
    pub width: u16,
    /// Target height in cells
    pub height: u16,
    /// FPS cap (0 = uncapped)
    pub fps_cap: u32,
    /// Monochrome mode
    pub mono: bool,
}

impl Default for WebcamConfig {
    fn default() -> Self {
        Self {
            device: "0".to_string(),
            width: 80,
            height: 24,
            fps_cap: 30,
            mono: false,
        }
    }
}

/// Webcam capture handler that spawns a background thread
pub struct WebcamCapture {
    receiver: Receiver<AsciiFrame>,
    _sender: Sender<WebcamCommand>,
}

/// Commands to control the webcam
pub enum WebcamCommand {
    Stop,
}

impl WebcamCapture {
    /// Start capturing from webcam with given configuration
    pub fn start(config: WebcamConfig) -> Result<Self> {
        let (tx, rx) = bounded::<AsciiFrame>(2);
        let (cmd_tx, cmd_rx) = bounded::<WebcamCommand>(1);

        thread::spawn(move || {
            if let Err(e) = Self::capture_loop(&config, &tx, &cmd_rx) {
                eprintln!("Webcam capture error: {}", e);
            }
        });

        Ok(Self {
            receiver: rx,
            _sender: cmd_tx,
        })
    }

    /// Main capture loop - runs in background thread
    fn capture_loop(
        config: &WebcamConfig,
        tx: &Sender<AsciiFrame>,
        cmd_rx: &Receiver<WebcamCommand>,
    ) -> Result<()> {
        ffmpeg_next::init()?;

        // Try to open the video device
        // FFmpeg input format varies by OS:
        // - Linux: "v4l2" with device "/dev/video0"
        // - macOS: "avfoundation" with device "0" or "[0]"
        // - Windows: "dshow" with device or index

        let (device_spec, format_name) = if cfg!(target_os = "macos") {
            // On macOS, avfoundation requires format specification
            // Use device index directly: "0" for video device 0
            // Optional audio can be added: "0:0" for video 0 + audio 0
            (config.device.clone(), "avfoundation")
        } else if cfg!(target_os = "linux") {
            (config.device.clone(), "v4l2")
        } else {
            (config.device.clone(), "")
        };

        tracing::info!("Opening webcam device: {} (format: {})", device_spec, format_name);

        // Create FFmpeg options dictionary
        let mut opts = ffmpeg_next::Dictionary::new();

        if cfg!(target_os = "macos") {
            // These options tell avfoundation how to configure the device
            // Most cameras support 24fps as a safe framerate
            opts.set("framerate", "24");
            // Use uyvy (yuv422) which is widely supported, fallback to default if not
            opts.set("pixel_format", "uyvy422");
            tracing::info!("Using macOS avfoundation options: framerate=24, pixel_format=uyvy422");
        }

        // Try to open the device with explicit format
        let mut ictx = if !format_name.is_empty() {
            // Use unsafe FFmpeg C API to explicitly set format
            unsafe {
                let format_cstr = CString::new(format_name)?;
                let device_cstr = CString::new(device_spec.as_str())?;

                // Find the input format
                let fmt = ffmpeg_sys_next::av_find_input_format(format_cstr.as_ptr());
                if fmt.is_null() {
                    return Err(anyhow!("Failed to find input format: {}", format_name));
                }

                // Create a mutable options dictionary pointer
                let mut options_ptr = opts.as_mut_ptr();

                // Open the input
                let mut ictx_ptr: *mut ffmpeg_sys_next::AVFormatContext = std::ptr::null_mut();
                let ret = ffmpeg_sys_next::avformat_open_input(
                    &mut ictx_ptr,
                    device_cstr.as_ptr(),
                    fmt,
                    &mut options_ptr,
                );

                if ret < 0 {
                    let mut error_buf = [0u8; 256];
                    ffmpeg_sys_next::av_strerror(ret, error_buf.as_mut_ptr() as *mut i8, error_buf.len());
                    let error_str = CStr::from_ptr(error_buf.as_ptr() as *const i8)
                        .to_string_lossy();
                    tracing::error!("Failed to open device '{}': {}", device_spec, error_str);
                    return Err(anyhow!("Failed to open device '{}': {}", device_spec, error_str));
                }

                // Convert the raw pointer to ffmpeg_next Input context
                ffmpeg_next::format::context::Input::wrap(ictx_ptr)
            }
        } else {
            ffmpeg_next::format::input_with_dictionary(&device_spec, opts)
                .map_err(|e| {
                    tracing::error!("Failed to open device '{}': {}", device_spec, e);
                    anyhow!("Failed to open webcam device '{}': {}", device_spec, e)
                })?
        };

        tracing::info!("Successfully opened webcam device: {}", device_spec);

        Self::process_frames(&mut ictx, &tx, &cmd_rx, config)
    }

    /// Process frames from the input context
    fn process_frames(
        ictx: &mut ffmpeg_next::format::context::Input,
        tx: &Sender<AsciiFrame>,
        cmd_rx: &Receiver<WebcamCommand>,
        config: &WebcamConfig,
    ) -> Result<()> {
        let video_stream = ictx
            .streams()
            .best(Type::Video)
            .ok_or_else(|| anyhow!("No video stream found"))?;

        let video_stream_idx = video_stream.index();

        let dec_ctx = codec::context::Context::from_parameters(video_stream.parameters())
            .context("Failed to create decoder context")?;
        let mut decoder = dec_ctx.decoder().video().context("Failed to get video decoder")?;

        let src_width = decoder.width() as u32;
        let src_height = decoder.height() as u32;

        // Create scaler to convert to target resolution and RGB24
        let mut scaler = ffmpeg_next::software::scaling::Context::get(
            decoder.format(),
            src_width,
            src_height,
            Pixel::RGB24,
            config.width as u32,
            config.height as u32,
            Flags::BILINEAR,
        ).context("Failed to create scaler")?;

        let mut decoded = ffmpeg_next::frame::Video::empty();
        let mut rgb = Video::new(Pixel::RGB24, config.width as u32, config.height as u32);

        // Calculate frame duration for FPS capping
        let frame_duration = if config.fps_cap > 0 {
            std::time::Duration::from_millis(1000 / config.fps_cap as u64)
        } else {
            std::time::Duration::from_millis(0)
        };

        let mut last_frame_time = std::time::Instant::now();

        // Main capture loop
        for (_stream, packet) in ictx.packets() {
            // Check for stop command
            if cmd_rx.try_recv().is_ok() {
                break;
            }

            if _stream.index() != video_stream_idx {
                continue;
            }

            match decoder.send_packet(&packet) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("Send packet error: {}", e);
                    continue;
                }
            }

            while let Ok(_) = decoder.receive_frame(&mut decoded) {
                // Apply FPS cap
                let elapsed = last_frame_time.elapsed();
                if frame_duration.as_millis() > 0 && elapsed < frame_duration {
                    let wait_time = frame_duration - elapsed;
                    thread::sleep(wait_time);
                }

                // Scale frame to target resolution
                scaler.run(&decoded, &mut rgb)?;

                // Convert to ASCII
                let frame = ascii::to_ascii_frame(&rgb, config.width, config.height, config.mono);

                // Send frame to receiver (blocking if buffer full)
                if tx.send(frame).is_err() {
                    // Receiver dropped, exit
                    return Ok(());
                }

                last_frame_time = std::time::Instant::now();
            }
        }

        Ok(())
    }

    /// Try to receive a frame without blocking
    pub fn try_recv(&self) -> Option<AsciiFrame> {
        self.receiver.try_recv().ok()
    }

    /// Receive next frame (blocking)
    pub fn recv(&self) -> Result<AsciiFrame> {
        self.receiver
            .recv()
            .map_err(|e| anyhow!("Failed to receive frame: {}", e))
    }
}

/// Detect available webcam devices
pub fn detect_devices() -> Result<Vec<String>> {
    // This is a simplified detection for common platforms
    #[cfg(target_os = "macos")]
    {
        // On macOS, typically just "0" for default device
        Ok(vec!["0".to_string()])
    }

    #[cfg(target_os = "linux")]
    {
        // Check for v4l2 devices
        let mut devices = Vec::new();
        for i in 0..10 {
            let device = format!("/dev/video{}", i);
            if std::path::Path::new(&device).exists() {
                devices.push(device);
            }
        }
        if devices.is_empty() {
            Ok(vec!["/dev/video0".to_string()])
        } else {
            Ok(devices)
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows detection is complex, just return common options
        Ok(vec!["0".to_string()])
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Ok(vec!["0".to_string()])
    }
}
