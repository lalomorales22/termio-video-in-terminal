mod ascii;
mod client;
mod message;
mod server;
mod user;
mod webcam;
mod ui;

use anyhow::Result;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && args[1] == "client" {
        // Client mode
        let username = if args.len() > 2 {
            args[2].clone()
        } else {
            "User".to_string()
        };

        let server_url = if args.len() > 3 {
            args[3].clone()
        } else {
            "ws://127.0.0.1:8080".to_string()
        };

        println!("Starting TermIO client as '{}' connecting to {}", username, server_url);
        let mut client = client::TermIOClient::new(username, server_url.clone());
        client.connect().await?;

        // Give the client a moment to connect
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Start the terminal UI
        println!("Starting terminal UI...");
        let mut ui = ui::TermioUI::new(client);
        ui.run().await?;

        println!("Shutting down...");
    } else {
        // Server mode (default)
        let bind_addr = if args.len() > 1 {
            &args[1]
        } else {
            "127.0.0.1:8080"
        };

        println!("Starting TermIO server on {}", bind_addr);
        println!("Clients can connect with: cargo run client <username> ws://{}", bind_addr);

        let server = Arc::new(server::TermIOServer::new());
        server.run(bind_addr).await?;
    }

    Ok(())
}
