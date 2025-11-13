use crate::message::{AsciiFrame, UserInfo};
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Represents a connected user in the TermIO session
#[derive(Debug)]
pub struct User {
    pub id: String,
    pub username: String,
    pub connected_at: String,
    pub last_frame: Arc<RwLock<Option<AsciiFrame>>>,
}

impl User {
    /// Create a new user
    pub fn new(username: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            username,
            connected_at: Utc::now().to_rfc3339(),
            last_frame: Arc::new(RwLock::new(None)),
        }
    }

    /// Get user info
    pub fn info(&self) -> UserInfo {
        UserInfo {
            user_id: self.id.clone(),
            username: self.username.clone(),
            connected_at: self.connected_at.clone(),
        }
    }

    /// Update the user's latest frame
    pub async fn update_frame(&self, frame: AsciiFrame) {
        *self.last_frame.write().await = Some(frame);
    }

    /// Get the latest frame
    pub async fn get_frame(&self) -> Option<AsciiFrame> {
        self.last_frame.read().await.clone()
    }
}

/// Manages all connected users
pub struct UserManager {
    users: Arc<RwLock<Vec<User>>>,
}

impl UserManager {
    /// Create a new user manager
    pub fn new() -> Self {
        Self {
            users: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add a new user
    pub async fn add_user(&self, username: String) -> User {
        let user = User::new(username);
        self.users.write().await.push(user.clone());
        user
    }

    /// Remove a user by ID
    pub async fn remove_user(&self, user_id: &str) -> Option<User> {
        let mut users = self.users.write().await;
        if let Some(pos) = users.iter().position(|u| u.id == user_id) {
            Some(users.remove(pos))
        } else {
            None
        }
    }

    /// Get a user by ID
    pub async fn get_user(&self, user_id: &str) -> Option<User> {
        self.users
            .read()
            .await
            .iter()
            .find(|u| u.id == user_id)
            .cloned()
    }

    /// Get all users except one (for broadcasting)
    pub async fn get_other_users(&self, exclude_id: &str) -> Vec<User> {
        self.users
            .read()
            .await
            .iter()
            .filter(|u| u.id != exclude_id)
            .map(|u| u.clone())
            .collect()
    }

    /// Get all connected users
    pub async fn get_all_users(&self) -> Vec<User> {
        self.users.read().await.clone()
    }

    /// Get user info for all users
    pub async fn get_user_list(&self) -> Vec<UserInfo> {
        self.users
            .read()
            .await
            .iter()
            .map(|u| u.info())
            .collect()
    }

    /// Check if user exists
    pub async fn user_exists(&self, user_id: &str) -> bool {
        self.users.read().await.iter().any(|u| u.id == user_id)
    }

    /// Get user count
    pub async fn count(&self) -> usize {
        self.users.read().await.len()
    }
}

impl Default for UserManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for User {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            username: self.username.clone(),
            connected_at: self.connected_at.clone(),
            last_frame: Arc::clone(&self.last_frame),
        }
    }
}

impl Clone for UserManager {
    fn clone(&self) -> Self {
        Self {
            users: Arc::clone(&self.users),
        }
    }
}
