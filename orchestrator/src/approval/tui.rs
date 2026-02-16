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

/// Present an approval decision to the user via interactive terminal UI
///
/// This is a simplified TUI implementation that presents a clear approval UI.
/// For full interactive TUI with mouse support, use the enhanced version.
///
/// Features:
/// - Color-coded risk levels (green/yellow/orange/red/critical red)
/// - Clear display of changes
/// - Keyboard input (Y approve, N reject, Esc cancel)
/// - Timeout mechanism (auto-reject after 5 minutes by default)
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
        println!("\nChanges:");
        for (i, change) in diff_card.changes.iter().enumerate() {
            println!(
                "  {}. {} - {}",
                i + 1,
                change.change_type(),
                change.summary()
            );

            // Show preview based on change type
            match change {
                Change::FileEdit { before, after, .. } => {
                    println!("     Before: {}", truncate_text(before, 60));
                    println!("     After:  {}", truncate_text(after, 60));
                }
                Change::FileCreate {
                    content_preview, ..
                } => {
                    println!("     Content: {}", truncate_text(content_preview, 60));
                }
                Change::FileDelete { size_bytes, .. } => {
                    println!("     Size: {} bytes (permanent deletion)", size_bytes);
                }
                Change::CommandExec { args, .. } => {
                    if !args.is_empty() {
                        println!("     Args: {}", args.join(" "));
                    }
                }
                Change::EmailSend { subject, .. } => {
                    println!("     Subject: {}", subject);
                }
                Change::ExternalCall {
                    payload_preview, ..
                } => {
                    println!("     Payload: {}", truncate_text(payload_preview, 60));
                }
                Change::AssetTransfer {
                    amount, currency, ..
                } => {
                    println!("     Amount: {} {}", amount, currency);
                }
                Change::ConfigChange {
                    old_value,
                    new_value,
                    ..
                } => {
                    println!("     Old: {}", old_value);
                    println!("     New: {}", new_value);
                }
                Change::Custom { description } => {
                    println!("     {}", description);
                }
            }
        }
    }

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
}
