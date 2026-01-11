# WGPUI E2E Test Plan

Comprehensive catalog of test scenarios for the WGPUI component library. Each test should be implemented using the testing framework DSL.

---

## Table of Contents

1. [Foundation Component Tests](#1-foundation-component-tests)
2. [Atom Component Tests](#2-atom-component-tests)
3. [Molecule Component Tests](#3-molecule-component-tests)
4. [Organism Component Tests](#4-organism-component-tests)
5. [Section Component Tests](#5-section-component-tests)
6. [HUD Component Tests](#6-hud-component-tests)
7. [User Flow Tests](#7-user-flow-tests)
8. [Keyboard Navigation Tests](#8-keyboard-navigation-tests)
9. [Animation & Timing Tests](#9-animation--timing-tests)
10. [Edge Cases & Error States](#10-edge-cases--error-states)
11. [Performance Tests](#11-performance-tests)
12. [Accessibility Tests](#12-accessibility-tests)

---

## 1. Foundation Component Tests

### 1.1 Button Component

#### Basic States
```rust
test("Button - Default State")
    .expect("#primary-button")
    .expect_visible("#primary-button")

test("Button - Hover State")
    .hover("#primary-button")
    .wait(100)
    // Verify hover style applied

test("Button - Click Interaction")
    .click("#submit-button")
    .expect("#success-indicator")

test("Button - Disabled State")
    .click("#disabled-button")
    // Verify no action occurs
    .expect_not("#action-result")
```

#### Variant Tests
| Test Name | Description |
|-----------|-------------|
| `Button - Primary Variant` | Verify primary button styling |
| `Button - Secondary Variant` | Verify secondary styling |
| `Button - Ghost Variant` | Verify transparent ghost styling |
| `Button - Danger Variant` | Verify red danger styling |
| `Button - With Icon` | Button renders icon correctly |
| `Button - Icon Only` | Icon-only button sizing |

#### Interaction Sequences
| Test Name | Description |
|-----------|-------------|
| `Button - Rapid Clicks` | Multiple rapid clicks handled correctly |
| `Button - Click and Hold` | Mouse down without release |
| `Button - Click Outside` | Mouse down inside, release outside |

---

### 1.2 TextInput Component

#### Basic Input
```rust
test("TextInput - Type Characters")
    .click("#email-input")
    .type_text("hello@example.com")
    .expect_text("#email-input", "hello@example.com")

test("TextInput - Backspace Delete")
    .click("#text-field")
    .type_text("hello")
    .press_backspace()
    .press_backspace()
    .expect_text("#text-field", "hel")

test("TextInput - Submit on Enter")
    .click("#search-input")
    .type_text("search query")
    .press_enter()
    .expect("#search-results")
```

#### Cursor Movement
| Test Name | Description |
|-----------|-------------|
| `TextInput - Arrow Left/Right` | Cursor moves within text |
| `TextInput - Home Key` | Cursor jumps to start |
| `TextInput - End Key` | Cursor jumps to end |
| `TextInput - Select All (Ctrl+A)` | All text selected |
| `TextInput - Delete Forward` | Delete key removes char ahead |

#### Focus Management
| Test Name | Description |
|-----------|-------------|
| `TextInput - Click to Focus` | Clicking focuses the input |
| `TextInput - Tab to Focus` | Tab key moves focus to input |
| `TextInput - Blur on Click Outside` | Focus lost when clicking elsewhere |
| `TextInput - Focus Ring Visible` | Visual focus indicator shown |

#### Edge Cases
| Test Name | Description |
|-----------|-------------|
| `TextInput - Empty Input Submit` | Submit with no text |
| `TextInput - Very Long Text` | Text longer than input width |
| `TextInput - Paste Text` | Ctrl+V pastes from clipboard |
| `TextInput - Special Characters` | Unicode, emojis, symbols |
| `TextInput - Placeholder Text` | Placeholder shown when empty |

---

### 1.3 Dropdown Component

#### Basic Selection
```rust
test("Dropdown - Open on Click")
    .click("#country-dropdown")
    .expect_visible("#dropdown-options")

test("Dropdown - Select Option")
    .click("#country-dropdown")
    .click("text:United States")
    .expect_text("#country-dropdown", "United States")

test("Dropdown - Close on Escape")
    .click("#country-dropdown")
    .press_escape()
    .expect_not_visible("#dropdown-options")
```

#### Keyboard Navigation
| Test Name | Description |
|-----------|-------------|
| `Dropdown - Arrow Down` | Highlight moves down |
| `Dropdown - Arrow Up` | Highlight moves up |
| `Dropdown - Enter to Select` | Selects highlighted option |
| `Dropdown - Type to Filter` | Typing filters options |
| `Dropdown - Wrap Around` | Arrow at end wraps to start |

#### Edge Cases
| Test Name | Description |
|-----------|-------------|
| `Dropdown - Empty Options` | Handle no options gracefully |
| `Dropdown - Single Option` | Works with one option |
| `Dropdown - Many Options` | Scrollable with 100+ options |
| `Dropdown - Long Option Text` | Text truncation/wrapping |
| `Dropdown - Click Outside to Close` | Closes when clicking elsewhere |

---

### 1.4 Modal Component

#### Open/Close
```rust
test("Modal - Open on Trigger")
    .click("#open-modal-button")
    .expect_visible("#modal-dialog")
    .expect_visible("#modal-backdrop")

test("Modal - Close on X Button")
    .click("#open-modal-button")
    .click("#modal-close-button")
    .expect_not_visible("#modal-dialog")

test("Modal - Close on Backdrop Click")
    .click("#open-modal-button")
    .click("#modal-backdrop")
    .expect_not_visible("#modal-dialog")

test("Modal - Close on Escape")
    .click("#open-modal-button")
    .press_escape()
    .expect_not_visible("#modal-dialog")
```

#### Focus Trapping
| Test Name | Description |
|-----------|-------------|
| `Modal - Focus Trapped Inside` | Tab cycles within modal |
| `Modal - Initial Focus` | First focusable element focused |
| `Modal - Restore Focus on Close` | Focus returns to trigger |

#### Configuration
| Test Name | Description |
|-----------|-------------|
| `Modal - Backdrop Disabled` | Click backdrop doesn't close |
| `Modal - Escape Disabled` | Escape key doesn't close |
| `Modal - Custom Animation` | Open/close animation plays |

---

### 1.5 Tabs Component

#### Tab Switching
```rust
test("Tabs - Click to Switch")
    .click("#tab-settings")
    .expect_visible("#settings-panel")
    .expect_not_visible("#general-panel")

test("Tabs - Active Indicator")
    .click("#tab-account")
    .expect("#tab-account.active")
```

#### Keyboard Navigation
| Test Name | Description |
|-----------|-------------|
| `Tabs - Arrow Right` | Move to next tab |
| `Tabs - Arrow Left` | Move to previous tab |
| `Tabs - Home Key` | Jump to first tab |
| `Tabs - End Key` | Jump to last tab |

---

### 1.6 ScrollView Component

#### Scroll Interactions
```rust
test("ScrollView - Mouse Wheel")
    .hover("#scroll-container")
    .scroll("#scroll-container", 0.0, -100.0)
    // Verify scroll position changed

test("ScrollView - Scrollbar Drag")
    .click("#scrollbar-thumb")
    .drag_to("#scrollbar-track-bottom")
```

| Test Name | Description |
|-----------|-------------|
| `ScrollView - Scroll to Top` | Content scrolls to beginning |
| `ScrollView - Scroll to Bottom` | Content scrolls to end |
| `ScrollView - Momentum Scroll` | Smooth deceleration after swipe |
| `ScrollView - Bounds Check` | Can't scroll past content |
| `ScrollView - Nested Scroll` | Inner/outer scroll containers |

---

### 1.7 VirtualList Component

#### Rendering Efficiency
```rust
test("VirtualList - Renders Visible Only")
    .scroll("#virtual-list", 0.0, -500.0)
    .expect_visible("#item-20")
    .expect_not_visible("#item-0")
```

| Test Name | Description |
|-----------|-------------|
| `VirtualList - 10k Items` | Handle very large datasets |
| `VirtualList - Fast Scroll` | Rapid scrolling renders correctly |
| `VirtualList - Item Click` | Click item at any scroll position |
| `VirtualList - Dynamic Heights` | Variable height items |
| `VirtualList - Empty List` | Handle zero items |

---

## 2. Atom Component Tests

### 2.1 Status Indicators

| Test Name | Description |
|-----------|-------------|
| `StatusDot - Online State` | Green dot displayed |
| `StatusDot - Offline State` | Gray dot displayed |
| `StatusDot - Busy State` | Red dot displayed |
| `StatusDot - Away State` | Yellow dot displayed |
| `StatusDot - Pulsing Animation` | Pulse animation plays |
| `StreamingIndicator - Animation` | Dots animate in sequence |
| `StreamingIndicator - Stop` | Animation stops on completion |

### 2.2 Badges

| Test Name | Description |
|-----------|-------------|
| `ModeBadge - Normal Mode` | Correct color/text for Normal |
| `ModeBadge - Plan Mode` | Correct color/text for Plan |
| `ModeBadge - Act Mode` | Correct color/text for Act |
| `ModelBadge - Codex` | Shows Codex model info |
| `ModelBadge - GPT4` | Shows GPT-4 model info |
| `AgentStatusBadge - Active` | Green active indicator |
| `AgentStatusBadge - Blocked` | Red blocked indicator |
| `IssueStatusBadge - Open` | Open issue styling |
| `IssueStatusBadge - Closed` | Closed issue styling |
| `NetworkBadge - Mainnet` | Mainnet styling |
| `NetworkBadge - Testnet` | Testnet styling |

### 2.3 Interactive Atoms

| Test Name | Description |
|-----------|-------------|
| `CheckpointBadge - Click` | Checkpoint selection callback |
| `ThinkingToggle - Toggle On` | Shows thinking content |
| `ThinkingToggle - Toggle Off` | Hides thinking content |
| `PermissionButton - Allow` | Allow action triggered |
| `PermissionButton - Deny` | Deny action triggered |
| `FeedbackButton - Thumbs Up` | Positive feedback recorded |
| `FeedbackButton - Thumbs Down` | Negative feedback recorded |

---

## 3. Molecule Component Tests

### 3.1 Headers

| Test Name | Description |
|-----------|-------------|
| `MessageHeader - User Type` | Shows user avatar/name |
| `MessageHeader - Assistant Type` | Shows AI avatar/model |
| `MessageHeader - Timestamp` | Relative time displayed |
| `ToolHeader - Running State` | Shows spinner animation |
| `ToolHeader - Success State` | Shows checkmark |
| `ToolHeader - Failed State` | Shows error indicator |
| `DiffHeader - Add Change` | Green plus indicator |
| `DiffHeader - Delete Change` | Red minus indicator |
| `DiffHeader - Modify Change` | Blue modify indicator |

### 3.2 Selectors

```rust
test("ModeSelector - Change Mode")
    .click("#mode-selector")
    .click("text:Plan")
    .expect_text("#mode-selector", "Plan")

test("ModelSelector - Change Model")
    .click("#model-selector")
    .click("text:GPT-4")
    .expect_text("#model-selector", "GPT-4")
```

### 3.3 Cards & Rows

| Test Name | Description |
|-----------|-------------|
| `BalanceCard - Display Amount` | Shows correct BTC/USD |
| `PaymentRow - Pending State` | Pending payment styling |
| `PaymentRow - Completed State` | Completed payment styling |
| `RelayRow - Connected` | Green connection status |
| `RelayRow - Disconnected` | Red disconnection status |
| `ThinkingBlock - Expand` | Click expands content |
| `ThinkingBlock - Collapse` | Click collapses content |

---

## 4. Organism Component Tests

### 4.1 Message Components

```rust
test("ThreadEntry - Copy Action")
    .hover("#message-1")
    .click("#copy-button")
    .expect("#copied-toast")

test("ThreadEntry - Retry Action")
    .hover("#message-1")
    .click("#retry-button")
    .expect("#loading-indicator")
```

| Test Name | Description |
|-----------|-------------|
| `UserMessage - Render Markdown` | Markdown formatted correctly |
| `AssistantMessage - Streaming` | Tokens appear incrementally |
| `AssistantMessage - Complete` | Full message rendered |
| `ThreadControls - New Thread` | Starts new conversation |
| `ThreadControls - Clear` | Clears all messages |

### 4.2 Tool Display

| Test Name | Description |
|-----------|-------------|
| `TerminalToolCall - Output Display` | Terminal output rendered |
| `TerminalToolCall - Error Highlight` | Errors highlighted red |
| `SearchToolCall - Results List` | Search results shown |
| `SearchToolCall - No Results` | Empty state message |
| `DiffToolCall - Line Numbers` | Line numbers displayed |
| `DiffToolCall - Syntax Highlight` | Code syntax colored |

### 4.3 Complex Flows

#### Permission Dialog
```rust
test("PermissionDialog - Approve")
    .expect_visible("#permission-dialog")
    .click("#allow-button")
    .expect_not_visible("#permission-dialog")
    .expect("#action-executed")

test("PermissionDialog - Deny")
    .expect_visible("#permission-dialog")
    .click("#deny-button")
    .expect_not_visible("#permission-dialog")
    .expect_not("#action-executed")
```

#### Send Flow (Multi-step)
```rust
test("SendFlow - Complete Transaction")
    .click("#send-button")
    .expect_visible("#send-step-1")
    .type_text("bc1q...")
    .click("#next-button")
    .expect_visible("#send-step-2")
    .type_text("0.001")
    .click("#next-button")
    .expect_visible("#send-step-3")
    .click("#confirm-button")
    .expect("#transaction-sent")
```

### 4.4 Management Interfaces

| Test Name | Description |
|-----------|-------------|
| `RelayManager - Add Relay` | New relay added to list |
| `RelayManager - Remove Relay` | Relay removed from list |
| `RelayManager - Toggle Connection` | Connect/disconnect relay |
| `ScheduleConfig - Set Interval` | Interval saved correctly |
| `ScheduleConfig - Enable/Disable` | Schedule toggled |
| `ThresholdKeyManager - Add Peer` | Peer added to group |
| `ThresholdKeyManager - Sign Request` | Signing flow initiated |

---

## 5. Section Component Tests

### 5.1 ThreadView

```rust
test("ThreadView - Auto Scroll")
    .expect_visible("#latest-message")
    // New message arrives
    .wait(500)
    .expect_visible("#new-message")
```

| Test Name | Description |
|-----------|-------------|
| `ThreadView - Load History` | Previous messages loaded |
| `ThreadView - Infinite Scroll` | More messages load on scroll up |
| `ThreadView - Jump to Bottom` | Button scrolls to latest |

### 5.2 MessageEditor

```rust
test("MessageEditor - Submit Message")
    .click("#message-editor")
    .type_text("Hello, AI!")
    .click("#send-button")
    .expect("#message-sent")
    .expect_text("#message-editor", "")  // Cleared
```

| Test Name | Description |
|-----------|-------------|
| `MessageEditor - Shift+Enter Newline` | Adds newline, doesn't submit |
| `MessageEditor - Mode Selection` | Mode changes before send |
| `MessageEditor - Model Selection` | Model changes before send |
| `MessageEditor - Empty Submit` | Disabled when empty |
| `MessageEditor - Max Length` | Character limit enforced |

---

## 6. HUD Component Tests

### 6.1 Command Palette

```rust
test("CommandPalette - Open with Shortcut")
    .press_key_with(Key::Character("k"), Modifiers { ctrl: true, ..Default::default() })
    .expect_visible("#command-palette")

test("CommandPalette - Search and Execute")
    .press_key_with(Key::Character("k"), Modifiers { ctrl: true, ..Default::default() })
    .type_text("new file")
    .press_enter()
    .expect("#file-created")
```

| Test Name | Description |
|-----------|-------------|
| `CommandPalette - Filter Results` | Typing filters commands |
| `CommandPalette - Arrow Navigation` | Up/down selects commands |
| `CommandPalette - Escape Closes` | Escape dismisses palette |
| `CommandPalette - No Results` | Empty state shown |

### 6.2 Context Menu

```rust
test("ContextMenu - Right Click")
    .right_click("#file-item")
    .expect_visible("#context-menu")

test("ContextMenu - Select Action")
    .right_click("#file-item")
    .click("text:Delete")
    .expect("#delete-confirmation")
```

| Test Name | Description |
|-----------|-------------|
| `ContextMenu - Nested Submenu` | Hover opens submenu |
| `ContextMenu - Keyboard Select` | Arrow keys + Enter |
| `ContextMenu - Click Outside` | Dismisses menu |

### 6.3 Tooltip

| Test Name | Description |
|-----------|-------------|
| `Tooltip - Show on Hover` | Appears after delay |
| `Tooltip - Hide on Leave` | Disappears when mouse leaves |
| `Tooltip - Position Top` | Displays above target |
| `Tooltip - Position Bottom` | Displays below target |
| `Tooltip - Position Flip` | Flips if near edge |

### 6.4 Notifications

```rust
test("Notifications - Auto Dismiss")
    .click("#trigger-notification")
    .expect_visible("#notification-toast")
    .wait(5000)
    .expect_not_visible("#notification-toast")
```

| Test Name | Description |
|-----------|-------------|
| `Notifications - Stack Multiple` | Multiple toasts stack |
| `Notifications - Manual Dismiss` | Click X closes toast |
| `Notifications - Clear All` | Clear all button works |
| `Notifications - Position Options` | Top/bottom left/right |

### 6.5 StatusBar

| Test Name | Description |
|-----------|-------------|
| `StatusBar - Item Alignment` | Left/center/right items |
| `StatusBar - Dynamic Updates` | Items update in real-time |
| `StatusBar - Click Item` | Item click callback |

---

## 7. User Flow Tests

### 7.1 Authentication Flow

```rust
test("Login Flow - Success")
    .click("#login-button")
    .expect_visible("#login-modal")
    .click("#email-input")
    .type_text("user@example.com")
    .click("#password-input")
    .type_text("password123")
    .click("#submit-login")
    .wait_for("#dashboard", 5000)
    .expect("#user-avatar")
```

```rust
test("Login Flow - Invalid Credentials")
    .click("#login-button")
    .click("#email-input")
    .type_text("user@example.com")
    .click("#password-input")
    .type_text("wrongpassword")
    .click("#submit-login")
    .expect_visible("#error-message")
    .expect_text("#error-message", "Invalid credentials")
```

### 7.2 Conversation Flow

```rust
test("Complete Conversation")
    .click("#new-chat")
    .click("#message-editor")
    .type_text("What is 2 + 2?")
    .click("#send-button")
    .wait_for("#assistant-response", 10000)
    .expect_text("#assistant-response", "4")
```

```rust
test("Conversation with Tool Use")
    .click("#message-editor")
    .type_text("Search for files named test.rs")
    .click("#send-button")
    .wait_for("#tool-call-card", 5000)
    .expect_visible("#search-results")
```

### 7.3 Wallet Flow

```rust
test("Receive Payment Flow")
    .click("#receive-button")
    .expect_visible("#receive-modal")
    .click("#amount-input")
    .type_text("1000")
    .click("#generate-invoice")
    .wait_for("#qr-code", 3000)
    .expect_visible("#invoice-string")
```

```rust
test("Send Payment Flow")
    .click("#send-button")
    .expect_visible("#send-modal")
    .click("#address-input")
    .type_text("bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh")
    .click("#amount-input")
    .type_text("50000")
    .click("#review-button")
    .expect_visible("#confirmation-screen")
    .click("#confirm-send")
    .wait_for("#success-screen", 5000)
```

### 7.4 Settings Flow

```rust
test("Change Theme")
    .click("#settings-button")
    .click("#appearance-tab")
    .click("#theme-dropdown")
    .click("text:Dark")
    .expect("#app.theme-dark")
```

### 7.5 Search & Filter Flow

```rust
test("Search with Filters")
    .click("#search-input")
    .type_text("error")
    .click("#filter-dropdown")
    .click("text:Last 24 hours")
    .click("#search-button")
    .wait_for("#search-results", 3000)
    .expect_visible("#result-count")
```

---

## 8. Keyboard Navigation Tests

### 8.1 Tab Order

```rust
test("Tab Through Form")
    .press_tab()  // Focus first input
    .expect_focused("#name-input")
    .press_tab()
    .expect_focused("#email-input")
    .press_tab()
    .expect_focused("#submit-button")
```

### 8.2 Shortcut Keys

| Test Name | Shortcut | Expected Action |
|-----------|----------|-----------------|
| `Shortcut - New Chat` | Ctrl+N | Opens new chat |
| `Shortcut - Search` | Ctrl+F | Opens search |
| `Shortcut - Command Palette` | Ctrl+K | Opens palette |
| `Shortcut - Close Modal` | Escape | Closes modal |
| `Shortcut - Submit` | Ctrl+Enter | Submits form |
| `Shortcut - Undo` | Ctrl+Z | Undoes action |
| `Shortcut - Redo` | Ctrl+Shift+Z | Redoes action |
| `Shortcut - Copy` | Ctrl+C | Copies selection |
| `Shortcut - Paste` | Ctrl+V | Pastes content |

### 8.3 Focus Management

| Test Name | Description |
|-----------|-------------|
| `Focus - Skip Link` | Skip to main content |
| `Focus - Modal Trap` | Tab stays within modal |
| `Focus - Return on Close` | Focus returns to trigger |
| `Focus - Menu Navigation` | Arrow keys in menus |

---

## 9. Animation & Timing Tests

### 9.1 Transition Tests

| Test Name | Description |
|-----------|-------------|
| `Animation - Button Hover` | Color transition smooth |
| `Animation - Modal Open` | Fade/scale animation |
| `Animation - Modal Close` | Reverse animation |
| `Animation - Dropdown Open` | Slide down animation |
| `Animation - Tab Switch` | Indicator slides |
| `Animation - Notification Enter` | Slide in from edge |
| `Animation - Notification Exit` | Fade out |

### 9.2 Loading States

| Test Name | Description |
|-----------|-------------|
| `Loading - Spinner Visible` | Spinner shows during load |
| `Loading - Skeleton Screen` | Placeholder content shown |
| `Loading - Progress Bar` | Progress updates correctly |
| `Loading - Infinite Spinner` | Doesn't freeze on long load |

### 9.3 Streaming

```rust
test("Streaming Response")
    .click("#send-button")
    .wait(500)
    .expect_visible("#streaming-indicator")
    .wait_for("#response-complete", 10000)
    .expect_not_visible("#streaming-indicator")
```

---

## 10. Edge Cases & Error States

### 10.1 Error Handling

| Test Name | Description |
|-----------|-------------|
| `Error - Network Failure` | Shows offline message |
| `Error - API Error` | Shows error toast |
| `Error - Validation Error` | Field error message |
| `Error - 404 Page` | Not found page shown |
| `Error - Permission Denied` | Access denied message |
| `Error - Rate Limited` | Rate limit message |

### 10.2 Empty States

| Test Name | Description |
|-----------|-------------|
| `Empty - No Messages` | Empty chat placeholder |
| `Empty - No Search Results` | No results message |
| `Empty - No Notifications` | Empty notifications |
| `Empty - No Transactions` | No history message |

### 10.3 Boundary Conditions

| Test Name | Description |
|-----------|-------------|
| `Boundary - Max Input Length` | Input truncates/rejects |
| `Boundary - Min Window Size` | UI adapts to small window |
| `Boundary - Very Long Text` | Text wraps/truncates |
| `Boundary - Many Items` | Performance with 1000+ items |
| `Boundary - Deep Nesting` | Deeply nested components |
| `Boundary - Rapid Actions` | Debounce/throttle works |

### 10.4 Interrupted Actions

| Test Name | Description |
|-----------|-------------|
| `Interrupt - Close During Load` | Cleanup on cancel |
| `Interrupt - Navigate During Submit` | Submit completes or cancels |
| `Interrupt - Resize During Animation` | Animation adapts |

---

## 11. Performance Tests

### 11.1 Render Performance

| Test Name | Metric | Threshold |
|-----------|--------|-----------|
| `Perf - Initial Render` | Time to first paint | < 100ms |
| `Perf - Large List Scroll` | Frame rate during scroll | > 30fps |
| `Perf - Rapid Input` | Input latency | < 16ms |
| `Perf - Animation Smoothness` | Frame drops | < 5% |

### 11.2 Memory

| Test Name | Description |
|-----------|-------------|
| `Memory - Virtual List` | Memory stable with scroll |
| `Memory - Open/Close Modal` | No memory leak on repeat |
| `Memory - Long Conversation` | Memory bounded |

---

## 12. Accessibility Tests

### 12.1 Screen Reader

| Test Name | Description |
|-----------|-------------|
| `A11y - Button Label` | Button has accessible name |
| `A11y - Input Label` | Input associated with label |
| `A11y - Error Announcement` | Errors announced |
| `A11y - Live Region` | Dynamic content announced |
| `A11y - Landmark Regions` | Main/nav/footer landmarks |

### 12.2 Visual

| Test Name | Description |
|-----------|-------------|
| `A11y - Color Contrast` | 4.5:1 ratio for text |
| `A11y - Focus Indicator` | Visible focus ring |
| `A11y - Text Resize` | UI works at 200% zoom |
| `A11y - Reduced Motion` | Respects motion preference |

### 12.3 Motor

| Test Name | Description |
|-----------|-------------|
| `A11y - Click Target Size` | Min 44x44px targets |
| `A11y - Keyboard Only` | All actions via keyboard |
| `A11y - No Hover Required` | Hover not required for action |

---

## Test Priority Matrix

### P0 - Critical (Must Have)
- Login/Authentication flow
- Message send/receive
- Button click interactions
- Text input functionality
- Modal open/close
- Error display

### P1 - High (Should Have)
- Keyboard navigation
- Dropdown selection
- Tab switching
- Scroll behavior
- Toast notifications
- Form validation

### P2 - Medium (Nice to Have)
- Animation smoothness
- Performance benchmarks
- Edge cases
- Accessibility compliance
- Context menus
- Command palette

### P3 - Low (Future)
- All badge variants
- All status indicators
- Complex multi-step flows
- Stress testing

---

## Implementation Checklist

- [ ] Foundation Components (7 components)
  - [ ] Button (10 tests)
  - [ ] TextInput (15 tests)
  - [ ] Dropdown (12 tests)
  - [ ] Modal (10 tests)
  - [ ] Tabs (8 tests)
  - [ ] ScrollView (6 tests)
  - [ ] VirtualList (5 tests)

- [ ] User Flows (5 flows)
  - [ ] Authentication
  - [ ] Conversation
  - [ ] Wallet operations
  - [ ] Settings
  - [ ] Search

- [ ] Keyboard Navigation (15 tests)

- [ ] Error States (10 tests)

- [ ] Accessibility (12 tests)

---

## Running Tests

```rust
// Run a single test
let test = test("Login Flow")
    .click("#login-button")
    // ...
    .build();

let harness = TestHarness::new(app)
    .with_runner(test)
    .show_overlay(true);

// Run a test suite
let suite = TestSuite::new("Authentication")
    .add(login_success_test)
    .add(login_failure_test)
    .add(logout_test);

suite.run_all();
```

---

## Adding New Tests

1. Identify the component/flow to test
2. Determine test category (unit/integration/e2e)
3. Write test using DSL
4. Add to appropriate section in this document
5. Update implementation checklist
6. Run and verify test passes

---

*Last updated: 2024-12*
