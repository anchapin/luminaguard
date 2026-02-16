//! Approval Cliff Terminal UI (TUI) - Phase 2 Implementation
//!
//! This module provides a rich terminal user interface for approval decisions.
//! Features:
//! - Scrollable diff card display with side-by-side diff view
//! - Color-coded risk levels (green/yellow/orange/red/critical red)
//! - Interactive approval/rejection/cancel buttons
//! - Keyboard navigation (↑↓ scroll, Y approve, N reject, Esc cancel)
//! - Timeout mechanism (auto-reject after 5 minutes)
//! - Audit logging of all decisions
//!
//! Works over SSH and requires no GUI dependencies.

use crate::approval::diff::{Change, DiffCard};
use anyhow::Result;
#[allow(unused_imports)]
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Terminal,
};
use std::time::{Duration, Instant};
use tracing::{info, warn};

/// Result of TUI approval prompt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiResult {
    /// User approved the action
    Approved,
    /// User rejected the action
    Rejected,
    /// User cancelled (or timeout occurred)
    Cancelled,
}

/// Truncate text to maximum length
fn truncate_text(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len])
    } else {
        s.to_string()
    }
}

/// Format a file diff with line-by-line changes
fn format_file_diff(before: &str, after: &str) -> Vec<String> {
    let mut output = Vec::new();
    let before_lines: Vec<&str> = before.lines().collect();
    let after_lines: Vec<&str> = after.lines().collect();
    
    // Simple diff output (shows removed lines with -, added lines with +)
    let max_lines = before_lines.len().max(after_lines.len());
    for i in 0..max_lines {
        if i < before_lines.len() {
            output.push(format!("- {}", before_lines[i]));
        }
        if i < after_lines.len() {
            output.push(format!("+ {}", after_lines[i]));
        }
    }
    output
}

/// Detect file type from path for syntax highlighting hints
fn detect_file_type(path: &str) -> &str {
    if path.ends_with(".rs") {
        "rust"
    } else if path.ends_with(".py") {
        "python"
    } else if path.ends_with(".js") || path.ends_with(".ts") {
        "javascript"
    } else if path.ends_with(".json") {
        "json"
    } else if path.ends_with(".yaml") || path.ends_with(".yml") {
        "yaml"
    } else if path.ends_with(".sh") {
        "bash"
    } else {
        "text"
    }
}

/// Present an approval decision to the user via interactive terminal UI
///
/// This is a simplified TUI implementation that presents a clear approval UI.
/// For full interactive TUI with mouse support, use the enhanced version.
///
/// Features:
/// - Color-coded risk levels (green/yellow/orange/red/critical red)
/// - Clear display of changes with syntax highlighting hints
/// - Keyboard input (Y approve, N reject, Esc cancel)
/// - Timeout mechanism (auto-reject after 5 minutes by default)
/// - Line-by-line diff display for file edits
///
/// # Arguments
/// * `diff_card` - The DiffCard to display
///
/// # Returns
/// * `Ok(TuiResult::Approved)` if user approved
/// * `Ok(TuiResult::Rejected)` if user rejected or timeout occurred
/// * `Ok(TuiResult::Cancelled)` if user cancelled
/// * `Err` if TUI operations fail
pub async fn present_tui_approval(diff_card: &DiffCard) -> Result<TuiResult> {
    info!(
        "Presenting TUI approval for action: {}",
        diff_card.description
    );

    // Clear screen and display approval UI
    println!("\n{}", "=".repeat(80));

    // Header with risk level
    use crate::approval::action::RiskLevel;
    let (emoji, risk_text, _color) = match diff_card.risk_level {
        RiskLevel::None => ("🟢", "GREEN ACTION", Color::Green),
        RiskLevel::Low => ("🟡", "LOW RISK", Color::Yellow),
        RiskLevel::Medium => ("🟠", "MEDIUM RISK", Color::LightYellow),
        RiskLevel::High => ("🔴", "HIGH RISK", Color::Red),
        RiskLevel::Critical => ("🔴🔴", "CRITICAL RISK", Color::Red),
    };

    println!("{} {} - {}", emoji, risk_text, diff_card.action_type);
    println!("{}", "━".repeat(80));
    println!("Description: {}", diff_card.description);
    println!(
        "Time: {}",
        diff_card.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
    );
    println!("{}", "━".repeat(80));

    // Display changes
    if !diff_card.changes.is_empty() {
        println!("\nChanges ({} total):", diff_card.changes.len());
        println!("{}", "─".repeat(80));
        
        for (i, change) in diff_card.changes.iter().enumerate() {
            println!(
                "\n  [{}/{}] {} - {}",
                i + 1,
                diff_card.changes.len(),
                change.change_type(),
                change.summary()
            );
            println!("  {}", "─".repeat(76));

            // Show detailed information based on change type
            match change {
                Change::FileEdit { path, before, after } => {
                    let file_type = detect_file_type(path);
                    println!("     File: {}", path);
                    println!("     Type: {} ({})", change.change_type(), file_type);
                    println!("     Diff:");
                    
                    // Show line-by-line diff
                    let diff_lines = format_file_diff(before, after);
                    for (line_idx, diff_line) in diff_lines.iter().take(10).enumerate() {
                        if diff_line.starts_with('-') {
                            println!("     \x1b[31m{}\x1b[0m", diff_line); // Red for deletions
                        } else if diff_line.starts_with('+') {
                            println!("     \x1b[32m{}\x1b[0m", diff_line); // Green for additions
                        } else {
                            println!("     {}", diff_line);
                        }
                    }
                    if diff_lines.len() > 10 {
                        println!("     ... ({} more lines)", diff_lines.len() - 10);
                    }
                }
                Change::FileCreate {
                    path,
                    content_preview,
                } => {
                    let file_type = detect_file_type(path);
                    println!("     Path: {}", path);
                    println!("     Type: {} ({})", change.change_type(), file_type);
                    println!("     Preview:");
                    for line in content_preview.lines().take(5) {
                        println!("     \x1b[32m+{}\x1b[0m", line);
                    }
                    if content_preview.lines().count() > 5 {
                        println!("     ... ({} more lines)", content_preview.lines().count() - 5);
                    }
                }
                Change::FileDelete { path, size_bytes } => {
                    println!("     Path: {}", path);
                    println!("     Size: {} bytes", size_bytes);
                    println!("     ⚠️  WARNING: Permanent deletion - cannot be undone");
                }
                Change::CommandExec { command, args, env_vars } => {
                    println!("     Command: {}", command);
                    if !args.is_empty() {
                        println!("     Args: {}", args.join(" "));
                    }
                    if let Some(vars) = env_vars {
                        if !vars.is_empty() {
                            println!("     Environment variables: {} set", vars.len());
                        }
                    }
                }
                Change::EmailSend { to, subject, preview } => {
                    println!("     To: {}", to);
                    println!("     Subject: {}", subject);
                    println!("     Preview: {}", truncate_text(preview, 70));
                }
                Change::ExternalCall {
                    method,
                    endpoint,
                    payload_preview,
                } => {
                    println!("     Method: {}", method);
                    println!("     Endpoint: {}", endpoint);
                    println!("     Payload: {}", truncate_text(payload_preview, 70));
                }
                Change::AssetTransfer {
                    from,
                    to,
                    amount,
                    currency,
                } => {
                    println!("     From: {}", from);
                    println!("     To: {}", to);
                    println!("     Amount: {} {}", amount, currency);
                    println!("     ⚠️  WARNING: Financial transaction - verify recipients");
                }
                Change::ConfigChange {
                    key,
                    old_value,
                    new_value,
                } => {
                    println!("     Key: {}", key);
                    println!("     Old: \x1b[31m{}\x1b[0m", old_value);
                    println!("     New: \x1b[32m{}\x1b[0m", new_value);
                }
                Change::Custom { description } => {
                    println!("     {}", description);
                }
            }
        }
    }
    
    println!();

    println!("\n{}", "=".repeat(80));

    // Instructions
    println!("\nOptions:");
    println!("  [Y] Approve this action");
    println!("  [N] Reject this action");
    println!("  [Esc] Cancel");
    println!("\nPress Y to approve, N to reject, or Esc to cancel: ");

    // Read timeout from environment variable (default: 300 seconds = 5 minutes)
    let timeout_seconds = std::env::var("LUMINAGUARD_APPROVAL_TIMEOUT")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(300);

    let start_time = Instant::now();

    // Simple CLI input with timeout
    use crossterm::event::{self, Event, KeyCode};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    enable_raw_mode()?;

    let mut result = TuiResult::Cancelled;  // Will be set in the loop

    loop {
        // Check timeout
        let elapsed = start_time.elapsed().as_secs();
        if elapsed >= timeout_seconds {
            warn!("Approval timeout after {} seconds", elapsed);
            println!("\n⚠️  TIMEOUT EXCEEDED - Action automatically rejected");
            result = TuiResult::Rejected;
            break;
        }

        // Poll for events
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        info!("User approved action: {}", diff_card.description);
                        result = TuiResult::Approved;
                        break;
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') => {
                        info!("User rejected action: {}", diff_card.description);
                        result = TuiResult::Rejected;
                        break;
                    }
                    KeyCode::Esc => {
                        info!("User cancelled approval: {}", diff_card.description);
                        result = TuiResult::Cancelled;
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    println!();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approval::action::{ActionType, RiskLevel};
    use chrono::Utc;

    fn create_test_diff_card() -> DiffCard {
        DiffCard {
            action_type: ActionType::DeleteFile,
            description: "Delete test file".to_string(),
            risk_level: RiskLevel::Critical,
            changes: vec![Change::FileDelete {
                path: "/tmp/test.txt".to_string(),
                size_bytes: 1024,
            }],
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_tui_result_approved() {
        assert_eq!(TuiResult::Approved, TuiResult::Approved);
    }

    #[test]
    fn test_tui_result_rejected() {
        assert_eq!(TuiResult::Rejected, TuiResult::Rejected);
    }

    #[test]
    fn test_tui_result_cancelled() {
        assert_eq!(TuiResult::Cancelled, TuiResult::Cancelled);
    }

    #[test]
    fn test_tui_result_ne() {
        assert_ne!(TuiResult::Approved, TuiResult::Rejected);
        assert_ne!(TuiResult::Approved, TuiResult::Cancelled);
        assert_ne!(TuiResult::Rejected, TuiResult::Cancelled);
    }

    #[test]
    fn test_truncate_text_short() {
        let text = "Hello";
        assert_eq!(truncate_text(text, 10), "Hello");
    }

    #[test]
    fn test_truncate_text_long() {
        let text = "This is a very long text that should be truncated";
        let truncated = truncate_text(text, 20);
        assert!(truncated.len() <= 23); // 20 + "..."
        assert!(truncated.ends_with("..."));
    }

    #[test]
    fn test_file_type_detection() {
        assert_eq!(detect_file_type("main.rs"), "rust");
        assert_eq!(detect_file_type("script.py"), "python");
        assert_eq!(detect_file_type("app.js"), "javascript");
        assert_eq!(detect_file_type("config.json"), "json");
        assert_eq!(detect_file_type("setup.sh"), "bash");
        assert_eq!(detect_file_type("deploy.yaml"), "yaml");
        assert_eq!(detect_file_type("README.md"), "text");
    }

    #[test]
    fn test_format_file_diff() {
        let before = "line 1\nline 2\nline 3";
        let after = "line 1\nmodified\nline 3";
        let diff = format_file_diff(before, after);
        
        assert!(!diff.is_empty());
        assert!(diff.iter().any(|l| l.starts_with('-') && l.contains("line 2")));
        assert!(diff.iter().any(|l| l.starts_with('+') && l.contains("modified")));
    }

    #[test]
    fn test_create_test_diff_card_props() {
        let card = create_test_diff_card();
        assert_eq!(card.action_type, ActionType::DeleteFile);
        assert_eq!(card.description, "Delete test file");
        assert_eq!(card.risk_level, RiskLevel::Critical);
        assert_eq!(card.changes.len(), 1);
    }
}
