# Wave 4 Implementation Plan - Approval Cliff UI (Phase 2)

**Issue:** #192  
**Branch:** `feature/192-approval-cliff`  
**Worktree:** `.worktrees/wave4-approval-192`  
**Status:** Planning Phase  
**Estimated Effort:** 2-3 hours  

---

## Overview

Wave 4 implements the **Approval Cliff** - LuminaGuard's core security feature requiring explicit human approval for high-stakes actions. This completes Phase 2 and establishes the foundation for the "Agentic Engineering" paradigm.

---

## Module Architecture

### Directory Structure
```
orchestrator/src/approval/
├── mod.rs           # Module exports and public API
├── action.rs        # Action types and classification logic
├── diff.rs          # Diff card generation (before/after state)
├── history.rs       # Approval history storage and retrieval
├── ui.rs            # CLI/UI for approval prompts
└── tests.rs         # Comprehensive unit tests
```

### Module Dependencies
```
approval/
  ├── uses: action types (define in action.rs)
  ├── uses: diff generation (define in diff.rs)
  ├── uses: approval history (define in history.rs)
  ├── uses: user interaction (define in ui.rs)
  └── exports: ApprovalManager (main API)
```

---

## Implementation Roadmap

### Phase 1: Action Classification (30 min)

**File: `orchestrator/src/approval/action.rs`**

Define action types and determine approval requirements:

```rust
pub enum ActionType {
    // Green Actions (no approval needed)
    ReadFile,
    SearchWeb,
    CheckLogs,
    ListDirectory,
    GetSystemInfo,
    
    // Red Actions (require approval)
    CreateFile,
    EditFile,
    DeleteFile,
    ExecuteCommand,
    SendEmail,
    TransferAsset,
    ModifySystem,
    ExternalCall,
}

pub struct Action {
    pub action_type: ActionType,
    pub description: String,
    pub target: String,           // File path, email recipient, etc.
    pub context: serde_json::Value,  // Additional context
}

impl Action {
    pub fn requires_approval(&self) -> bool {
        match self.action_type {
            // Green actions
            ActionType::ReadFile | 
            ActionType::SearchWeb | 
            ActionType::CheckLogs | 
            ActionType::ListDirectory | 
            ActionType::GetSystemInfo => false,
            
            // Red actions
            _ => true,
        }
    }
    
    pub fn risk_level(&self) -> RiskLevel {
        match self.action_type {
            ActionType::DeleteFile | ActionType::TransferAsset => RiskLevel::Critical,
            ActionType::EditFile | ActionType::ExecuteCommand => RiskLevel::High,
            ActionType::CreateFile | ActionType::ExternalCall => RiskLevel::Medium,
            _ => RiskLevel::Low,
        }
    }
}

pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}
```

**Tests:**
- Green actions return `requires_approval() == false`
- Red actions return `requires_approval() == true`
- Risk levels assigned correctly
- Unknown actions treated conservatively (require approval)

---

### Phase 2: Diff Card Generation (45 min)

**File: `orchestrator/src/approval/diff.rs`**

Generate human-readable before/after state:

```rust
pub struct DiffCard {
    pub action_type: String,
    pub description: String,
    pub risk_level: RiskLevel,
    pub changes: Vec<Change>,
    pub timestamp: DateTime<Utc>,
}

pub enum Change {
    FileCreate {
        path: String,
        content_preview: String,
    },
    FileEdit {
        path: String,
        before: String,
        after: String,
    },
    FileDelete {
        path: String,
        size_bytes: u64,
    },
    CommandExec {
        command: String,
        args: Vec<String>,
        env_vars: Option<Vec<(String, String)>>,
    },
    EmailSend {
        to: String,
        subject: String,
        preview: String,
    },
    ExternalCall {
        method: String,
        endpoint: String,
        payload_preview: String,
    },
}

impl DiffCard {
    pub fn new(action: &Action) -> Result<Self> {
        // Generate diff based on action type
    }
    
    pub fn to_human_readable(&self) -> String {
        // Format for CLI display with colors/formatting
    }
}
```

**Features:**
- File edit: Show side-by-side diff (before/after)
- File delete: Show size and path clearly
- Command execution: Show full command line
- External calls: Show endpoint and payload
- Color-coded risk level
- Timestamp for audit trail

**Tests:**
- File creation diff generation
- File edit with actual differences
- File deletion without showing content
- Command execution formatting
- Email preview formatting
- External API call preview
- Human-readable output includes all critical info

---

### Phase 3: Approval History (30 min)

**File: `orchestrator/src/approval/history.rs`**

Store and retrieve approval decisions:

```rust
pub struct ApprovalRecord {
    pub id: String,                    // UUID
    pub timestamp: DateTime<Utc>,
    pub action_description: String,
    pub decision: ApprovalDecision,
    pub approved_by: String,           // User identifier or "system"
    pub justification: Option<String>,
    pub execution_result: Option<String>,  // Success/failure outcome
}

pub enum ApprovalDecision {
    Approved,
    Denied,
    DeferredToLater,
}

pub struct ApprovalHistory {
    records: Vec<ApprovalRecord>,
}

impl ApprovalHistory {
    pub fn new() -> Self {}
    
    pub fn record_decision(&mut self, record: ApprovalRecord) -> Result<()> {
        // Store decision with timestamp
    }
    
    pub fn get_history(&self, limit: Option<usize>) -> Vec<&ApprovalRecord> {
        // Return recent decisions
    }
    
    pub fn get_by_action(&self, description: &str) -> Vec<&ApprovalRecord> {
        // Return decisions for specific action
    }
    
    pub fn export_audit_log(&self) -> String {
        // Export as JSON or CSV for auditing
    }
}
```

**Storage:** In-memory Vec<ApprovalRecord> (Phase 3+: persistent DB)

**Tests:**
- Record approval/denial decisions
- Retrieve history with limit
- Filter by action description
- Export audit log format
- Timestamp accuracy
- Decision is immutable

---

### Phase 4: UI/CLI Interaction (30 min)

**File: `orchestrator/src/approval/ui.rs`**

Present approval prompts to user:

```rust
pub struct ApprovalPrompt;

impl ApprovalPrompt {
    pub async fn ask_for_approval(diff_card: &DiffCard) -> Result<ApprovalDecision> {
        // Display diff card and get user decision
        println!("\n{}", "═".repeat(80));
        println!("{}", diff_card.to_human_readable());
        println!("{}", "═".repeat(80));
        
        loop {
            println!("\nApprove this action? (yes/no/details): ");
            let input = read_user_input().await?;
            
            match input.trim().to_lowercase().as_str() {
                "yes" | "y" | "approve" => return Ok(ApprovalDecision::Approved),
                "no" | "n" | "deny" => return Ok(ApprovalDecision::Denied),
                "details" | "d" => {
                    println!("{}", diff_card.detailed_view());
                }
                _ => println!("Please enter: yes, no, or details"),
            }
        }
    }
}

// For testing: mock approval UI
pub struct MockApprovalUI {
    decisions: Vec<ApprovalDecision>,
    current_index: usize,
}

impl MockApprovalUI {
    pub fn new(decisions: Vec<ApprovalDecision>) -> Self {}
    
    pub fn next_decision(&mut self) -> ApprovalDecision {
        // For testing without user input
    }
}
```

**Features:**
- Clear, formatted display of diff card
- Easy approval/denial options
- "Details" option to expand information
- Support for non-interactive mode (with mock UI for testing)
- Color-coded output for different risk levels
- Keyboard shortcuts for quick decisions

**Tests:**
- Display diff card without errors
- User input parsing (yes/no/details)
- Handle interruption gracefully (Ctrl+C)
- Mock UI for deterministic testing

---

### Phase 5: Integration with Orchestrator (30 min)

**File: `orchestrator/src/approval/mod.rs`**

Main module exports and ApprovalManager:

```rust
pub mod action;
pub mod diff;
pub mod history;
pub mod ui;

#[cfg(test)]
mod tests;

pub use action::{Action, ActionType, RiskLevel};
pub use diff::DiffCard;
pub use history::{ApprovalHistory, ApprovalRecord, ApprovalDecision};
pub use ui::ApprovalPrompt;

pub struct ApprovalManager {
    history: ApprovalHistory,
    enable_approval_cliff: bool,  // Can be disabled for testing
}

impl ApprovalManager {
    pub fn new() -> Self {
        Self {
            history: ApprovalHistory::new(),
            enable_approval_cliff: true,
        }
    }
    
    pub async fn check_and_approve(
        &mut self,
        action: &Action,
    ) -> Result<ApprovalDecision> {
        if !self.enable_approval_cliff {
            return Ok(ApprovalDecision::Approved);
        }
        
        // Green actions skip approval
        if !action.requires_approval() {
            return Ok(ApprovalDecision::Approved);
        }
        
        // Generate diff card
        let diff_card = DiffCard::new(action)?;
        
        // Ask user for approval
        let decision = ApprovalPrompt::ask_for_approval(&diff_card).await?;
        
        // Record decision
        let record = ApprovalRecord {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: chrono::Utc::now(),
            action_description: action.description.clone(),
            decision: decision.clone(),
            approved_by: std::env::var("USER").unwrap_or_else(|_| "unknown".to_string()),
            justification: None,
            execution_result: None,
        };
        
        self.history.record_decision(record)?;
        
        Ok(decision)
    }
    
    pub fn disable_for_testing(&mut self) {
        self.enable_approval_cliff = false;
    }
    
    pub fn get_history(&self) -> Vec<&ApprovalRecord> {
        self.history.get_history(None)
    }
}
```

**Integration Points:**
- Main orchestrator execution pipeline
- Tool invocation (decide if tool call is red action)
- File operations (intercept writes)
- External API calls (intercept before sending)
- Agent decision execution (approve before acting)

---

### Phase 6: Comprehensive Testing (30 min)

**File: `orchestrator/src/approval/tests.rs`**

Unit tests for all modules:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    mod action_tests {
        // Test action classification
        // Test risk level assignment
        // Test approval requirements
    }
    
    mod diff_tests {
        // Test file creation diff
        // Test file edit diff
        // Test file deletion diff
        // Test command execution diff
        // Test email diff
        // Test external call diff
        // Test human-readable formatting
    }
    
    mod history_tests {
        // Test recording decisions
        // Test retrieving history
        // Test filtering by action
        // Test audit log export
        // Test immutability
    }
    
    mod ui_tests {
        // Test diff card display (using mock)
        // Test user input parsing
        // Test error handling
        // Test mock approval UI
    }
    
    mod integration_tests {
        // Test full approval workflow
        // Test green action bypass
        // Test red action approval
        // Test history recording
        // Test disable for testing
    }
}
```

**Coverage Target:** 85%+ for approval module

---

## Testing Strategy

### Unit Tests (No User Input)
```bash
cargo test --lib approval::
```

### Integration Tests (Mocked UI)
```bash
cargo test --lib approval:: -- --include-ignored
```

### Manual Testing (With CLI)
```bash
cargo run -- --enable-approval-cliff
# Test with real user input
```

---

## Implementation Checklist

- [ ] Create `action.rs` with ActionType enum and classification logic
- [ ] Create `diff.rs` with DiffCard generation
- [ ] Create `history.rs` with ApprovalRecord storage
- [ ] Create `ui.rs` with ApprovalPrompt and user interaction
- [ ] Create `mod.rs` with ApprovalManager
- [ ] Create `tests.rs` with comprehensive unit tests
- [ ] Add `pub mod approval;` to `orchestrator/src/lib.rs`
- [ ] Update CLAUDE.md with approval cliff documentation
- [ ] Update PLATFORM_SUPPORT_STATUS.md
- [ ] Verify all tests passing
- [ ] Verify rustfmt and clippy clean
- [ ] Create PR with descriptive body

---

## Success Criteria

- [x] Issue #192 created
- [ ] All module files created
- [ ] 50+ unit tests passing
- [ ] ApprovalManager integration complete
- [ ] Documentation updated
- [ ] Code quality verified (rustfmt/clippy)
- [ ] PR created and ready for review

---

## Estimated Timeline

| Phase | Task | Time | Status |
|-------|------|------|--------|
| 1 | Action Classification | 30 min | ⏳ Pending |
| 2 | Diff Card Generation | 45 min | ⏳ Pending |
| 3 | Approval History | 30 min | ⏳ Pending |
| 4 | UI/CLI Interaction | 30 min | ⏳ Pending |
| 5 | Integration | 30 min | ⏳ Pending |
| 6 | Testing | 30 min | ⏳ Pending |
| **Total** | | **~3 hours** | ⏳ Ready to start |

---

## Key Design Decisions

### 1. Action Classification
- Explicit enum for action types
- Clear green/red separation
- Conservative default (unknown → require approval)

### 2. Diff Cards
- Structured Change enum (not free-form text)
- Size limits on previews (prevent huge diffs)
- Always include timestamp and risk level

### 3. Approval History
- Start with in-memory (Vec<ApprovalRecord>)
- Design for future persistence (DB in Phase 3)
- Immutable records (no tampering)

### 4. UI Interaction
- CLI-based (works in any terminal)
- Support for interactive and non-interactive modes
- Color-coded output for clarity
- Mock UI for testing

### 5. ApprovalManager
- Single entry point for all approval logic
- Disable flag for testing (no UI prompts)
- Integrated history tracking
- Ready for async operations

---

## Future Enhancements (Phase 3+)

1. **Persistent Storage**
   - SQLite database for approval history
   - Query interface for audit logs
   - Export to JSON/CSV

2. **Policy Engine**
   - Define custom approval policies
   - Auto-approve certain actions (whitelist)
   - Escalation rules (critical actions)

3. **Multi-User Workflows**
   - Multiple approvers for critical actions
   - Approval chains/signatures
   - Role-based permissions

4. **Web UI**
   - Dashboard for approval history
   - API endpoint for remote approval
   - Real-time notifications

5. **Integration with External Systems**
   - Slack notifications of pending approvals
   - Email confirmations
   - Webhook support

---

**Ready to begin Wave 4 implementation!**

Current status: Worktree `.worktrees/wave4-approval-192` is ready. Implementation can start immediately following this plan.
