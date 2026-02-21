# LuminaGuard GUI Architecture

## Overview

The LuminaGuard GUI is built with **slint**, a modern Rust-based GUI framework that provides native-looking widgets, reactive state management, and excellent cross-platform support.

## Framework: slint

### Key Features Used

- **Reactive Model**: Automatic UI updates when state changes
- **Native Widgets**: Buttons, text fields, scroll views, dialogs
- **Live Preview**: slint-viewer for instant visual feedback
- **Styling System**: Custom themes (dark/light mode support)
- **Layout Engine**: Flex and Grid layouts for responsive design

## Module Structure

```
orchestrator/src/gui/
├── mod.rs              # Public API exports
├── main.rs              # Main window and application lifecycle
├── state.rs             # Global application state (reactive model)
├── approval/            # Approval Cliff UI
│   ├── mod.rs
│   ├── diff_card.rs    # Diff card component
│   ├── action_list.rs   # List of pending actions
│   └── controls.rs     # Approve/Reject buttons
├── log_viewer/          # Log viewer with filtering
│   ├── mod.rs
│   ├── log_entry.rs    # Single log entry component
│   ├── filter_panel.rs  # Filter controls
│   └── search_bar.rs   # Search functionality
├── settings/            # Settings panel
│   ├── mod.rs
│   ├── general.rs      # General settings
│   ├── security.rs     # Security configuration
│   └── network.rs      # Network settings
├── status/              # Agent status dashboard
│   ├── mod.rs
│   ├── metrics.rs      # Performance metrics
│   ├── agents.rs       # Connected agents (mesh)
│   └── health.rs       # System health indicators
├── ipc/                 # IPC communication with orchestrator
│   ├── mod.rs
│   ├── client.rs       # IPC client (gRPC/WebSocket)
│   ├── protocol.rs     # Message definitions
│   └── events.rs       # Event handlers
└── theme.rs             # Color scheme and styling
```

## IPC Mechanism

### Protocol: gRPC over Unix Domain Sockets

**Rationale:**
- gRPC provides type-safe, documented communication
- Unix domain sockets for local communication (no network overhead)
- Automatic code generation from .proto files
- Streaming support for real-time log updates

### Protocol Definition (LuminaGuard.proto)

```protobuf
package luminaguard.gui;

service Orchestrator {
  // Subscribe to real-time events
  rpc SubscribeEvents (EventFilter) returns (stream Event);

  // Approval actions
  rpc GetPendingActions (Empty) returns (ActionList);
  rpc ApproveAction (ActionId) returns (ActionResult);
  rpc RejectAction (ActionId, string reason) returns (ActionResult);

  // Log operations
  rpc GetLogs (LogFilter) returns (stream LogEntry);

  // Status
  rpc GetStatus (Empty) returns (SystemStatus);
  rpc GetMetrics (Empty) returns (Metrics);

  // Settings
  rpc GetSettings (Empty) returns (Settings);
  rpc UpdateSettings (Settings) returns (Empty);
}

message Event {
  enum Type {
    ACTION_PENDING = 0;
    ACTION_APPROVED = 1;
    ACTION_REJECTED = 2;
    LOG_ENTRY = 3;
    STATUS_UPDATE = 4;
    AGENT_CONNECTED = 5;
    AGENT_DISCONNECTED = 6;
  }

  Type type = 1;
  bytes payload = 2;
  int64 timestamp = 3;
}

message Action {
  string id = 1;
  string description = 2;
  string type = 3;  // "file_edit", "network_request", etc.
  Diff diff = 4;
  int64 timestamp = 5;
}

message Diff {
  string file_path = 1;
  string old_content = 2;
  string new_content = 3;
  repeated DiffLine lines = 4;
}

message DiffLine {
  enum Type {
    ADDED = 0;
    REMOVED = 1;
    MODIFIED = 2;
    CONTEXT = 3;
  }
  Type type = 1;
  int32 line_number = 2;
  string content = 3;
}
```

### Event Flow

```
┌─────────────┐         gRPC          ┌──────────────┐
│     GUI   │ <────────────────> │ Orchestrator │
│            │                    │              │
│  - Main   │  Unix Socket       │  - Approval  │
│  - Approval│                    │  - Agent     │
│  - Logs   │  (/tmp/lg.sock)  │  - Metrics   │
│  - Status │                    │  - IPC       │
└─────────────┘                    └──────────────┘
```

### Event Handling

GUI maintains a reactive state model:

```rust
// state.rs
#[slint::component]
pub struct AppState {
    // Approval state
    pending_actions: Vec<Action>,
    approval_history: Vec<ActionResult>,

    // Log state
    logs: Vec<LogEntry>,
    log_filter: LogFilter,

    // Status state
    agent_status: AgentStatus,
    metrics: Metrics,
    connected_agents: Vec<AgentInfo>,

    // Settings
    settings: Settings,
}
```

IPC client updates state via events:

```rust
// ipc/events.rs
pub async fn handle_events(
    mut ipc_client: OrchestratorClient,
    app_state: Rc<AppState>,
) {
    let mut event_stream = ipc_client.subscribe_events(EventFilter::all()).await?;

    while let Some(event) = event_stream.next().await {
        match event.type {
            EventType::ACTION_PENDING => {
                let action: Action = parse_payload(&event.payload);
                app_state.pending_actions.push(action.clone());
            }
            EventType::LOG_ENTRY => {
                let entry: LogEntry = parse_payload(&event.payload);
                app_state.logs.push(entry);
            }
            // ... other events
        }
    }
}
```

## UI Components

### Main Window

- **Layout**: Tab-based navigation
  - Dashboard tab (default)
  - Approval tab
  - Logs tab
  - Settings tab

- **Status Bar**:
  - Orchestrator connection status
  - Active agents count
  - CPU/Memory usage

### Approval Cliff UI

```
┌─────────────────────────────────────────────────────────────────┐
│ Approval Required                                        [X] │
├─────────────────────────────────────────────────────────────────┤
│                                                         │
│  File: /home/user/project/src/main.rs                  │
│  Type: FILE_EDIT                                       │
│                                                         │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ Diff Card                                    │   │
│  │                                              │   │
│  │ -1-  fn main() {                                │   │
│  │ +0+  fn main() {                                │   │
│  │      println!("Hello");                             │   │
│  │ +     println!("Hello, World!");                    │   │
│  │                                              │   │
│  │ +1+      println!("Hello, World!");                │   │
│  │                                              │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                         │
│  [Approve]  [Reject with reason]                     │
│                                                         │
└─────────────────────────────────────────────────────────────────┘
```

**Features:**
- Syntax highlighting for code diffs
- Line numbers
- Color coding: green (added), red (removed), blue (modified)
- Hover for full context
- Bulk approve/reject actions

### Log Viewer

```
┌─────────────────────────────────────────────────────────────────┐
│ Logs                                                   │
├─────────────────────────────────────────────────────────────────┤
│ Filter: [INFO  ▼]  Search: [_______________]      │
│ Auto-scroll: [✓]                                     │
│                                                         │
│ ┌─────────────────────────────────────────────────────┐      │
│ │ [14:32:15] INFO  Started agent #123      │      │
│ │ orch/orchestrator/src/main.rs:42              │      │
│ │                                               │      │
│ │ [14:32:16] DEBUG Loading config...       │      │
│ │ orch/orchestrator/src/config.rs:87              │      │
│ │                                               │      │
│ │ [14:32:17] WARN  Config file not found    │      │
│ │ orch/orchestrator/src/config.rs:91              │      │
│ │ Using defaults...                             │      │
│ └─────────────────────────────────────────────────────┘      │
│                                                         │
└─────────────────────────────────────────────────────────────────┘
```

**Features:**
- Log level filtering (DEBUG, INFO, WARN, ERROR)
- Search functionality
- Auto-scroll to latest
- Click to copy log entry
- Export logs (JSON, text)
- Real-time streaming

### Settings Panel

```
┌─────────────────────────────────────────────────────────────────┐
│ Settings                                                │
├─────────────────────────────────────────────────────────────────┤
│                                                         │
│ [General]  [Security]  [Network]                      │
│                                                         │
│ General:                                                │
│   Theme: ◉ Dark  ○ Light                             │
│   Auto-start orchestrator: [✓]                            │
│   Log retention: [7 days ▼]                               │
│                                                         │
│ Security:                                               │
│   Approval required for:                                    │
│     ☑ File edits                                         │
│     ☑ Network requests                                   │
│     ☑ System commands                                    │
│   Require password for approvals: [ ]                          │
│   Mesh encryption: ☑ Enable  ☑ Require signatures          │
│                                                         │
│ Network:                                               │
│   Listen address: [0.0.0.0.0:45721]                  │
│   Mesh discovery: ☑ Enable                                 │
│   Max agents: [10]                                        │
│                                                         │
│                                    [Apply]  [Reset to defaults]   │
│                                                         │
└─────────────────────────────────────────────────────────────────┘
```

### Status Dashboard

```
┌─────────────────────────────────────────────────────────────────┐
│ Dashboard                                               │
├─────────────────────────────────────────────────────────────────┤
│                                                         │
│ System Status: ● Online                                 │
│                                                         │
│ ┌────────────────┐  ┌────────────────┐  ┌──────┐ │
│ │ CPU: 23%     │  │ Memory: 145MB│  │Uptime│ │
│ │               │  │                │  │2h 15m│ │
│ │ Network: ↑1MB/s│  │ Disk: 0 B/s   │  │      │ │
│ │               │  │                │  │      │ │
│ │               │  │                │  │      │ │
│ └────────────────┘  └────────────────┘  └──────┘ │
│                                                         │
│ Connected Agents (3):                                     │
│ ┌─────────────────────────────────────────────┐            │
│ │ ● agent-researcher (192.168.1.100)    │            │
│ │ ● agent-coder (192.168.1.101)          │            │
│ │ ● agent-tester (192.168.1.102)         │            │
│ └─────────────────────────────────────────────┘            │
│                                                         │
└─────────────────────────────────────────────────────────────────┘
```

## Theme System

### Dark Theme (Default)

```rust
// theme.rs
export const DARK_THEME = slint::Theme {
    base_color: "#1e1e1e",
    text_color: "#e0e0e0",
    accent_color: "#4a9eff",
    success_color: "#4caf50",
    warning_color: "#ff9800",
    error_color: "#f44336",
    border_color: "#2d2d2d",
    background: "#121212",
}
```

### Light Theme

```rust
export const LIGHT_THEME = slint::Theme {
    base_color: "#ffffff",
    text_color: "#000000",
    accent_color: "#2196f3",
    success_color: "#4caf50",
    warning_color: "#ff9800",
    error_color: "#f44336",
    border_color: "#e0e0e0",
    background: "#f5f5f5",
}
```

## Dependencies

### Runtime Dependencies

```toml
# orchestrator/Cargo.toml

[dependencies]
slint = "1.8"                   # UI framework
tokio = "1.40"                  # Async runtime
prost = "0.12"                  # Protocol buffers
tonic = "0.11"                  # gRPC
tonic-build = "0.11"              # gRPC codegen
serde = "1.0"                   # Serialization
serde_json = "1.0"
chrono = "0.4"                   # Time handling
```

### Build Dependencies

```toml
[build-dependencies]
tonic-build = "0.11"
```

## Build System

### proto Build

```bash
# Build gRPC code from .proto
tonic-build --proto_path=proto/ proto/luminaguard.proto \
  --out_dir=orchestrator/src/ipc/ \
  --rust_out=orchestrator/src/ipc/
```

### Slint Compile

```bash
# Compile .slint files to Rust
slint-compiler src/gui/main.slint \
  --output src/gui/main.rs.slint \
  --style=all
```

## Security Considerations

### IPC Authentication

- Unix domain socket with restricted permissions (0700)
- Verify orchestrator identity on connection
- Rate limiting on sensitive actions

### UI Security

- No automatic approval of sensitive actions
- Password protection for approval actions
- Audit log of all approvals/rejections
- Clear distinction between read-only and destructive actions

## Performance Targets

- **Startup time**: <1 second (cold start)
- **UI responsiveness**: <16ms per frame (60 FPS)
- **Log update latency**: <50ms from orchestrator to UI
- **Memory footprint**: <50MB GUI process

## Accessibility

- **Keyboard navigation**: Full keyboard support (Tab, arrows, Enter, Escape)
- **Screen reader**: Label all controls with ARIA roles
- **High contrast**: WCAG AA compliant color ratios
- **Font scaling**: Support for 150%-200% zoom

## Future Enhancements

- [ ] Tray icon for quick access
- [ ] Notifications for pending approvals
- [ ] Remote control (optional)
- [ ] Plugin system for custom UI components
- [ ] Dark mode auto-detect from system
