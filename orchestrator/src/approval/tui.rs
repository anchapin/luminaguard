use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

use super::{Action, ActionChanges, ApprovalDecision};

/// Run the Diff Card TUI
///
/// This function takes control of the terminal, displays the action details,
/// and waits for user approval or rejection.
pub fn run_diff_card(action: &Action) -> Result<ApprovalDecision> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app loop
    let res = run_app(&mut terminal, action);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, action: &Action) -> Result<ApprovalDecision> {
    loop {
        terminal.draw(|f| ui(f, action))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => return Ok(ApprovalDecision::Approve),
                KeyCode::Char('n') | KeyCode::Char('N') => return Ok(ApprovalDecision::Reject),
                KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                    return Ok(ApprovalDecision::Reject)
                }
                _ => {}
            }
        }
    }
}

fn ui(f: &mut Frame, action: &Action) {
    let size = f.size();

    // Vertical layout: Header, Content, Footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(3), // Header
                Constraint::Min(10),   // Content (Diff/Details)
                Constraint::Length(3), // Footer (Instructions)
            ]
            .as_ref(),
        )
        .split(size);

    // 1. Header
    let header = Paragraph::new(format!("Action Approval Required: {}", action.kind.to_string()))
        .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" ⚠️  SECURITY ALERT "),
        );
    f.render_widget(header, chunks[0]);

    // 2. Content
    let content_block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" Description: {} ", action.description));

    let content_text = match &action.changes {
        ActionChanges::FileWrite {
            path,
            old_content,
            new_content,
        } => render_diff(path, old_content, new_content),
        ActionChanges::FileDelete { path } => vec![
            Line::from(vec![
                Span::styled("DELETING FILE: ", Style::default().fg(Color::Red)),
                Span::raw(path),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "This action is permanent and cannot be undone.",
                Style::default().fg(Color::Yellow),
            )),
        ],
        ActionChanges::ExternalMessage { recipient, content } => vec![
            Line::from(vec![
                Span::styled("Recipient: ", Style::default().fg(Color::Blue)),
                Span::raw(recipient),
            ]),
            Line::from(""),
            Line::from(Span::styled("Message:", Style::default().fg(Color::Blue))),
            Line::from(content.as_str()),
        ],
        ActionChanges::Custom { description } => vec![Line::from(description.as_str())],
    };

    let paragraph = Paragraph::new(content_text)
        .block(content_block)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, chunks[1]);

    // 3. Footer
    let footer = Paragraph::new("Press 'y' to Approve, 'n' to Reject (Default)")
        .style(Style::default().fg(Color::Cyan))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

fn render_diff<'a>(path: &str, old_content: &str, new_content: &str) -> Vec<Line<'a>> {
    let mut lines = vec![];

    lines.push(Line::from(vec![
        Span::styled("File: ", Style::default().fg(Color::Blue)),
        Span::raw(path),
    ]));
    lines.push(Line::from(""));

    let diff = similar::TextDiff::from_lines(old_content, new_content);

    for change in diff.iter_all_changes() {
        let (symbol, style) = match change.tag() {
            similar::ChangeTag::Delete => ("-", Style::default().fg(Color::Red)),
            similar::ChangeTag::Insert => ("+", Style::default().fg(Color::Green)),
            similar::ChangeTag::Equal => (" ", Style::default().fg(Color::Gray)),
        };

        lines.push(Line::from(vec![
            Span::styled(symbol, style),
            Span::styled(change.value().trim_end_matches('\n'), style),
        ]));
    }

    lines
}

// Helper to display ActionKind
impl std::fmt::Display for super::ActionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            super::ActionKind::Green => write!(f, "Green (Autonomous)"),
            super::ActionKind::Red => write!(f, "Red (Requires Approval)"),
        }
    }
}
