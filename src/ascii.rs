use crate::message::AsciiFrame;
use ffmpeg_next::util::frame::Video;

/// ASCII character palette ordered from light to dark density
/// 68 characters provide good granularity for luminance mapping
const PALETTE: &[u8] = b" .'`^\",:;Il!i><~+_-?][}{1)(|\\tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$";

/// Calculate luminance (brightness) from RGB using Rec. 601 standard
/// Matches human eye sensitivity: green (0.587) > red (0.299) > blue (0.114)
fn luminance(r: u8, g: u8, b: u8) -> u8 {
    let y = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    y.min(255.0) as u8
}

/// Select ASCII character based on RGB values
pub fn ascii_for(r: u8, g: u8, b: u8) -> char {
    let y = luminance(r, g, b) as usize;
    let idx = (y * (PALETTE.len() - 1)) / 255;
    PALETTE[idx.min(PALETTE.len() - 1)] as char
}

/// Convert RGB24 video frame to ASCII art frame
pub fn to_ascii_frame(rgb: &Video, width: u16, height: u16, mono: bool) -> AsciiFrame {
    let mut frame = AsciiFrame::new(width, height);
    let stride = rgb.stride(0) as usize;
    let data = rgb.data(0);

    for y in 0..height as usize {
        let row_offset = y * stride;
        for x in 0..width as usize {
            // Each RGB24 pixel is 3 bytes: R, G, B
            let pixel_offset = x * 3;
            let idx = row_offset + pixel_offset;

            let (r, g, b) = if idx + 2 < data.len() {
                (data[idx], data[idx + 1], data[idx + 2])
            } else {
                (0, 0, 0)
            };

            let ch = ascii_for(r, g, b);

            if mono {
                let gray = luminance(r, g, b);
                frame.set_cell(x as u16, y as u16, ch, gray, gray, gray);
            } else {
                frame.set_cell(x as u16, y as u16, ch, r, g, b);
            }
        }
    }

    frame
}

/// Simple brightness/contrast adjustment for better visibility
pub fn adjust_contrast(frame: &mut AsciiFrame, contrast: f32, brightness: i32) {
    for i in (0..frame.data.len()).step_by(4) {
        if i + 3 < frame.data.len() {
            let r = (frame.data[i + 1] as i32 + brightness).max(0).min(255) as u8;
            let g = (frame.data[i + 2] as i32 + brightness).max(0).min(255) as u8;
            let b = (frame.data[i + 3] as i32 + brightness).max(0).min(255) as u8;

            let r = ((r as f32 - 128.0) * contrast + 128.0).max(0.0).min(255.0) as u8;
            let g = ((g as f32 - 128.0) * contrast + 128.0).max(0.0).min(255.0) as u8;
            let b = ((b as f32 - 128.0) * contrast + 128.0).max(0.0).min(255.0) as u8;

            // Recalculate character based on new RGB
            let ch = ascii_for(r, g, b);
            frame.data[i] = ch as u8;
            frame.data[i + 1] = r;
            frame.data[i + 2] = g;
            frame.data[i + 3] = b;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_luminance() {
        // White
        assert_eq!(luminance(255, 255, 255), 255);
        // Black
        assert_eq!(luminance(0, 0, 0), 0);
        // Gray
        let gray = luminance(128, 128, 128);
        assert!(gray > 100 && gray < 150);
    }

    #[test]
    fn test_ascii_for() {
        // Black should map to space
        let ch = ascii_for(0, 0, 0);
        assert_eq!(ch, ' ');

        // White should map to dense character
        let ch = ascii_for(255, 255, 255);
        assert_eq!(ch, '@');
    }
}
