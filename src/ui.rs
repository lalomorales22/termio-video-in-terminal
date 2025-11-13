use crate::client::TermIOClient;
use crate::message::AsciiFrame;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::prelude::*;
use ratatui::widgets::*;
use std::io;
use std::time::Duration;

/// Terminal UI state
pub struct TermioUI {
    client: TermIOClient,
    input_buffer: String,
    scroll_position: u16,
    should_exit: bool,
}

impl TermioUI {
    pub fn new(client: TermIOClient) -> Self {
        Self {
            client,
            input_buffer: String::new(),
            scroll_position: 0,
            should_exit: false,
        }
    }

    /// Run the terminal UI
    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let mut term = Terminal::new(backend)?;

        // Clear screen initially
        term.clear()?;

        // Main loop
        let result = loop {
            // Draw UI
            term.draw(|f| self.draw(f))?;

            // Handle input with timeout
            if crossterm::event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key).await?;
                }
            }

            if self.should_exit {
                break Ok(());
            }
        };

        // Restore terminal
        disable_raw_mode()?;
        term.clear()?;

        result
    }

    /// Draw the UI
    fn draw(&self, f: &mut Frame) {
        let size = f.area();

        // Main layout: top for video, bottom for chat
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Min(15), Constraint::Length(8)])
            .split(size);

        self.draw_video_area(f, chunks[0]);
        self.draw_chat_area(f, chunks[1]);
    }

    /// Draw the video display area
    fn draw_video_area(&self, f: &mut Frame, area: Rect) {
        // Layout for multiple users
        let frames = self.client.last_frames.read().clone();
        let users = self.client.connected_users.read().clone();

        // Create a title showing connected users
        let title = format!("📹 Users: {}", users.join(", "));

        if frames.is_empty() {
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded);
            let paragraph = Paragraph::new("Waiting for frames...").block(block);
            f.render_widget(paragraph, area);
        } else {
            // Display each user's frame
            let frame_count = frames.len().min(2); // Display up to 2 frames side by side
            let columns = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(vec![Constraint::Percentage(50); frame_count])
                .split(area);

            for (idx, (username, frame)) in frames.iter().enumerate() {
                if idx >= frame_count {
                    break;
                }
                self.render_frame(f, columns[idx], username, frame);
            }
        }
    }

    /// Render a single ASCII frame
    fn render_frame(&self, f: &mut Frame, area: Rect, username: &str, frame: &AsciiFrame) {
        // Scale the frame to fit the area, preserving aspect ratio
        let width = area.width as usize;
        let height = area.height as usize - 3; // Leave room for border

        // Create the ASCII art text with colors
        let mut text: Vec<Line> = Vec::new();

        // Display the frame row by row
        for y in 0..frame.height.min(height as u16) {
            let mut line_spans: Vec<Span> = Vec::new();

            for x in 0..frame.width.min(width as u16) {
                if let Some((ch, r, g, b)) = frame.get_cell(x, y) {
                    // Create colored span for each character
                    let color = Color::Rgb(r, g, b);
                    let span = Span::styled(ch.to_string(), Style::default().fg(color));
                    line_spans.push(span);
                }
            }

            text.push(Line::from(line_spans));
        }

        // Create block with username - highlight if it's you
        let is_self = username == &self.client.username;
        let title = if is_self {
            format!("📹 {} (You)", username)
        } else {
            format!("📹 {}", username)
        };
        
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(if is_self {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Blue)
            });

        let paragraph = Paragraph::new(text)
            .block(block)
            .scroll((self.scroll_position, 0));

        f.render_widget(paragraph, area);
    }

    /// Draw the chat area
    fn draw_chat_area(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)])
            .split(area);

        // Messages area
        let messages = self.client.chat_messages.read().clone();

        let mut message_lines: Vec<Line> = Vec::new();
        for (username, content) in messages.iter().rev().take(10) {
            let is_self = username == &self.client.username;
            let line = if is_self {
                Line::from(vec![
                    Span::styled(format!("{}: ", username), Style::default().fg(Color::Green).bold()),
                    Span::raw(content),
                ])
            } else {
                Line::from(vec![
                    Span::styled(format!("{}: ", username), Style::default().fg(Color::Cyan).bold()),
                    Span::raw(content),
                ])
            };
            message_lines.push(line);
        }
        message_lines.reverse();

        let block = Block::default()
            .title("💬 Chat")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);

        let messages_widget = Paragraph::new(message_lines)
            .block(block)
            .scroll((self.scroll_position, 0));

        f.render_widget(messages_widget, chunks[0]);

        // Input area with instructions
        let input_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .title("Type message (Enter to send, Esc/q to quit)");

        let input = Paragraph::new(self.input_buffer.as_str())
            .block(input_block)
            .style(Style::default().fg(Color::White));

        f.render_widget(input, chunks[1]);
    }

    /// Handle keyboard input
    async fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.should_exit = true;
            }
            KeyCode::Enter => {
                if !self.input_buffer.is_empty() {
                    self.client.send_chat(self.input_buffer.clone()).await?;
                    self.input_buffer.clear();
                }
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            KeyCode::Up => {
                self.scroll_position = self.scroll_position.saturating_add(1);
            }
            KeyCode::Down => {
                self.scroll_position = self.scroll_position.saturating_sub(1);
            }
            _ => {}
        }
        Ok(())
    }
}
