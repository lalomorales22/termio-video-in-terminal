use crate::message::Message;
use crate::user::{User, UserManager};
use anyhow::Result;
use futures::{SinkExt, StreamExt};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsMessage;

/// Main TermIO server
pub struct TermIOServer {
    user_manager: UserManager,
    /// Map of user_id -> broadcast sender
    connections: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Message>>>>,
}

impl TermIOServer {
    /// Create a new TermIO server
    pub fn new() -> Self {
        Self {
            user_manager: UserManager::new(),
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the server and listen for connections
    pub async fn run(&self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        tracing::info!("TermIO server listening on {}", addr);

        loop {
            let (stream, peer_addr) = listener.accept().await?;
            tracing::info!("New connection from {}", peer_addr);

            let server = Self::new();
            let user_mgr = self.user_manager.clone();
            let conns = Arc::clone(&self.connections);

            tokio::spawn(async move {
                if let Err(e) = server.handle_connection(stream, user_mgr, conns).await {
                    tracing::error!("Connection error: {}", e);
                }
            });
        }
    }

    /// Handle a single client connection
    async fn handle_connection(
        &self,
        stream: TcpStream,
        user_manager: UserManager,
        connections: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Message>>>>,
    ) -> Result<()> {
        let ws_stream = tokio_tungstenite::accept_async(stream).await?;
        tracing::debug!("WebSocket connection established");

        let (mut ws_tx, mut ws_rx) = ws_stream.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

        let mut user: Option<User> = None;

        // Handle incoming messages
        loop {
            tokio::select! {
                msg = ws_rx.next() => {
                    match msg {
                        Some(Ok(ws_msg)) => {
                            match ws_msg {
                                WsMessage::Text(text) => {
                                    match serde_json::from_str::<Message>(&text) {
                                        Ok(msg) => {
                                            match msg {
                                                Message::Join { username } => {
                                                    // Create and register user
                                                    let new_user = user_manager.add_user(username.clone()).await;
                                                    let user_id = new_user.id.clone();

                                                    tracing::info!("User {} joined: {}", user_id, username);

                                                    // Store connection
                                                    connections.write().insert(user_id.clone(), tx.clone());
                                                    user = Some(new_user.clone());

                                                    // Send acknowledgment
                                                    let ack = Message::Ack {
                                                        success: true,
                                                        message: format!("Welcome, {}!", username),
                                                    };
                                                    let _ = ws_tx.send(WsMessage::Text(
                                                        serde_json::to_string(&ack)?,
                                                    )).await;

                                                    // Broadcast user list to all
                                                    let user_list = user_manager.get_user_list().await;
                                                    let list_msg = Message::UserList(user_list);
                                                    Self::broadcast_to_all(
                                                        &connections,
                                                        &list_msg,
                                                    )?;

                                                    // Notify others
                                                    let joined = Message::UserJoined {
                                                        user_id: new_user.id,
                                                        username: new_user.username,
                                                    };
                                                    Self::broadcast_except(
                                                        &connections,
                                                        &joined,
                                                        &user_id,
                                                    )?;
                                                }
                                                Message::Frame { frame, user_id: _, username: _ } => {
                                                    if let Some(ref u) = user {
                                                        u.update_frame(frame.clone()).await;

                                                        // Broadcast to ALL including the sender
                                                        let frame_msg = Message::Frame {
                                                            user_id: u.id.clone(),
                                                            username: u.username.clone(),
                                                            frame: frame.clone(),
                                                        };
                                                        Self::broadcast_to_all(
                                                            &connections,
                                                            &frame_msg,
                                                        )?;
                                                    }
                                                }
                                                Message::Chat { content, .. } => {
                                                    if let Some(ref u) = user {
                                                        // Broadcast to all
                                                        let chat_msg = Message::Chat {
                                                            user_id: u.id.clone(),
                                                            username: u.username.clone(),
                                                            content,
                                                        };
                                                        Self::broadcast_to_all(
                                                            &connections,
                                                            &chat_msg,
                                                        )?;
                                                    }
                                                }
                                                Message::Ping => {
                                                    let _ = ws_tx.send(WsMessage::Text(
                                                        serde_json::to_string(&Message::Pong)?,
                                                    )).await;
                                                }
                                                _ => {}
                                            }
                                        }
                                        Err(e) => {
                                            tracing::warn!("Failed to parse message: {}", e);
                                        }
                                    }
                                }
                                WsMessage::Close(_) => {
                                    if let Some(ref u) = user {
                                        user_manager.remove_user(&u.id).await;
                                        connections.write().remove(&u.id);

                                        tracing::info!("User {} disconnected: {}", u.id, u.username);

                                        // Broadcast user left
                                        let left_msg = Message::UserLeft {
                                            user_id: u.id.clone(),
                                            username: u.username.clone(),
                                        };
                                        Self::broadcast_to_all(&connections, &left_msg)?;

                                        // Update user list
                                        let user_list = user_manager.get_user_list().await;
                                        let list_msg = Message::UserList(user_list);
                                        Self::broadcast_to_all(&connections, &list_msg)?;
                                    }
                                    break;
                                }
                                _ => {}
                            }
                        }
                        Some(Err(e)) => {
                            tracing::error!("WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            tracing::debug!("WebSocket closed by client");
                            break;
                        }
                    }
                }

                Some(msg) = rx.recv() => {
                    if let Ok(json) = serde_json::to_string(&msg) {
                        let _ = ws_tx.send(WsMessage::Text(json)).await;
                    }
                }
            }
        }

        // Clean up on disconnect
        if let Some(ref u) = user {
            user_manager.remove_user(&u.id).await;
            connections.write().remove(&u.id);

            let left_msg = Message::UserLeft {
                user_id: u.id.clone(),
                username: u.username.clone(),
            };
            let _ = Self::broadcast_to_all(&connections, &left_msg);

            let user_list = user_manager.get_user_list().await;
            let list_msg = Message::UserList(user_list);
            let _ = Self::broadcast_to_all(&connections, &list_msg);
        }

        Ok(())
    }

    /// Broadcast a message to all connected users
    fn broadcast_to_all(
        connections: &Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Message>>>>,
        message: &Message,
    ) -> Result<()> {
        let conns = connections.read();
        for tx in conns.values() {
            let _ = tx.send(message.clone());
        }
        Ok(())
    }

    /// Broadcast a message to all except one user
    fn broadcast_except(
        connections: &Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Message>>>>,
        message: &Message,
        except_id: &str,
    ) -> Result<()> {
        let conns = connections.read();
        for (id, tx) in conns.iter() {
            if id != except_id {
                let _ = tx.send(message.clone());
            }
        }
        Ok(())
    }
}

impl Default for TermIOServer {
    fn default() -> Self {
        Self::new()
    }
}
