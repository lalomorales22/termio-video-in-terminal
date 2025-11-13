use serde::{Deserialize, Serialize};

/// WebSocket message types for TermIO protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Message {
    /// Client joins the server with a username
    Join {
        username: String,
    },

    /// Frame of ASCII video data from a user
    Frame {
        user_id: String,
        username: String,
        frame: AsciiFrame,
    },

    /// Chat message from a user
    Chat {
        user_id: String,
        username: String,
        content: String,
    },

    /// List of currently connected users
    UserList(Vec<UserInfo>),

    /// User has disconnected
    UserLeft {
        user_id: String,
        username: String,
    },

    /// User connected notification
    UserJoined {
        user_id: String,
        username: String,
    },

    /// Acknowledgment/Error message
    Ack {
        success: bool,
        message: String,
    },

    /// Keep-alive ping
    Ping,

    /// Keep-alive pong
    Pong,
}

/// ASCII video frame data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsciiFrame {
    pub width: u16,
    pub height: u16,
    /// Compressed frame: stores (char, r, g, b) for each cell
    /// Format: each cell is [char_byte, r, g, b]
    pub data: Vec<u8>,
}

impl AsciiFrame {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            data: vec![0; width as usize * height as usize * 4],
        }
    }

    /// Set a cell at (x, y) with character and RGB color
    pub fn set_cell(&mut self, x: u16, y: u16, ch: char, r: u8, g: u8, b: u8) {
        let idx = (y as usize * self.width as usize + x as usize) * 4;
        if idx + 3 < self.data.len() {
            self.data[idx] = ch as u8;
            self.data[idx + 1] = r;
            self.data[idx + 2] = g;
            self.data[idx + 3] = b;
        }
    }

    /// Get cell at (x, y)
    pub fn get_cell(&self, x: u16, y: u16) -> Option<(char, u8, u8, u8)> {
        let idx = (y as usize * self.width as usize + x as usize) * 4;
        if idx + 3 < self.data.len() {
            Some((
                self.data[idx] as char,
                self.data[idx + 1],
                self.data[idx + 2],
                self.data[idx + 3],
            ))
        } else {
            None
        }
    }
}

/// Information about a connected user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub user_id: String,
    pub username: String,
    pub connected_at: String,
}
