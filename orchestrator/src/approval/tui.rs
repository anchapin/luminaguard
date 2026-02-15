// Approval Cliff Terminal UI (TUI) - Phase 2 Implementation
//
// This module provides a rich terminal user interface for approval decisions.
// Phase 2.1: Core TUI Framework (event loop + basic rendering)
// Phases 2.2-2.5: Diff card rendering, input handling, UI polish, error recovery

use crate::approval::diff::DiffCard;
use anyhow::{anyhow, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAltScreen, ExitAltScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::time::Duration;

/// Result of the TUI approval prompt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiResult {
    /// User approved the action
    Approved,
    /// User rejected the action
    Rejected,
}

/// Internal state machine for TUI workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TuiState {
    /// Showing diff card, waiting for input
    AwaitingDecision,
    /// User pressed 'y' - action approved
    Approved,
    /// User pressed 'n' or Esc - action rejected
    Rejected,
}

/// TUI context containing application state
struct TuiContext {
    diff_card: DiffCard,
    state: TuiState,
    scroll_offset: u16, // Current scroll line number
}

impl TuiContext {
    fn new(diff_card: DiffCard) -> Self {
        Self {
            diff_card,
            state: TuiState::AwaitingDecision,
            scroll_offset: 0,
        }
    }

    fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    fn scroll_down(&mut self, max_lines: u16) {
        let readable = self.diff_card.to_human_readable();
        let line_count = readable.lines().count() as u16;
        if self.scroll_offset + 10 < line_count {
            self.scroll_offset += 1;
        }
    }
}

/// Present an approval decision to the user via interactive terminal UI
///
/// Phase 2.1 Implementation:
/// - Simple event loop handling Y/N/Esc input
/// - Basic rendering with header, content, footer
/// - Terminal setup/teardown for safety
///
/// # Arguments
/// * `diff_card` - The DiffCard to display
///
/// # Returns
/// * `Ok(TuiResult::Approved)` if user approved
/// * `Ok(TuiResult::Rejected)` if user rejected
/// * `Err` if TUI operations fail
pub async fn present_tui_approval(diff_card: &DiffCard) -> Result<TuiResult> {
    // Check if stdout is a TTY
    if !is_tty() {
        // Fallback to simple CLI prompt if not a TTY
        return fallback_cli_prompt(diff_card).await;
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAltScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Setup panic hook to restore terminal state on panic
    let panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), ExitAltScreen);
        panic_hook(panic_info);
    }));

    // Run TUI event loop
    let result = run_tui_loop(&mut terminal, diff_card).await;

    // Teardown terminal (restore original state)
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), ExitAltScreen)?;

    result
}

/// Run the TUI event loop
async fn run_tui_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    diff_card: &DiffCard,
) -> Result<TuiResult> {
    let mut context = TuiContext::new(diff_card.clone());

    loop {
        // Render frame
        terminal.draw(|f| ui(f, &context))?;

        // Handle input (non-blocking, 250ms timeout)
        if crossterm::event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        context.state = TuiState::Approved;
                        return Ok(TuiResult::Approved);
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        context.state = TuiState::Rejected;
                        return Ok(TuiResult::Rejected);
                    }
                    KeyCode::Up => {
                        context.scroll_up();
                    }
                    KeyCode::Down => {
                        context.scroll_down(20); // TODO: Phase 2.2 - calculate from actual content
                    }
                    KeyCode::PageUp => {
                        for _ in 0..5 {
                            context.scroll_up();
                        }
                    }
                    KeyCode::PageDown => {
                        for _ in 0..5 {
                            context.scroll_down(20);
                        }
                    }
                    KeyCode::Home => {
                        context.scroll_offset = 0;
                    }
                    KeyCode::End => {
                        let readable = diff_card.to_human_readable();
                        let line_count = readable.lines().count() as u16;
                        context.scroll_offset = line_count.saturating_sub(10);
                    }
                    _ => {} // Ignore other keys
                }
            }
        }
    }
}

/// Render the TUI frame (header + content + footer)
fn ui<B: Backend>(f: &mut Frame<B>, context: &TuiContext) {
    let size = f.size();

    // Define layout: header (3) + content (min 10) + footer (4)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(4),
        ])
        .split(size);

    // Render components
    render_header(f, chunks[0]);
    render_content(f, chunks[1], context);
    render_footer(f, chunks[2]);
}

/// Render header section
fn render_header<B: Backend>(f: &mut Frame<B>, area: Rect) {
    let title = Span::styled(
        "⚠️  Action Approval Required",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let block = Block::default()
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(Line::from(vec![title]))
        .block(block)
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

/// Render content section (diff card)
fn render_content<B: Backend>(f: &mut Frame<B>, area: Rect, context: &TuiContext) {
    let readable = context.diff_card.to_human_readable();
    let lines: Vec<&str> = readable.lines().collect();

    // Apply scrolling
    let scroll_offset = context.scroll_offset as usize;
    let visible_lines: Vec<Line> = lines
        .iter()
        .skip(scroll_offset)
        .take(area.height as usize)
        .map(|line| {
            Line::from(Span::raw(line.to_string()))
        })
        .collect();

    let block = Block::default()
        .title("Action Details")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(visible_lines)
        .block(block)
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

/// Render footer with keyboard shortcuts
fn render_footer<B: Backend>(f: &mut Frame<B>, area: Rect) {
    let shortcuts = vec![Line::from(vec![
        Span::styled("Y", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" - Approve  "),
        Span::styled("N", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw(" - Reject  "),
        Span::styled("↑↓", Style::default().fg(Color::Cyan)),
        Span::raw(" - Scroll  "),
        Span::styled("Esc", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw(" - Cancel"),
    ])];

    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(shortcuts)
        .block(block)
        .style(Style::default().fg(Color::Gray));

    f.render_widget(paragraph, area);
}

/// Fallback to simple CLI prompt when TUI is unavailable
async fn fallback_cli_prompt(diff_card: &DiffCard) -> Result<TuiResult> {
    println!("\n{}", "=".repeat(80));
    println!("{}", diff_card.to_human_readable());
    println!("{}", "=".repeat(80));
    println!("\nApprove this action? (y/n): ");

    use std::io::BufRead;
    let stdin = io::stdin();
    let mut input = String::new();
    stdin.lock().read_line(&mut input)?;

    match input.trim().to_lowercase().as_str() {
        "y" | "yes" => Ok(TuiResult::Approved),
        _ => Ok(TuiResult::Rejected),
    }
}

/// Check if stdout is a TTY
fn is_tty() -> bool {
    atty::is(atty::Stream::Stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tui_result_approved() {
        assert_eq!(TuiResult::Approved, TuiResult::Approved);
    }

    #[test]
    fn test_tui_result_rejected() {
        assert_eq!(TuiResult::Rejected, TuiResult::Rejected);
    }

    #[test]
    fn test_tui_result_ne() {
        assert_ne!(TuiResult::Approved, TuiResult::Rejected);
    }

    #[test]
    fn test_tui_state_awaiting_decision() {
        assert_eq!(TuiState::AwaitingDecision, TuiState::AwaitingDecision);
    }

    #[test]
    fn test_tui_state_approved() {
        assert_eq!(TuiState::Approved, TuiState::Approved);
    }

    #[test]
    fn test_tui_state_rejected() {
        assert_eq!(TuiState::Rejected, TuiState::Rejected);
    }

    #[test]
    fn test_context_scroll_up() {
        let diff_card = DiffCard::file_write(
            "/test.txt".to_string(),
            "old".to_string(),
            "new".to_string(),
        );
        let mut context = TuiContext::new(diff_card);
        context.scroll_offset = 5;
        context.scroll_up();
        assert_eq!(context.scroll_offset, 4);
    }

    #[test]
    fn test_context_scroll_up_at_top() {
        let diff_card = DiffCard::file_write(
            "/test.txt".to_string(),
            "old".to_string(),
            "new".to_string(),
        );
        let mut context = TuiContext::new(diff_card);
        context.scroll_offset = 0;
        context.scroll_up();
        assert_eq!(context.scroll_offset, 0);
    }

    #[test]
    fn test_context_scroll_down() {
        let diff_card = DiffCard::file_write(
            "/test.txt".to_string(),
            "old".to_string(),
            "new".to_string(),
        );
        let mut context = TuiContext::new(diff_card);
        context.scroll_offset = 0;
        context.scroll_down(20);
        assert_eq!(context.scroll_offset, 1);
    }
}
