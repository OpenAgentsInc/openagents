# NIP-90 Event Feed Viewer (macOS Dev-Only)

**Phase:** 1 - MVP
**Component:** macOS App (OpenAgents)
**Priority:** P1 (High - Critical for marketplace debugging and validation)
**Estimated Effort:** 1-2 weeks
**Status:** ✅ **READY TO IMPLEMENT** (No blocking dependencies)

## Summary

Implement a development-only NIP-90 event feed viewer on the macOS app homepage that connects to Nostr relays and displays real-time marketplace events (job requests, results, and feedback). This feed serves as a critical debugging tool for Phase 1 MVP, enabling developers to verify relay connectivity, monitor marketplace activity, debug event parsing, and validate the marketplace protocol before building the full iOS marketplace viewer (issue #006).

## Motivation

Before building the full iOS marketplace viewer (issue #006) and macOS worker (issue #007), we need to validate that:

1. **Nostr SDK integration works correctly** - The nostr-sdk-ios fork can connect to relays and subscribe to events
2. **NIP-90 events are flowing** - Real marketplace events are being published and can be received
3. **Event filtering and parsing** - Our filter logic and event parsing correctly handles all DVM kinds
4. **Job schema registry alignment** - The kinds defined in issue #004 match real marketplace activity

A dev-only event feed viewer provides immediate visibility into the marketplace protocol layer without requiring full buyer/seller UX. This enables rapid iteration during development and serves as a living diagnostic tool for the entire Phase 1 implementation.

Without this tool, debugging would require:
- Manual relay querying via CLI tools
- Building the full marketplace viewer before validating basics
- Blind protocol development without visibility into real events
- Lengthy debug cycles when things break

## Acceptance Criteria

### Relay Connection Management (Pattern 6: Sync State Visibility)

- [ ] Connect to configurable set of Nostr relays
  - [ ] **Smart default relays** (hardcoded for MVP):
    - `wss://relay.damus.io` (high uptime, popular)
    - `wss://relay.nostr.band` (NIP-90 support, testing-friendly)
    - `wss://nos.lol` (developer-friendly, low latency)
  - [ ] Additional relays configurable in DEBUG builds only
- [ ] **Display connection status with clear visual feedback**
  - [ ] States with icons and colors:
    - 🔵 Connecting (blue, animated pulse)
    - 🟢 Connected (green, solid)
    - ⚪ Disconnected (gray)
    - 🔴 Error (red)
  - [ ] **Status text in plain language**:
    - ✅ "Connected to relay.damus.io"
    - 🔄 "Connecting to nos.lol..."
    - ⚠️ "relay.nostr.band failed: Connection timeout"
    - ℹ️ "Disconnected (stopped by user)"
  - [ ] **Last connected timestamp**: "Last connected 2 minutes ago"
  - [ ] **Overall status indicator**: "2 of 3 relays connected"
- [ ] **Automatic reconnection with visible feedback**
  - [ ] Exponential backoff (1s, 2s, 4s, 8s, 16s max)
  - [ ] **Show reconnection attempts**: "Retrying in 4s... (attempt 3/5)"
  - [ ] Cancel reconnection when feed is stopped
  - [ ] **User control**: "Cancel reconnection" button during retry
- [ ] Relay management UI (DEBUG mode only, Pattern 5: Progressive Complexity)
  - [ ] Add relay by URL
    - **URL validation** with clear error messages
    - **Test connection** before adding
  - [ ] Remove relay from list with confirmation
  - [ ] Persist relay list to UserDefaults
  - [ ] **Relay stats** (advanced): Events received, avg latency, uptime %
- [ ] Clean disconnection when feed is stopped
  - [ ] **Status message**: "Disconnected" (not silent disconnection)

### NIP-90 Event Subscription

- [ ] Subscribe to job request events (kinds 5000-5999)
  - [ ] Official DVM kinds (5000-5970): text extraction, summarization, translation, etc.
  - [ ] OpenAgents custom kinds (6500-6519): code generation, review, Q&A, etc.
- [ ] Subscribe to job result events (kinds 6000-6999)
  - [ ] Official DVM results (6000-6970)
  - [ ] OpenAgents custom results (7500-7519)
- [ ] Subscribe to job feedback events (kind 7000)
- [ ] Filter events by kind category
  - [ ] Dropdown options:
    - "All NIP-90" (kinds 5000-5999, 6000-6999, 7000)
    - "Job Requests" (5000-5999)
    - "Job Results" (6000-6999)
    - "Job Feedback" (7000)
    - "OpenAgents Custom" (6500-6599, 7500-7599)
  - [ ] Re-subscribe when filter changes
- [ ] Filter events by time range
  - [ ] Options: Last Hour, Last 24 Hours, Last 7 Days, All Time
  - [ ] Default: Last 24 Hours
  - [ ] Re-subscribe when time range changes
- [ ] Limit initial fetch
  - [ ] Default: 100 most recent events
  - [ ] Configurable in DEBUG mode
- [ ] Handle subscription lifecycle
  - [ ] Subscribe when feed starts
  - [ ] Unsubscribe when feed stops or collapses
  - [ ] Clean up resources on view disappear

### Event Display (Pattern 3: Core Interactions - Clear Visual Feedback)

- [ ] Real-time event feed (newest first chronological order)
- [ ] Event card UI for each event showing:
  - [ ] Event type badge (color-coded with consistent semantics):
    - Blue badge: Job Request (kinds 5000-5999)
    - Green badge: Job Result (kinds 6000-6999)
    - Orange badge: Job Feedback (kind 7000)
    - Purple accent: OpenAgents custom kinds
    - **Badge should have icon + text** (not just color for accessibility)
  - [ ] Event kind with human-readable name
    - Lookup from JobKind enum (issue #004)
    - Fallback to "Kind XXXX" if not in registry
    - Indicate "(Custom)" for OpenAgents kinds
  - [ ] Event ID (truncated to 8 chars with "..." + copy button)
    - **Copy button feedback**: Show checkmark ✓ briefly after copy
    - **Tooltip**: "Copy event ID" on hover
  - [ ] Author pubkey (truncated to 8 chars npub format + copy button)
    - **Copy button feedback**: Show checkmark ✓ briefly after copy
    - **Tooltip**: "Copy author public key" on hover
  - [ ] Timestamp
    - Relative time: "Just now", "2m ago", "3h ago", "2d ago"
    - Absolute time on hover tooltip: "Jan 15, 2025 at 3:45 PM"
    - **Auto-update** relative time every minute for visible events
  - [ ] Content preview
    - First 200 characters for plain text
    - "[JSON content]" indicator if starts with `{` or `[`
    - "[Encrypted content (NIP-04)]" indicator if contains `?iv=`
    - Line limit: 2 lines when collapsed
  - [ ] Tag summary
    - Show first 5 tag names: "Tags: i, param, bid, output, relays ..."
    - Full tag count if more than 5: "Tags (12): i, param, bid..."
  - [ ] Expand/collapse button
    - **Clear labels**: "Show more" when collapsed, "Show less" when expanded
    - **Smooth animation**: 200ms ease-in-out when expanding/collapsing
    - **Icon**: Chevron down/up to indicate state
- [ ] Event detail view (when expanded)
  - [ ] Full event fields:
    - Event ID (full hex + copy button)
    - Pubkey (full hex + npub bech32 + copy button)
    - Created_at (Unix timestamp + human-readable date)
    - Kind (raw integer + registry name)
    - Signature (full hex + verification status)
  - [ ] Signature verification
    - ✓ Valid (green checkmark) or ✗ Invalid (red X)
    - Use NostrSDK.isVerified() method
  - [ ] Content display
    - Syntax-highlighted JSON if content is JSON
    - Plain text otherwise
    - Monospace font
    - Text selection enabled
    - Copy button
  - [ ] Tags table
    - Format: [tag_name] value
    - Monospace font
    - Show all tags (not just first 5)
    - Display tag count header: "Tags (12):"
  - [ ] Action buttons (Pattern 3: Core Interactions)
    - **"Copy Event JSON"**:
      - Copies entire event as JSON to clipboard
      - **Feedback**: Button changes to "Copied ✓" for 2 seconds
      - **Error handling**: Show "Copy failed" if clipboard unavailable
    - **"View in Explorer"**:
      - Opens njump.me link in default browser (if supported)
      - **Loading state**: Show spinner while opening browser
      - **Disabled state**: Grayed out if explorer doesn't support this kind
- [ ] Event list states (Pattern 4: Performance - Perceived Performance)
  - [ ] **Loading state**:
    - Show **skeleton screens** (3-5 placeholder event cards with shimmer animation)
    - Text: "Connecting to relays..." (first 2s) → "Listening for events..." (after 2s)
    - **Progress indicator**: Show which relays are connected (e.g., "2 of 3 relays connected")
    - Keep header and filter bar visible during load (don't block entire UI)
  - [ ] **Empty state** (Pattern 2: Content Discovery - Empty States):
    - Icon: Magnifying glass or radio waves icon
    - Title: "No events yet"
    - Message: "Waiting for marketplace activity. This may take a minute..."
    - **Helpful context**: Show relay connection status below message
    - **Action button** (optional): "Refresh" button to retry subscription
  - [ ] **Error state**:
    - Icon: Warning triangle
    - Title: "Connection failed"
    - **Clear error message** in plain language (not technical jargon)
      - ❌ "WebSocket error 1006"
      - ✅ "Can't connect to relays. Check your internet connection."
    - **Action button**: Prominent "Retry" button
    - **Secondary action**: "Check relay status" to see which relays failed
  - [ ] **Normal state**: Scrollable list of events with smooth scrolling
- [ ] Performance optimizations (Pattern 4: Performance)
  - [ ] **Lazy loading** (LazyVStack)
    - Only render visible event cards
    - Pre-render 5 cards above/below viewport for smooth scrolling
  - [ ] **Virtual scrolling** for large lists
    - Recycle view components for off-screen events
    - Maintain scroll position during updates
  - [ ] **Skeleton screens** during initial load
    - Show placeholder cards with shimmer animation
    - Replace with real content as events arrive
  - [ ] **Optimistic UI updates**
    - Insert new events immediately (don't wait for re-sort)
    - Smooth animation when new events appear
  - [ ] Max height: 600px with scroll
  - [ ] **Debounce rapid updates** (max 10 events/second)
    - Batch multiple events into single UI update
    - Prevents UI thrashing during high-volume periods
  - [ ] **Background processing**
    - Parse events on background thread
    - Update UI on main thread only
  - [ ] **Perceived performance**
    - Show "Listening for events..." immediately (don't wait for connection)
    - Update status as relays connect progressively
    - Display partial results (don't wait for all relays)

### Dev-Only UI Integration

- [ ] Add "Nostr Event Feed (Dev)" card to macOS home view
  - [ ] File: `SimplifiedMacOSView.swift`
  - [ ] Only visible in DEBUG builds (`#if DEBUG`)
  - [ ] Position after existing dev cards (tinyvexDevCard, nostrDevCard)
- [ ] Card styling matches existing dev cards
  - [ ] Same padding, border radius, background color
  - [ ] Header with icon (antenna.radiowaves.left.and.right)
  - [ ] Collapsible with expand/collapse button
- [ ] Card state persistence
  - [ ] Default to collapsed on app launch
  - [ ] Remember expanded/collapsed state across app restarts
  - [ ] Store in UserDefaults
- [ ] Resource management
  - [ ] Cancel subscriptions when card is collapsed
  - [ ] Clear event cache when feed is stopped (optional)
  - [ ] Disconnect from relays when app backgrounds (macOS)

### Performance & Resource Limits

- [ ] In-memory event cache
  - [ ] Max events: 1000 (configurable in DEBUG)
  - [ ] FIFO eviction (oldest events dropped first)
  - [ ] Deduplicate by event ID (prevent duplicate display)
- [ ] UI performance
  - [ ] Process events on background thread
  - [ ] Update UI on main thread only
  - [ ] Batch UI updates (max 10 events/second)
- [ ] Memory management
  - [ ] Release event cache when feed stops
  - [ ] Cancel all subscriptions on deinit
  - [ ] No memory leaks during 10-minute session

## Technical Design

### File Structure

```
ios/OpenAgents/
├── Views/
│   ├── Nostr/                                # NEW: Nostr-specific views
│   │   ├── NostrEventFeedView.swift          # Main feed view (LazyVStack)
│   │   ├── NostrEventCard.swift              # Individual event card UI
│   │   ├── NostrEventDetailView.swift        # Expanded event detail
│   │   ├── NostrRelayStatusView.swift        # Relay connection indicators
│   │   └── NostrEventFilterView.swift        # Filter dropdown controls
│   └── SimplifiedMacOSView.swift             # UPDATE: Add nostrEventFeedCard

ios/OpenAgentsCore/Sources/OpenAgentsCore/
├── Nostr/                                     # NEW: Nostr integration layer
│   ├── NostrEventFeedManager.swift           # Event subscription & filtering
│   ├── NostrRelayManager.swift               # Relay connection management
│   └── DVMEventParser.swift                  # NIP-90 specific event parsing
├── JobSchemas/
│   └── JobKind.swift                         # DEPENDS: From issue #004

ios/OpenAgentsCore/Sources/OpenAgentsNostr/
└── NostrShim.swift                           # UPDATE: Add RelayPool helpers

ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/
└── Nostr/                                     # NEW: Test suite
    ├── NostrEventFeedManagerTests.swift
    ├── NostrRelayManagerTests.swift
    └── DVMEventParserTests.swift
```

### Core Swift Types

```swift
// NostrRelayManager.swift

import Foundation
import NostrSDK
import Combine

/// Manages Nostr relay connections and lifecycle
@MainActor
public class NostrRelayManager: ObservableObject {
    @Published public private(set) var relays: [RelayInfo] = []
    @Published public private(set) var connectionStatus: ConnectionStatus = .disconnected

    private var relayPool: RelayPool?
    private var cancellables = Set<AnyCancellable>()

    public struct RelayInfo: Identifiable, Equatable {
        public let id: String  // URL string
        public let url: URL
        public var status: RelayStatus
        public var lastConnected: Date?
        public var errorMessage: String?

        public enum RelayStatus: Equatable {
            case connecting
            case connected
            case disconnected
            case error(String)
        }
    }

    public enum ConnectionStatus: Equatable {
        case disconnected
        case connecting
        case connected(count: Int)  // Number of connected relays
        case error(String)
    }

    // Default relays for MVP
    public static let defaultRelays: [URL] = [
        URL(string: "wss://relay.damus.io")!,
        URL(string: "wss://relay.nostr.band")!,
        URL(string: "wss://nos.lol")!
    ]

    public init(relayURLs: [URL] = Self.defaultRelays) {
        self.relays = relayURLs.map { RelayInfo(id: $0.absoluteString, url: $0, status: .disconnected) }
    }

    /// Connect to all configured relays
    public func connect() async throws { /* Implementation */ }

    /// Disconnect from all relays
    public func disconnect() { /* Implementation */ }

    /// Subscribe to events with filter
    public func subscribe(
        kinds: [Int],
        limit: Int = 100,
        since: Date? = nil,
        until: Date? = nil
    ) -> AnyPublisher<NostrEvent, Never> { /* Implementation */ }

    /// Add a new relay
    public func addRelay(url: URL) throws { /* Implementation */ }

    /// Remove a relay
    public func removeRelay(url: URL) { /* Implementation */ }
}
```

```swift
// NostrEventFeedManager.swift

import Foundation
import Combine
import NostrSDK
import OpenAgentsCore

/// Manages NIP-90 event feed subscription and filtering
@MainActor
public class NostrEventFeedManager: ObservableObject {
    @Published public private(set) var events: [DVMEventItem] = []
    @Published public private(set) var isLoading = false
    @Published public private(set) var error: FeedError?
    @Published public var selectedKindFilter: KindFilter = .all
    @Published public var timeFilter: TimeFilter = .last24Hours

    private let relayManager: NostrRelayManager
    private let maxEvents = 1000
    private var subscription: AnyCancellable?

    public struct DVMEventItem: Identifiable, Equatable {
        public let id: String  // Event ID
        public let event: NostrEvent
        public let type: EventType
        public let timestamp: Date
        public let authorNpub: String
        public let kindInfo: KindInfo?

        public enum EventType {
            case jobRequest(kind: Int)
            case jobResult(kind: Int)
            case jobFeedback
        }

        public struct KindInfo {
            let name: String
            let displayName: String
            let isCustom: Bool
        }
    }

    public enum KindFilter: Equatable, CaseIterable {
        case all            // 5000-5999, 6000-6999, 7000
        case requests       // 5000-5999
        case results        // 6000-6999
        case feedback       // 7000
        case customOnly     // 6500-6599, 7500-7599

        var kinds: [Int] { /* Implementation */ }
    }

    public enum TimeFilter: Equatable, CaseIterable {
        case lastHour
        case last24Hours
        case last7Days
        case allTime

        var since: Date? { /* Implementation */ }
    }

    public init(relayManager: NostrRelayManager) { /* Implementation */ }

    /// Start the event feed (connect + subscribe)
    public func startFeed() async { /* Implementation */ }

    /// Stop the feed (unsubscribe + disconnect)
    public func stopFeed() { /* Implementation */ }

    /// Refresh the feed (clear + restart)
    public func refreshFeed() async { /* Implementation */ }

    /// Copy event as JSON to clipboard
    public func copyEventJSON(_ event: NostrEvent) { /* Implementation */ }

    /// Verify event signature
    public func verifySignature(_ event: NostrEvent) -> Bool { /* Implementation */ }
}
```

### SwiftUI View Structure

```swift
// NostrEventFeedView.swift

#if os(macOS) && DEBUG

struct NostrEventFeedView: View {
    @StateObject private var feedManager: NostrEventFeedManager
    @State private var isExpanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Header: icon + title + expand/collapse button
            header

            if isExpanded {
                VStack(spacing: 16) {
                    // Filter controls (kind filter + time filter + refresh button)
                    filterBar

                    // Event list or loading/error/empty states
                    content
                }
            }
        }
        .padding(20)
        .background(RoundedRectangle(cornerRadius: 12).fill(/* theme color */))
        .task { await feedManager.startFeed() }
        .onDisappear { feedManager.stopFeed() }
    }

    private var filterBar: some View { /* Filter pickers */ }
    private var content: some View { /* Event list or states */ }
}

#endif
```

```swift
// NostrEventCard.swift

#if os(macOS) && DEBUG

struct NostrEventCard: View {
    let item: NostrEventFeedManager.DVMEventItem
    let feedManager: NostrEventFeedManager
    @State private var isExpanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Compact view: badge + kind + time + ID + author + content preview
            compactView

            // Expand/collapse button
            Button(isExpanded ? "Show less" : "Show more") {
                isExpanded.toggle()
            }

            // Expanded detail (if isExpanded)
            if isExpanded {
                eventDetail
            }
        }
        .padding(12)
        .background(RoundedRectangle(cornerRadius: 8).fill(/* theme color */))
    }

    private var compactView: some View { /* Type badge, kind, ID, author, preview */ }
    private var eventDetail: some View { /* Full event fields, tags, copy buttons */ }
}

#endif
```

### Integration with macOS Home View

```swift
// SimplifiedMacOSView.swift (UPDATE)

#if DEBUG
@ViewBuilder private var nostrEventFeedCard: some View {
    NostrEventFeedView(relayManager: relayManager)
}

// Add to LazyVGrid
LazyVGrid(columns: columns, alignment: .leading, spacing: 24) {
    bridgeStatusCard
    workingDirectoryCard
    configureAgentsCard
    #if DEBUG
    tinyvexDevCard
    nostrDevCard
    nostrEventFeedCard  // NEW
    #endif
}
#endif
```

## Dependencies

### OpenAgents Dependencies

- **NostrSDK** - ✅ **ALREADY AVAILABLE**
  - Source: `/Users/christopherdavid/code/nostr-sdk-ios` (OpenAgents fork)
  - Already integrated via local package dependency
  - Provides: `RelayPool`, `NostrEvent`, `Filter`, `Relay` types
  - Signature verification: `event.isVerified()` available
- **Keypair Generation** - ✅ **ALREADY AVAILABLE**
  - Implemented on macOS Nostr Dev card
  - Ephemeral keys sufficient for relay connections
  - No blocking work needed
- **Issue #004**: Job Schema Registry - **SOFT DEPENDENCY**
  - Provides `JobKind` enum for kind metadata lookup
  - Feed can show raw kind numbers without registry (degraded UX)
  - Can implement feed without this, then enhance later

### System Frameworks

- **Foundation**: `Date`, `DateFormatter`, `JSONEncoder`, `UserDefaults`
- **SwiftUI**: `View`, `LazyVStack`, `Picker`, `ScrollView`
- **Combine**: `AnyPublisher`, `@Published`, `sink`, `AnyCancellable`

## Testing Requirements

### Unit Tests

#### `NostrRelayManagerTests.swift`
- [ ] **Test: Connect to relays successfully**
  - Given: List of valid relay URLs
  - When: `connect()` is called
  - Then: All relays transition to `.connected` status
  - Then: `connectionStatus` is `.connected(count: 3)`
- [ ] **Test: Handle connection failures gracefully**
  - Given: Invalid relay URL (e.g., `wss://nonexistent.relay`)
  - When: `connect()` is called
  - Then: Relay status is `.error(message)`
  - Then: Other relays continue connecting
- [ ] **Test: Reconnect on disconnect**
  - Given: Connected relay
  - When: Relay disconnects
  - Then: Automatic reconnection attempt with exponential backoff
- [ ] **Test: Add/remove relays dynamically**
  - When: `addRelay(url:)` is called
  - Then: New relay appears in `relays` list
  - When: `removeRelay(url:)` is called
  - Then: Relay removed from list and disconnected
- [ ] **Test: Subscribe to events with filters**
  - Given: Connected relay pool
  - When: `subscribe(kinds: [5000, 6000], limit: 10)` is called
  - Then: Returns publisher that emits matching events
  - Then: Subscription is active until cancelled

#### `NostrEventFeedManagerTests.swift`
- [ ] **Test: Filter events by kind category**
  - Given: Mix of request, result, feedback events
  - When: `selectedKindFilter = .requests`
  - Then: Only events with kinds 5000-5999 are displayed
- [ ] **Test: Filter events by time range**
  - Given: Events from last hour, yesterday, last week
  - When: `timeFilter = .lastHour`
  - Then: Only events from last hour are displayed
- [ ] **Test: Deduplicate events by ID**
  - Given: Same event received twice
  - When: Events are added to feed
  - Then: Only one copy appears in `events` list
- [ ] **Test: Trim event cache to max size**
  - Given: 1100 events received
  - When: Cache exceeds `maxEvents` (1000)
  - Then: Oldest 100 events are dropped
  - Then: Cache size is 1000
- [ ] **Test: Parse DVM event types correctly**
  - Given: Event with kind 5001 (job request)
  - Then: `DVMEventItem.type` is `.jobRequest(kind: 5001)`
  - Given: Event with kind 6001 (job result)
  - Then: `DVMEventItem.type` is `.jobResult(kind: 6001)`
  - Given: Event with kind 7000 (feedback)
  - Then: `DVMEventItem.type` is `.jobFeedback`
- [ ] **Test: Lookup kind info from registry**
  - Given: Event with kind 6500 (code generation)
  - When: Kind info is looked up
  - Then: Returns `KindInfo(name: "code-generation", displayName: "Code Generation", isCustom: true)`

#### `DVMEventParserTests.swift`
- [ ] **Test: Parse job request tags**
  - Extract `i` tags (inputs)
  - Extract `param` tags (parameters)
  - Extract `bid` tag (payment bid)
  - Extract `output` tag (MIME type)
- [ ] **Test: Parse job result tags**
  - Extract `request` tag (original job ID)
  - Extract `amount` tag (price)
  - Extract `bolt11` tag (Lightning invoice)
- [ ] **Test: Parse job feedback tags**
  - Extract `status` tag (payment-required, processing, success, error)
  - Extract `amount` tag (price)
- [ ] **Test: Detect encrypted content**
  - Given: Content with `?iv=` (NIP-04 encryption)
  - Then: Parser returns `isEncrypted: true`
- [ ] **Test: Parse OpenAgents custom kinds**
  - Given: Event with kind 6508 (classification)
  - Then: Correctly identified as custom kind

### Integration Tests

- [ ] **Test: Connect to real Nostr relays**
  - Use test relays (not production): `wss://relay.damus.io`, `wss://nos.lol`
  - Assert successful connection
  - Assert relay status transitions to `.connected`
- [ ] **Test: Subscribe to NIP-90 events**
  - Subscribe to kinds 5000-7000
  - Wait for at least 1 event (timeout: 30 seconds)
  - Assert event is valid NostrEvent
- [ ] **Test: Filter events correctly**
  - Subscribe to all NIP-90 kinds
  - Switch filter to "Job Requests" only
  - Assert feed updates to show only request events
- [ ] **Test: Verify event signatures**
  - Receive event from relay
  - Call `feedManager.verifySignature(event)`
  - Assert signature is valid (assuming relay sent valid events)
- [ ] **Test: UI updates on event arrival**
  - Start feed
  - Wait for events to appear
  - Assert `feedManager.events.count > 0`
  - Assert UI updates (SwiftUI view shows event cards)

### Manual Testing Checklist

- [ ] Launch macOS app in DEBUG mode (Run scheme in Xcode)
- [ ] Verify "Nostr Event Feed (Dev)" card appears on home view
- [ ] Click expand button - card should expand
- [ ] Verify filter controls appear (Kind filter, Time filter, Refresh button)
- [ ] Verify relay connection indicators appear
  - Default relays should show "connecting" then "connected" (green)
- [ ] Wait for events to appear (may take 10-60 seconds depending on relay activity)
- [ ] Verify event cards display correctly:
  - Type badge color matches event type
  - Kind name appears (e.g., "Text Summarization")
  - Event ID truncated to 8 chars
  - Timestamp shows relative time
  - Content preview limited to 2 lines
- [ ] Click "Show more" on an event - should expand to show full details
- [ ] Verify expanded detail shows:
  - Full event ID with copy button
  - Signature verification status (checkmark or X)
  - Full content (monospace font)
  - All tags in table format
- [ ] Click "Copy Event JSON" - should copy to clipboard
  - Paste in text editor - should be valid JSON
- [ ] Change kind filter to "Job Requests" - feed should update
- [ ] Change time filter to "Last Hour" - feed should update
- [ ] Click "Refresh" button - feed should clear and re-subscribe
- [ ] Collapse card - feed should stop subscriptions
- [ ] Expand card again - feed should restart
- [ ] Quit app and relaunch - card should remember collapsed/expanded state
- [ ] Verify no crashes during 10-minute session
- [ ] Verify no memory leaks (Instruments > Leaks)

## Apple Compliance Considerations

### App Store Review Guidelines

**ASRG 2.5.2 (No Downloaded Code)**: ✅ **Compliant**
- Event content is **data** (JSON), not executable code
- Parsing and displaying JSON events does not execute code
- No dynamic code loading or interpretation

**ASRG 2.5.4 (Background Services)**: ✅ **Compliant**
- Feed only active when view is visible (expanded card)
- Subscriptions cancelled when view collapses or app backgrounds
- No ambient background execution
- No persistent background tasks

**ASRG 2.4.2 (Power Efficiency)**: ✅ **Compliant**
- WebSocket connections are lightweight
- No continuous polling (event-driven via WebSocket)
- Cancel subscriptions when not in use (collapsed card)

**Foundation Models AUP**: ✅ **N/A**
- Feed viewer does not use Foundation Models framework
- Future: If AI summarization of events is added, enforce AUP

### Privacy & Security

**Event Content**: All NIP-90 events are public by design
- No user PII exposed (only public Nostr pubkeys)
- Encrypted events are detected but not decrypted (no keys available)
- Content display is read-only (no modification)

**Relay Connections**: WebSocket Secure (WSS) only
- No plaintext `ws://` connections (all relays use `wss://`)
- No persistent storage of events (in-memory cache only)
- Cache cleared when feed stops or app quits

**Dev-Only Feature**: Not exposed to end users
- `#if DEBUG` preprocessor guard ensures feature is not in RELEASE builds
- Will not be submitted to App Store in production app

## Success Metrics

- [ ] Successfully connect to 3+ Nostr relays
- [ ] Subscribe to all NIP-90 event kinds (5000-7000)
- [ ] Display at least 10 real marketplace events in feed
- [ ] Event detail view shows all fields correctly
- [ ] Signature verification works (shows valid/invalid status)
- [ ] Filters work correctly (kind filter + time filter)
- [ ] No crashes or memory leaks during 10-minute session
- [ ] Feed performance: Handle 100+ events without UI lag
- [ ] Dev-only: Feature is NOT visible in RELEASE builds

## Reference Links

### NIP Specifications
- **NIP-01 (Basic Protocol)**: https://github.com/nostr-protocol/nips/blob/master/01.md
- **NIP-04 (Encrypted Direct Messages)**: https://github.com/nostr-protocol/nips/blob/master/04.md
- **NIP-90 (Data Vending Machines)**: https://github.com/nostr-protocol/nips/blob/master/90.md
- **NIP-89 (Application Handlers)**: https://github.com/nostr-protocol/nips/blob/master/89.md

### OpenAgents
- **Issue #001**: Nostr Client Library (Swift)
- **Issue #002**: secp256k1 & Cryptography
- **Issue #004**: Job Schema Registry
- **Issue #006**: iOS Marketplace Viewer (Read-Only)
- **Issue #007**: macOS Foundation Models Worker
- **Nostr Integration**: `docs/nostr/README.md`
- **DVM Kinds**: `docs/nostr/dvm-kinds/`
- **Apple Terms Research**: `docs/compute/apple-terms-research.md`

### External Tools
- **Nostr Event Explorers**:
  - njump.me (event viewer)
  - nostr.guru (relay inspector)
  - nostrrr.com (dev tools)

## Notes

### Why Dev-Only?

This feed viewer is DEBUG-only because:

1. **Raw Data**: Shows unfiltered, technical event data (not user-friendly)
2. **No Pagination**: MVP has no virtual scrolling limits; high event volume could overwhelm UI
3. **Noise**: May receive many unrelated events from public relays
4. **Unpolished UX**: Minimal styling, rapid prototyping, no animations
5. **Debugging Tool**: Intended for developers, not end users

The production marketplace viewer (issue #006 for iOS) will have:
- Curated UX (provider profiles, job cards, stats)
- Proper pagination and virtual scrolling
- Encrypted content handling (decrypt when user has keys)
- Reputation and filtering (show only quality providers)
- Polish, animations, and professional UI

### Relay Selection Rationale

Default relays chosen for:
- **relay.damus.io**: High uptime, widely used, good for testing
- **relay.nostr.band**: Known NIP-90 support, developer-friendly
- **nos.lol**: Low latency, good for real-time debugging

In DEBUG mode, developers can add custom relays:
- Local relay for testing: `ws://localhost:8080` (dev only)
- Private relay for internal marketplace testing

### Event Volume Considerations

If the marketplace is active, the feed could receive:
- **100+ events/hour** (rough estimate for active marketplace)
- **1000+ events in cache** (max limit enforced)

Performance mitigations:
- **Virtual scrolling**: LazyVStack only renders visible cards
- **Deduplication**: Prevent duplicate event IDs
- **Cache trimming**: FIFO eviction when exceeding 1000 events
- **Debouncing**: Max 10 UI updates/second (batch rapid events)
- **Background processing**: Parse events off main thread

### Future Work (Post-MVP)

The feed viewer can evolve into additional dev tools:

1. **Event Publishing UI**: Test interface to publish sample NIP-90 events
2. **WebSocket Inspector**: View raw WebSocket frames (subscribe/close messages)
3. **Event Search**: Full-text search across event content
4. **Job Flow Visualization**: Link requests → feedback → results visually
5. **Real-time Charts**: Event volume over time, top job kinds
6. **Relay Info (NIP-11)**: Display relay metadata (software, version, supported NIPs)
7. **Export Events**: Export to JSON file for offline analysis
8. **Advanced Filtering**: DSL for complex filters (e.g., `kind:5001 AND tag:bid>1000`)

## Future Enhancements (Post-MVP)

- Relay configuration UI (add/remove relays, view NIP-11 relay info)
- Event search (full-text search across content)
- Job flow visualization (link requests → feedback → results)
- Event export (save to JSON file)
- Real-time charts (event volume, top kinds)
- WebSocket frame inspector (raw protocol debugging)
- Event publishing UI (test interface to publish sample events)
- Advanced filter DSL (e.g., `kind:5001 AND tag:param=model`)
