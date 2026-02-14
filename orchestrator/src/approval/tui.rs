// Approval Cliff Terminal UI (TUI) - Phase 2 Implementation
//
// This module provides a rich terminal user interface for approval decisions.
// Current implementation is a placeholder for Phase 2 TUI work (#200).
// Actual ratatui integration will be completed in Phase 2.

use crate::approval::diff::DiffCard;
use anyhow::Result;

/// Result of the TUI approval prompt
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TuiResult {
    /// User approved the action
    Approved,
    /// User rejected the action
    Rejected,
}

/// Present an approval decision to the user via interactive terminal UI
///
/// Phase 1 (current): Simple CLI prompt
/// Phase 2 (future): Rich ratatui-based TUI with:
/// - Scrollable diff card display
/// - Color-coded risk levels
/// - Interactive approval/rejection buttons
/// - Keyboard navigation (↑↓ scroll, Y/N approve/reject, Esc cancel)
///
/// # Arguments
/// * `diff_card` - The DiffCard to display
///
/// # Returns
/// * `Ok(TuiResult::Approved)` if user approved
/// * `Ok(TuiResult::Rejected)` if user rejected
/// * `Err` if TUI operations fail
pub async fn present_tui_approval(diff_card: &DiffCard) -> Result<TuiResult> {
    // Phase 1: Simple CLI prompt (no ratatui yet)
    println!("\n{}", "=".repeat(80));
    println!("{}", diff_card.to_human_readable());
    println!("{}", "=".repeat(80));
    
    println!("\nApprove this action? (y/n): ");
    
    use std::io::{self, BufRead};
    let stdin = io::stdin();
    let mut input = String::new();
    stdin.lock().read_line(&mut input)?;
    
    match input.trim().to_lowercase().as_str() {
        "y" | "yes" => Ok(TuiResult::Approved),
        _ => Ok(TuiResult::Rejected),
    }
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
}
