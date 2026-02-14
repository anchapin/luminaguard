# Wave 5 - Approval Cliff TUI Implementation Guide (#200)

## Overview

This document provides a complete implementation guide for **#200: Approval Cliff Terminal UI** - replacing the Phase 1 CLI prompts with a rich, interactive terminal user interface using `ratatui`.

**Status**: Phase 1 complete (CLI baseline), Phase 2 TUI ready to implement
**Branch**: `feature/200-approval-cliff-tui`
**Estimated Effort**: 40-50 hours
**Foundation**: Built on #192 Approval Cliff Module (100% complete)

---

## Architecture

### Current State (Phase 1)

```
ApprovalManager::check_and_approve_tui()
    ↓
present_tui_approval(DiffCard)
    ↓
Simple CLI prompt (sync blocking read)
    ↓
return TuiResult::Approved/Rejected
```

**Features**:
- Async-ready API (`pub async fn present_tui_approval()`)
- Simple CLI fallback (works immediately)
- Full test coverage (3 unit tests)
- Ready for ratatui integration

### Target State (Phase 2 TUI)

```
ApprovalManager::check_and_approve_tui()
    ↓
present_tui_approval(DiffCard)
    ↓
TUI Application (ratatui):
    ├─ Header: "⚠️  Action Approval Required"
    ├─ Content Area:
    │  ├─ DiffCard (with color coding)
    │  ├─ Scrolling (↑↓, Page Up/Down, Home/End)
    │  └─ Scrollbar indicator
    ├─ Footer: Keyboard shortcuts
    └─ Event Loop:
        ├─ Render on every frame
        ├─ Handle keyboard input
        └─ Return decision on Y/N/Esc
    ↓
return TuiResult::Approved/Rejected
```

---

## Implementation Roadmap

### Phase 2.1: Core TUI Framework (8 hours)

**Goal**: Build the TUI event loop and basic rendering

**Files to modify**:
- `orchestrator/src/approval/tui.rs` - Main TUI implementation

**Steps**:

1. **Define TUI State Machine**
   ```rust
   enum TuiState {
       AwaitingDecision,  // Showing diff card, waiting for input
       Approved,          // User pressed 'y'
       Rejected,          // User pressed 'n' or Esc
   }

   struct TuiContext {
       diff_card: DiffCard,
       state: TuiState,
       scroll_offset: u16,  // Line number for scrolling
   }
   ```

2. **Implement Terminal Initialization**
   ```rust
   // Setup: enable raw mode, enter alt screen, hide cursor
   enable_raw_mode()?;
   execute!(stdout, EnterAltScreen, EnableMouseCapture)?;
   let terminal = Terminal::new(CrosstermBackend::new(stdout))?;
   
   // Teardown: disable raw mode, exit alt screen, show cursor
   disable_raw_mode()?;
   execute!(backend, ExitAltScreen, DisableMouseCapture)?;
   ```

3. **Implement Event Loop**
   ```rust
   loop {
       // Render frame
       terminal.draw(|f| ui(f, &context))?;
       
       // Poll for input (non-blocking)
       if event::poll(Duration::from_millis(250))? {
           match event::read()? {
               Event::Key(key) => match key.code {
                   KeyCode::Char('y') | KeyCode::Char('Y') => return Ok(Approved),
                   KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => return Ok(Rejected),
                   KeyCode::Up => scroll_up(&mut context),
                   KeyCode::Down => scroll_down(&mut context),
                   _ => {}
               },
               _ => {}
           }
       }
   }
   ```

4. **Implement Basic Rendering**
   ```rust
   fn ui(f: &mut Frame, context: &TuiContext) {
       let chunks = Layout::default()
           .direction(Direction::Vertical)
           .constraints([
               Constraint::Length(3),   // Header
               Constraint::Min(10),     // Content
               Constraint::Length(4),   // Footer
           ])
           .split(f.area());
       
       render_header(f, chunks[0]);
       render_diff_card(f, chunks[1], context);
       render_footer(f, chunks[2]);
   }
   ```

**Tests to add**:
- `test_tui_state_machine()`
- `test_event_loop_y_approves()`
- `test_event_loop_n_rejects()`
- `test_event_loop_esc_rejects()`
- `test_scroll_up()`
- `test_scroll_down()`

**Success Criteria**:
- TUI compiles without warnings
- Event loop correctly handles Y/N/Esc input
- Terminal state is properly managed (no terminal corruption)
- Tests cover all code paths

---

### Phase 2.2: Diff Card Rendering (10 hours)

**Goal**: Render the DiffCard with proper formatting and color coding

**Files to modify**:
- `orchestrator/src/approval/tui.rs` - DiffCard rendering
- `orchestrator/src/approval/diff.rs` - No changes (reuse `to_human_readable()`)

**Steps**:

1. **Implement DiffCard Rendering**
   ```rust
   fn render_diff_card(f: &mut Frame, area: Rect, context: &TuiContext) {
       let text = context.diff_card.to_human_readable();
       let lines: Vec<&str> = text.lines().collect();
       
       // Apply scrolling
       let scroll_offset = context.scroll_offset as usize;
       let visible_lines = lines
           .iter()
           .skip(scroll_offset)
           .take(area.height as usize)
           .collect();
       
       // Render with borders
       let block = Block::default()
           .title("Action Details")
           .borders(Borders::ALL)
           .border_style(Style::default().fg(Color::Cyan));
       
       let paragraph = Paragraph::new(visible_lines)
           .block(block)
           .wrap(Wrap { trim: true });
       
       f.render_widget(paragraph, area);
   }
   ```

2. **Implement Color Coding**
   - **🟢 Green** (No approval needed): Light green text
   - **🟡 Yellow** (Low risk): Yellow text
   - **🟠 Medium** (Medium risk): Orange text
   - **🔴 Red** (High risk): Bright red text
   - **🔴🔴 Critical** (Critical): Red + blinking

   ```rust
   fn color_for_risk_level(level: RiskLevel) -> Color {
       match level {
           RiskLevel::None => Color::Green,
           RiskLevel::Low => Color::Yellow,
           RiskLevel::Medium => Color::Yellow,
           RiskLevel::High => Color::LightRed,
           RiskLevel::Critical => Color::Red,
       }
   }
   ```

3. **Implement Scrollbar**
   ```rust
   // If content is scrollable, show scrollbar on right edge
   if lines.len() > area.height as usize {
       let progress = scroll_offset as f32 / lines.len() as f32;
       let scrollbar_height = (area.height as f32 * 
           ((area.height as f32) / lines.len() as f32)) as u16;
       let scrollbar_y = (progress * ((area.height - scrollbar_height) as f32)) as u16;
       
       // Draw scrollbar (█ for position, ░ for track)
   }
   ```

4. **Implement Syntax Highlighting** (optional for Phase 2)
   - Highlight file paths (bold blue)
   - Highlight function names (bold cyan)
   - Highlight keywords (bold yellow)

**Tests to add**:
- `test_color_coding_green_actions()`
- `test_color_coding_critical_actions()`
- `test_scrollbar_position()`
- `test_line_truncation()`
- `test_word_wrap()`

**Success Criteria**:
- DiffCard renders cleanly in terminal
- Colors are visually distinct
- Scrolling works smoothly (no off-by-one errors)
- Text wrapping handles edge cases (long lines, unicode)

---

### Phase 2.3: Input Handling & Keyboard Navigation (8 hours)

**Goal**: Implement responsive keyboard handling with visual feedback

**Files to modify**:
- `orchestrator/src/approval/tui.rs` - Input handling

**Steps**:

1. **Implement Scroll Controls**
   ```rust
   match key.code {
       KeyCode::Up => context.scroll_offset = context.scroll_offset.saturating_sub(1),
       KeyCode::Down => context.scroll_offset = context.scroll_offset.saturating_add(1),
       KeyCode::PageUp => context.scroll_offset.saturating_sub(10),
       KeyCode::PageDown => context.scroll_offset.saturating_add(10),
       KeyCode::Home => context.scroll_offset = 0,
       KeyCode::End => context.scroll_offset = u16::MAX, // Clamped during render
       _ => {}
   }
   ```

2. **Implement Approval/Rejection**
   ```rust
   KeyCode::Char('y') | KeyCode::Char('Y') => {
       context.state = TuiState::Approved;
       return Ok(TuiResult::Approved);
   }
   KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
       context.state = TuiState::Rejected;
       return Ok(TuiResult::Rejected);
   }
   ```

3. **Implement Mouse Support** (optional)
   ```rust
   Event::Mouse(mouse) => match mouse.kind {
       MouseEventKind::ScrollUp => scroll_up(&mut context),
       MouseEventKind::ScrollDown => scroll_down(&mut context),
       MouseEventKind::Down(MouseButton::Left) => {
           // Check if click is on approve/reject button
       }
       _ => {}
   }
   ```

4. **Add Visual Feedback for Keypresses** (optional)
   - Highlight the decision button before confirming
   - Add animation (fade-out) when decision is made

**Tests to add**:
- `test_keyboard_y_approves()`
- `test_keyboard_n_rejects()`
- `test_keyboard_esc_rejects()`
- `test_keyboard_scroll_up()`
- `test_keyboard_scroll_down()`
- `test_keyboard_page_up()`
- `test_keyboard_page_down()`
- `test_keyboard_home()`
- `test_keyboard_end()`

**Success Criteria**:
- All keyboard commands work correctly
- No key events are lost
- Scroll position is bounded (never goes negative or past EOF)
- Visual feedback is immediate (< 50ms)

---

### Phase 2.4: UI Layout & Polish (8 hours)

**Goal**: Create a professional-looking, responsive UI layout

**Files to modify**:
- `orchestrator/src/approval/tui.rs` - Layout and styling

**Steps**:

1. **Implement Header**
   ```rust
   let header = Block::default()
       .title("⚠️  Action Approval Required")
       .title_alignment(Alignment::Center)
       .borders(Borders::BOTTOM)
       .border_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
       .style(Style::default().bg(Color::Black));
   
   let header_text = Paragraph::new(
       "Confirm the following action before proceeding"
   )
       .block(header)
       .alignment(Alignment::Center);
   ```

2. **Implement Footer with Keyboard Shortcuts**
   ```rust
   let instructions = vec![
       Line::from(vec![
           Span::styled("Y", Style::default().fg(Color::Green).add_modifier(BOLD)),
           Span::raw(" - Approve  "),
           Span::styled("N", Style::default().fg(Color::Red).add_modifier(BOLD)),
           Span::raw(" - Reject  "),
           Span::styled("↑↓", Style::default().fg(Color::Cyan)),
           Span::raw(" - Scroll  "),
           Span::styled("Esc", Style::default().fg(Color::Red).add_modifier(BOLD)),
           Span::raw(" - Cancel"),
       ]),
   ];
   ```

3. **Implement Responsive Layout**
   ```rust
   // Adjust constraints based on terminal size
   let constraints = if area.height > 20 {
       vec![
           Constraint::Length(3),   // Header
           Constraint::Min(15),     // Content
           Constraint::Length(4),   // Footer
       ]
   } else {
       vec![
           Constraint::Length(2),   // Compact header
           Constraint::Min(5),      // Compact content
           Constraint::Length(2),   // Compact footer
       ]
   };
   ```

4. **Implement Theme Support** (optional)
   - Light/dark modes
   - Color schemes (default, high-contrast, colorblind-friendly)
   - Font preferences

**Tests to add**:
- `test_layout_divides_screen_equally()`
- `test_layout_responsive_to_size()`
- `test_footer_shows_all_shortcuts()`
- `test_header_displays_title()`
- `test_colors_meet_contrast_requirements()`

**Success Criteria**:
- UI looks professional in both 80x24 and 200x50 terminals
- All text is readable (good contrast)
- Layout doesn't break on edge cases (very small windows)
- Keyboard shortcuts are clearly labeled

---

### Phase 2.5: Error Handling & Edge Cases (6 hours)

**Goal**: Handle errors gracefully and edge cases robustly

**Files to modify**:
- `orchestrator/src/approval/tui.rs` - Error handling

**Steps**:

1. **Handle Terminal Errors**
   ```rust
   // Terminal not available (not a TTY)
   if !is_terminal::is(std::io::stdout()) {
       return Err(anyhow!("Not a TTY - falling back to CLI prompt"));
   }
   
   // Handle raw mode errors
   enable_raw_mode().map_err(|e| anyhow!("Failed to enable raw mode: {}", e))?;
   ```

2. **Handle Content Errors**
   ```rust
   // Empty DiffCard
   if context.diff_card.to_human_readable().is_empty() {
       return Ok(TuiResult::Rejected); // Reject if action is unclear
   }
   
   // Content too large (> 100MB)
   if context.diff_card.to_human_readable().len() > 100 * 1024 * 1024 {
       return Err(anyhow!("Action summary too large to display"));
   }
   ```

3. **Handle Rendering Errors**
   ```rust
   // Recover from draw errors
   match terminal.draw(|f| ui(f, &context)) {
       Ok(_) => {}
       Err(e) => {
           eprintln!("TUI render error (recovering): {}", e);
           // Continue event loop
       }
   }
   ```

4. **Handle Panic Recovery**
   ```rust
   // Use panic hook to restore terminal state
   let panic_hook = std::panic::take_hook();
   std::panic::set_hook(Box::new(move |panic_info| {
       disable_raw_mode().ok();
       execute!(io::stdout(), ExitAltScreen, DisableMouseCapture).ok();
       panic_hook(panic_info);
   }));
   ```

**Tests to add**:
- `test_fallback_when_not_tty()`
- `test_empty_diff_card_rejected()`
- `test_oversized_content_error()`
- `test_render_error_recovery()`
- `test_panic_restores_terminal()`

**Success Criteria**:
- Terminal is always restored to original state (even on panic/error)
- Graceful fallback to CLI when TUI unavailable
- Error messages are helpful and actionable
- No resource leaks

---

## Integration with ApprovalManager

The TUI integrates seamlessly with the existing `ApprovalManager`:

```rust
// Usage in orchestrator code
let mut manager = ApprovalManager::new();
let decision = manager
    .check_and_approve_tui(
        ActionType::FileCreate,
        "Create /etc/config.json".to_string(),
        vec![Change::FileCreate { path: "/etc/config.json".to_string() }],
    )
    .await?;

match decision {
    ApprovalDecision::Approved => {
        // Execute the action
    }
    ApprovalDecision::Denied => {
        // Log rejection
    }
}
```

**No changes needed to ApprovalManager** - the `check_and_approve_tui()` method is already implemented and ready to use the enhanced TUI.

---

## Testing Strategy

### Unit Tests (by phase)

| Phase | Component | Tests | Coverage |
|-------|-----------|-------|----------|
| 2.1   | TUI Framework | 6 | 95%+ |
| 2.2   | DiffCard Rendering | 5 | 90%+ |
| 2.3   | Input Handling | 9 | 95%+ |
| 2.4   | UI Layout | 5 | 85%+ |
| 2.5   | Error Handling | 5 | 90%+ |
| **Total** | **All** | **30** | **90%+** |

### Integration Tests

- End-to-end approval workflow with TUI
- DiffCard rendering with ApprovalManager
- Terminal state management (setup/teardown)
- Concurrent TUI operations

### Manual Testing

- Various terminal sizes (80x24, 200x50, etc.)
- Different terminal emulators (GNOME, Konsole, iTerm2, Windows Terminal)
- Long action descriptions (1KB+, 10KB+)
- Unicode content (emojis, CJK, RTL text)
- Slow network connections (event lag)

---

## Dependencies

**Already added**:
- `ratatui` v0.30.0 - TUI framework
- `crossterm` v0.29.0 - Terminal control

**May need to add**:
- `palette` - Advanced color support (optional)
- `textwrap` - Better text wrapping (optional)
- `once_cell` - Thread-safe statics (optional)

---

## Performance Targets

- **Render time**: < 16ms per frame (60 FPS)
- **Input latency**: < 50ms keyboard-to-screen
- **Memory footprint**: < 10MB for TUI + DiffCard
- **Startup time**: < 100ms from `present_tui_approval()` call

---

## Success Criteria (Phase 2 Complete)

- ✅ All 30 TUI tests passing
- ✅ 90%+ code coverage
- ✅ Terminal properly restored on exit (even on panic)
- ✅ Keyboard shortcuts work correctly
- ✅ Diff card displays with colors
- ✅ Scrolling works smoothly
- ✅ Responsive to various terminal sizes
- ✅ No compiler warnings
- ✅ Documentation complete

---

## Timeline

| Phase | Duration | Effort |
|-------|----------|--------|
| 2.1 - Framework | 8h | High |
| 2.2 - Rendering | 10h | High |
| 2.3 - Input | 8h | Medium |
| 2.4 - Polish | 8h | Medium |
| 2.5 - Error Handling | 6h | Medium |
| **Total** | **40h** | Manageable in 1 week |

---

## Next Steps

1. **Review this guide** - Ensure architecture is clear
2. **Implement Phase 2.1** - TUI framework and event loop
3. **Test Phase 2.1** - Ensure basic TUI works
4. **Implement Phases 2.2-2.5** - Complete TUI features
5. **Integration testing** - Test with ApprovalManager
6. **Documentation** - Update CLAUDE.md with TUI examples

---

## References

- **Approval Module**: `orchestrator/src/approval/mod.rs`
- **DiffCard**: `orchestrator/src/approval/diff.rs`
- **Ratatui Docs**: https://docs.rs/ratatui/latest/ratatui/
- **Crossterm Docs**: https://docs.rs/crossterm/latest/crossterm/
- **Phase 1 Implementation**: Current branch `feature/200-approval-cliff-tui`

---

**Created**: 2026-02-14
**Status**: Phase 1 Complete, Phase 2 Ready to Implement
**Author**: Amp AI
**Next Review**: After Phase 2.1 implementation
