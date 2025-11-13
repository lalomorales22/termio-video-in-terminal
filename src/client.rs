use crate::message::{AsciiFrame, Message};
use crate::webcam::{WebcamCapture, WebcamConfig};
use anyhow::Result;
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::Mutex;
use parking_lot::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage, MaybeTlsStream, WebSocketStream};
use tokio::net::TcpStream;

type WsSender = futures::stream::SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, WsMessage>;

/// Client state for a TermIO connection
pub struct TermIOClient {
    pub username: String,
    pub user_id: String,
    pub server_url: String,
    pub connected_users: Arc<RwLock<Vec<String>>>,
    pub last_frames: Arc<RwLock<std::collections::HashMap<String, AsciiFrame>>>,
    pub chat_messages: Arc<RwLock<Vec<(String, String)>>>, // (username, message)
    pub ws_sender: Arc<Mutex<Option<WsSender>>>,
}

impl TermIOClient {
    /// Create a new TermIO client
    pub fn new(username: String, server_url: String) -> Self {
        Self {
            username,
            user_id: String::new(),
            server_url,
            connected_users: Arc::new(RwLock::new(Vec::new())),
            last_frames: Arc::new(RwLock::new(std::collections::HashMap::new())),
            chat_messages: Arc::new(RwLock::new(Vec::new())),
            ws_sender: Arc::new(Mutex::new(None)),
        }
    }

    /// Connect to the TermIO server
    pub async fn connect(&mut self) -> Result<()> {
        let (ws_stream, _) = connect_async(&self.server_url).await?;
        tracing::info!("Connected to TermIO server at {}", self.server_url);

        let (ws_tx, mut ws_rx) = ws_stream.split();

        // Store the sender in Arc<Mutex<>> so it can be shared
        *self.ws_sender.lock().await = Some(ws_tx);

        // Send join message
        let join_msg = Message::Join {
            username: self.username.clone(),
        };
        {
            let mut sender = self.ws_sender.lock().await;
            if let Some(ref mut tx) = sender.as_mut() {
                tx.send(WsMessage::Text(serde_json::to_string(&join_msg)?)).await?;
            }
        }

        // Start webcam capture
        let webcam_config = WebcamConfig::default();
        let webcam = WebcamCapture::start(webcam_config)?;

        // Spawn tasks for handling messages and webcam
        let connected_users = Arc::clone(&self.connected_users);
        let last_frames = Arc::clone(&self.last_frames);
        let last_frames_webcam = Arc::clone(&self.last_frames); // Clone for webcam task
        let chat_messages = Arc::clone(&self.chat_messages);
        let username = self.username.clone();
        let username_webcam = username.clone(); // Clone for webcam task
        let ws_sender_clone = Arc::clone(&self.ws_sender);

        // Receiver task
        tokio::spawn(async move {
            while let Some(msg_result) = ws_rx.next().await {
                match msg_result {
                    Ok(WsMessage::Text(text)) => {
                        if let Ok(msg) = serde_json::from_str::<Message>(&text) {
                            match msg {
                                Message::UserList(users) => {
                                    let mut users_guard = connected_users.write();
                                    users_guard.clear();
                                    for user in users {
                                        users_guard.push(user.username);
                                    }
                                }
                                Message::Frame {
                                    username: frame_user,
                                    frame: frame_data,
                                    ..
                                } => {
                                    let mut frames = last_frames.write();
                                    frames.insert(frame_user, frame_data);
                                }
                                Message::Chat {
                                    username: chat_user,
                                    content,
                                    ..
                                } => {
                                    let mut msgs = chat_messages.write();
                                    msgs.push((chat_user, content));
                                }
                                Message::UserJoined { username: joined, .. } => {
                                    tracing::info!("{} joined the chat", joined);
                                }
                                Message::UserLeft { username: left, .. } => {
                                    tracing::info!("{} left the chat", left);
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok(WsMessage::Close(_)) => {
                        tracing::info!("Server closed connection");
                        break;
                    }
                    Err(e) => {
                        tracing::error!("WebSocket error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        // Webcam sender task (runs in background)
        let ws_sender_webcam = Arc::clone(&ws_sender_clone);
        tokio::spawn(async move {
            loop {
                if let Some(frame) = webcam.try_recv() {
                    // Store OWN frame locally so we can see it in the UI
                    {
                        let mut frames = last_frames_webcam.write();
                        frames.insert(username_webcam.clone(), frame.clone());
                    }

                    let frame_msg = Message::Frame {
                        user_id: String::new(),
                        username: username_webcam.clone(),
                        frame: frame.clone(),
                    };

                    // Send frame over WebSocket to server
                    if let Ok(json) = serde_json::to_string(&frame_msg) {
                        let mut sender = ws_sender_webcam.lock().await;
                        if let Some(ref mut tx) = sender.as_mut() {
                            if let Err(e) = tx.send(WsMessage::Text(json)).await {
                                tracing::error!("Failed to send frame: {}", e);
                                break;
                            }
                        }
                    }

                    tracing::debug!("Sent frame: {}x{}", frame.width, frame.height);
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(33)).await;
            }
        });

        Ok(())
    }

    /// Send a chat message
    pub async fn send_chat(&self, content: String) -> Result<()> {
        let msg = Message::Chat {
            user_id: self.user_id.clone(),
            username: self.username.clone(),
            content,
        };

        if let Ok(json) = serde_json::to_string(&msg) {
            let mut sender = self.ws_sender.lock().await;
            if let Some(ref mut tx) = sender.as_mut() {
                tx.send(WsMessage::Text(json)).await?;
            }
        }

        Ok(())
    }
}
