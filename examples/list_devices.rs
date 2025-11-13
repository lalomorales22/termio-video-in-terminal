// Quick utility to list available webcam devices on this machine
use std::process::Command;

fn main() {
    println!("Detecting available video devices...\n");

    #[cfg(target_os = "macos")]
    {
        println!("macOS detected - Using avfoundation\n");

        // Try to list devices using ffmpeg
        println!("Running: ffmpeg -f avfoundation -list_devices true -i \"\" 2>&1\n");
        println!("========================================\n");

        let output = Command::new("ffmpeg")
            .args(&["-f", "avfoundation", "-list_devices", "true", "-i", ""])
            .output();

        match output {
            Ok(out) => {
                // FFmpeg outputs to stderr for this command
                let stderr = String::from_utf8_lossy(&out.stderr);
                println!("{}", stderr);
            }
            Err(e) => {
                eprintln!("Error running ffmpeg: {}", e);
                println!("\nAlternatively, try these manually:");
                println!("  $ ffmpeg -f avfoundation -list_devices true -i \"\"");
            }
        }

        println!("========================================\n");
        println!("Once you identify your camera, use it like:");
        println!("  cargo run --release -- client <username> <device_number>");
        println!("\nExample (if Brio 500 is device 2):");
        println!("  cargo run --release -- client lalo :2");
    }

    #[cfg(target_os = "linux")]
    {
        println!("Linux detected - Using v4l2\n");
        println!("Checking for /dev/video* devices:");
        for i in 0..10 {
            let path = format!("/dev/video{}", i);
            if std::path::Path::new(&path).exists() {
                println!("  Found: {}", path);
            }
        }
        println!("\nUse like: cargo run --release -- client <username> /dev/video0");
    }

    #[cfg(target_os = "windows")]
    {
        println!("Windows detected - Using dshow\n");
        println!("Run this to list devices:");
        println!("$ ffmpeg -f dshow -list_devices true -i dummy");
    }
}
