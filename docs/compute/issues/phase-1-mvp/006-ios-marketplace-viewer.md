# iOS: Marketplace Viewer (Read-Only)

**Phase:** 1 - MVP
**Component:** iOS App
**Priority:** P1 (High - Enables marketplace discovery)
**Estimated Effort:** 1-2 weeks

## Summary

Implement a read-only marketplace viewer in the iOS app that allows users to browse available compute providers, view job capabilities, and see marketplace activity. This provides visibility into the marketplace without requiring full buyer/seller functionality (which comes in Phase 2).

## Motivation

For MVP, iOS needs to display the marketplace state:
- **Browse providers**: See who's offering compute (NIP-89 capability ads)
- **View job kinds**: Discover what services are available (from Job Schema Registry)
- **Monitor activity**: See recent jobs, results, feedback (NIP-90 events)
- **Build trust**: Reputation indicators, completion rates

This read-only view validates marketplace UX before implementing full buyer/seller flows.

## Acceptance Criteria

### Provider Discovery (NIP-89) - (Pattern 2: Content Discovery)
- [ ] Subscribe to NIP-89 capability advertisement events (kind:31990)
- [ ] **Display list of active providers** with scannable cards
  - Show pubkey (truncated with identicon for visual recognition)
  - Supported job kinds with icons (max 3-5 visible, "+X more" for overflow)
  - **Pricing preview**: "From 100 sats" or "Free" badge
  - **Availability indicator**: Green dot for "online", gray for "offline/unknown"
- [ ] **Filter providers by job kind** (Pattern 5: Progressive Complexity)
  - Default: Show all providers
  - Filter dropdown: "All Services", "Text Processing", "Code", "Media", etc.
  - **Filter chips** (show active filters, tap to remove)
  - Clear all filters button when multiple filters active
- [ ] **Sort providers** with clear options
  - Options: "Recommended" (default), "Lowest Price", "Most Jobs", "Newest"
  - **Visual indicator** showing current sort order
  - **Sticky sort** (remember user preference)
- [ ] **Provider detail view** (Pattern 5: Progressive Disclosure)
  - Summary view (top): Avatar, name, verification status, total jobs
  - Capabilities section (expandable list of job kinds with pricing)
  - Stats section (jobs completed, success rate, avg turnaround)
  - Recent jobs section (last 5-10 jobs with status)

### Job Browser
- [ ] List available job kinds from registry (issue #004)
- [ ] Job kind detail view (description, params, pricing range)
- [ ] Filter by category (text, code, image, etc.)
- [ ] Search job kinds by keyword

### Activity Feed (Pattern 2: Content Discovery + Pattern 4: Performance)
- [ ] Subscribe to recent job requests (kind:5000-5999)
- [ ] Subscribe to job results (kind:6000-6999)
- [ ] Subscribe to feedback events (kind:7000)
- [ ] **Display activity timeline** with clear visual hierarchy
  - **Activity cards** showing: Job type icon, status badge, timestamp, provider
  - **Status flow visualization**: "Submitted → Processing → Completed" with progress indicator
  - **Relative timestamps**: "2m ago", "1h ago", "Yesterday"
  - **Color-coded status**: Blue (submitted), Yellow (processing), Green (completed), Red (error)
- [ ] **Filter by job kind or provider** (Pattern 5: Progressive Complexity)
  - Default: Show all activity
  - **Quick filters** (chips): "My Jobs", "Completed", "In Progress", "Failed"
  - **Advanced filters** (collapsed): Job kind, Provider, Date range
  - **Active filter indicator**: Show count of active filters
- [ ] **Encrypted content indicator** with clear messaging
  - Don't show private params (privacy-safe)
  - Show icon with tooltip: "🔒 Encrypted parameters (not visible)"
  - **Not an error**: Use neutral gray color, not red/warning
- [ ] **Performance optimizations**:
  - **Lazy loading**: Load 20 items initially, load more on scroll
  - **Skeleton screens**: Show placeholder cards while loading
  - **Pull-to-refresh**: Refresh activity feed with haptic feedback
  - **Infinite scroll**: Load more items as user scrolls (max 200 cached)

### Provider Stats (Pattern 6: Sync State Visibility)
- [ ] **Calculate stats from Nostr events** with clear data sources:
  - **Jobs completed**: Count of kind:6000-6999 results
  - **Success rate**: success feedback / total jobs (show percentage + count)
  - **Avg turnaround time**: Estimate from timestamps (show in human-readable format: "~5 min", "~2 hours")
  - **Active since**: First capability ad (relative: "Active for 3 months")
- [ ] **Display stats on provider detail view** with context
  - Use **visual indicators**: Progress bars for success rate, sparklines for trend
  - Show **confidence level**: "Based on 50 jobs" or "Limited data (5 jobs)" for new providers
  - **Empty state**: "No stats yet" for providers with no completed jobs (not "0%" which looks bad)
- [ ] **Cache stats with visible refresh status** (Pattern 6)
  - **Last updated timestamp**: "Updated 5 minutes ago"
  - **Refresh indicator**: Small spinner during refresh
  - **Refresh periodically**: Every 5 minutes when view is visible
  - **Manual refresh**: Pull-to-refresh gesture
  - **Stale data indicator**: Show "Stats may be outdated" if >30 minutes old

### UI/UX (Pattern 3: Core Interactions + Pattern 2: Content Discovery)
- [ ] **Marketplace tab in bottom navigation**
  - Icon: Store or network icon
  - Label: "Marketplace" (not "Nostr Marketplace" - avoid jargon)
  - **Badge**: Show count of new providers or activity (optional)
- [ ] **Pull-to-refresh** for latest data (Pattern 3)
  - **Haptic feedback** when refresh starts
  - **Progress indicator** during refresh (spinner in nav bar)
  - **Success feedback**: Brief "Updated" toast or checkmark
  - **Error feedback**: "Update failed. Pull to retry." with error icon
- [ ] **Empty states** (Pattern 2: Content Discovery)
  - **No providers found**:
    - Icon: Magnifying glass or network icon
    - Title: "No providers yet"
    - Message: "The marketplace is just getting started. Check back soon!"
    - **Action button**: "Refresh" to retry
  - **No activity**:
    - Icon: Clock or activity icon
    - Title: "No recent activity"
    - Message: "Marketplace activity will appear here once jobs are submitted."
    - **No action needed** (passive state)
  - **No search results**:
    - Icon: Magnifying glass
    - Title: "No providers found"
    - Message: "Try different search terms or filters"
    - **Action button**: "Clear filters"
- [ ] **Loading indicators** during Nostr subscription (Pattern 4: Performance)
  - **Skeleton screens** (3-5 placeholder provider cards) during initial load
  - **Progressive loading**: Show providers as they arrive (don't wait for all relays)
  - **Loading text**: "Discovering providers..." → "Found X providers" when complete
  - **Don't block UI**: Show header and tabs immediately, load content below
- [ ] **Error states** with clear recovery actions (Pattern 3)
  - **Relay disconnected**:
    - Icon: Warning triangle or disconnected icon
    - Title: "Connection lost"
    - Message: "Can't connect to marketplace. Check your internet connection."
    - **Action button**: "Retry" (prominent)
  - **No data (timeout)**:
    - Icon: Clock or timeout icon
    - Title: "Taking longer than expected"
    - Message: "Still searching for providers. This may take a minute."
    - **Action button**: "Keep waiting" or "Cancel"
  - **Permission error** (if applicable):
    - Icon: Lock icon
    - Title: "Access denied"
    - Message: Clear explanation of what permission is needed and why
    - **Action button**: "Open Settings"

## Technical Design

### UI Structure

```
Marketplace/
├── MarketplaceTabView           // Main marketplace tab
│   ├── ProvidersView            // Provider list
│   ├── JobKindsView             // Job browser
│   └── ActivityFeedView         // Recent activity
├── ProviderDetailView           // Provider details + stats
├── JobKindDetailView            // Job kind info
└── JobDetailView                // Individual job (request → result)
```

### SwiftUI Views

```swift
// ios/OpenAgents/Views/Marketplace/

MarketplaceTabView.swift         // Tab container (Providers/Jobs/Activity)
ProvidersView.swift              // Provider list
ProviderDetailView.swift         // Provider detail
JobKindsView.swift               // Job kind browser
JobKindDetailView.swift          // Job kind detail
ActivityFeedView.swift           // Activity timeline
JobDetailView.swift              // Job request/result detail
MarketplaceEmptyView.swift       // Empty states
```

### View Models

```swift
// ios/OpenAgents/ViewModels/

MarketplaceViewModel.swift       // Marketplace state
ProviderViewModel.swift          // Provider data + stats
ActivityViewModel.swift          // Activity feed
```

### Core Types

```swift
// MarketplaceViewModel.swift

import Foundation
import OpenAgentsCore

@MainActor
class MarketplaceViewModel: ObservableObject {
    @Published var providers: [Provider] = []
    @Published var recentActivity: [ActivityItem] = []
    @Published var isLoading = false
    @Published var error: MarketplaceError?

    private let nostrClient: NostrRelayManager
    private let schemaRegistry = JobSchemaRegistry.shared

    struct Provider: Identifiable {
        let id: String              // Pubkey
        let npub: String            // Bech32 pubkey
        let capabilities: [JobCapability]
        let stats: ProviderStats
        let lastSeen: Date

        struct JobCapability {
            let jobKind: JobKind
            let version: String
            let pricing: PricingModel
            let limits: Limits?

            struct PricingModel {
                let basePrice: Int64      // msats
                let perUnitPrice: Int64?
                let unit: String?
            }

            struct Limits {
                let maxInputSize: Int?
                let maxOutputSize: Int?
                let maxTokens: Int?
                let timeout: Int?
            }
        }
    }

    struct ProviderStats {
        let jobsCompleted: Int
        let successRate: Double       // 0.0-1.0
        let avgTurnaround: TimeInterval?
        let activeSince: Date
    }

    struct ActivityItem: Identifiable {
        let id: String                // Event ID
        let type: ActivityType
        let jobKind: JobKind
        let timestamp: Date
        let providerPubkey: String?
        let status: JobStatus

        enum ActivityType {
            case request, feedback, result
        }

        enum JobStatus {
            case submitted
            case paymentRequired
            case processing
            case completed
            case error
        }
    }

    enum MarketplaceError: LocalizedError {
        case nostrConnectionFailed
        case noProvidersFound
        case dataFetchFailed(Error)
    }

    // MARK: - Initialization

    init(nostrClient: NostrRelayManager) {
        self.nostrClient = nostrClient
    }

    // MARK: - Provider Discovery

    func discoverProviders() async {
        isLoading = true
        defer { isLoading = false }

        do {
            // Subscribe to NIP-89 capability ads (kind:31990)
            try await nostrClient.subscribe(
                id: "marketplace-providers",
                filters: [NostrFilter(kinds: [31990])],
                relays: nil
            ) { [weak self] event, _ in
                self?.handleCapabilityEvent(event)
            }
        } catch {
            self.error = .dataFetchFailed(error)
        }
    }

    private func handleCapabilityEvent(_ event: NostrEvent) {
        // Parse NIP-89 event → Provider
        guard let capability = try? parseCapability(event) else { return }

        // Update or add provider
        if let index = providers.firstIndex(where: { $0.id == event.pubkey }) {
            // Update existing provider
            var provider = providers[index]
            if !provider.capabilities.contains(where: { $0.jobKind == capability.jobKind }) {
                provider.capabilities.append(capability)
                providers[index] = provider
            }
        } else {
            // New provider
            let provider = Provider(
                id: event.pubkey,
                npub: try! NostrBech32.npub(from: event.pubkey),
                capabilities: [capability],
                stats: ProviderStats(
                    jobsCompleted: 0,
                    successRate: 0.0,
                    avgTurnaround: nil,
                    activeSince: Date(timeIntervalSince1970: TimeInterval(event.created_at))
                ),
                lastSeen: Date(timeIntervalSince1970: TimeInterval(event.created_at))
            )
            providers.append(provider)
        }
    }

    private func parseCapability(_ event: NostrEvent) throws -> Provider.JobCapability {
        // Decode capability from event content (JSON)
        // See ProviderCapability from issue #004
        fatalError("Not implemented")
    }

    // MARK: - Activity Feed

    func subscribeToActivity() async {
        do {
            // Subscribe to job requests (kind:5000-5999)
            try await nostrClient.subscribe(
                id: "marketplace-requests",
                filters: [NostrFilter(kinds: Array(5000...5999), limit: 50)],
                relays: nil
            ) { [weak self] event, _ in
                self?.handleActivityEvent(event, type: .request)
            }

            // Subscribe to results (kind:6000-6999)
            try await nostrClient.subscribe(
                id: "marketplace-results",
                filters: [NostrFilter(kinds: Array(6000...6999), limit: 50)],
                relays: nil
            ) { [weak self] event, _ in
                self?.handleActivityEvent(event, type: .result)
            }

            // Subscribe to feedback (kind:7000)
            try await nostrClient.subscribe(
                id: "marketplace-feedback",
                filters: [NostrFilter(kinds: [7000], limit: 50)],
                relays: nil
            ) { [weak self] event, _ in
                self?.handleActivityEvent(event, type: .feedback)
            }
        } catch {
            self.error = .dataFetchFailed(error)
        }
    }

    private func handleActivityEvent(_ event: NostrEvent, type: ActivityItem.ActivityType) {
        let item = ActivityItem(
            id: event.id,
            type: type,
            jobKind: JobKind(rawValue: event.kind) ?? .textSummarization,
            timestamp: Date(timeIntervalSince1970: TimeInterval(event.created_at)),
            providerPubkey: type == .result ? event.pubkey : nil,
            status: statusFrom(event, type: type)
        )

        // Insert sorted by timestamp (newest first)
        if let index = recentActivity.firstIndex(where: { $0.timestamp < item.timestamp }) {
            recentActivity.insert(item, at: index)
        } else {
            recentActivity.append(item)
        }

        // Keep only recent 100 items
        if recentActivity.count > 100 {
            recentActivity = Array(recentActivity.prefix(100))
        }
    }

    private func statusFrom(_ event: NostrEvent, type: ActivityItem.ActivityType) -> ActivityItem.JobStatus {
        switch type {
        case .request:
            return .submitted
        case .feedback:
            // Parse status tag
            if let statusTag = event.tag("status")?.last {
                switch statusTag {
                case "payment-required": return .paymentRequired
                case "processing": return .processing
                case "success": return .completed
                case "error": return .error
                default: return .submitted
                }
            }
            return .submitted
        case .result:
            return .completed
        }
    }

    // MARK: - Stats Calculation

    func calculateStats(for providerPubkey: String) async -> ProviderStats {
        // Query Nostr for provider's job results (kind:6000-6999)
        // Count completed jobs, success rate, avg turnaround
        // For MVP: Return placeholder or cached stats
        return ProviderStats(
            jobsCompleted: 0,
            successRate: 1.0,
            avgTurnaround: nil,
            activeSince: Date()
        )
    }
}
```

### UI Example

```swift
// ProvidersView.swift

struct ProvidersView: View {
    @StateObject private var viewModel: MarketplaceViewModel

    var body: some View {
        NavigationStack {
            List {
                if viewModel.isLoading {
                    ProgressView("Discovering providers...")
                } else if viewModel.providers.isEmpty {
                    MarketplaceEmptyView(
                        icon: "person.3.fill",
                        title: "No Providers Found",
                        message: "Check back later or try different relays"
                    )
                } else {
                    ForEach(viewModel.providers) { provider in
                        NavigationLink(destination: ProviderDetailView(provider: provider)) {
                            ProviderRow(provider: provider)
                        }
                    }
                }
            }
            .navigationTitle("Providers")
            .refreshable {
                await viewModel.discoverProviders()
            }
            .task {
                await viewModel.discoverProviders()
            }
        }
    }
}

struct ProviderRow: View {
    let provider: MarketplaceViewModel.Provider

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(provider.npub.prefix(12) + "...")
                    .font(.headline)
                Spacer()
                Text("\(provider.capabilities.count) services")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            HStack {
                Label("\(provider.stats.jobsCompleted) completed", systemImage: "checkmark.circle.fill")
                    .font(.caption)
                    .foregroundColor(.green)

                Spacer()

                if provider.stats.successRate > 0 {
                    Text("\(Int(provider.stats.successRate * 100))% success")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
        }
        .padding(.vertical, 4)
    }
}
```

```swift
// ActivityFeedView.swift

struct ActivityFeedView: View {
    @StateObject private var viewModel: MarketplaceViewModel

    var body: some View {
        NavigationStack {
            List {
                if viewModel.recentActivity.isEmpty {
                    MarketplaceEmptyView(
                        icon: "chart.line.uptrend.xyaxis",
                        title: "No Recent Activity",
                        message: "Marketplace activity will appear here"
                    )
                } else {
                    ForEach(viewModel.recentActivity) { item in
                        NavigationLink(destination: JobDetailView(activityItem: item)) {
                            ActivityRow(item: item)
                        }
                    }
                }
            }
            .navigationTitle("Activity")
            .refreshable {
                await viewModel.subscribeToActivity()
            }
            .task {
                await viewModel.subscribeToActivity()
            }
        }
    }
}

struct ActivityRow: View {
    let item: MarketplaceViewModel.ActivityItem

    var body: some View {
        HStack {
            Image(systemName: iconFor(item.type))
                .foregroundColor(colorFor(item.status))

            VStack(alignment: .leading, spacing: 2) {
                Text(item.jobKind.displayName)
                    .font(.headline)
                Text(item.timestamp, style: .relative)
                    .font(.caption)
                    .foregroundColor(.secondary)
            }

            Spacer()

            StatusBadge(status: item.status)
        }
    }

    private func iconFor(_ type: MarketplaceViewModel.ActivityItem.ActivityType) -> String {
        switch type {
        case .request: return "arrow.up.circle"
        case .feedback: return "info.circle"
        case .result: return "checkmark.circle"
        }
    }

    private func colorFor(_ status: MarketplaceViewModel.ActivityItem.JobStatus) -> Color {
        switch status {
        case .submitted: return .blue
        case .paymentRequired: return .orange
        case .processing: return .yellow
        case .completed: return .green
        case .error: return .red
        }
    }
}

struct StatusBadge: View {
    let status: MarketplaceViewModel.ActivityItem.JobStatus

    var body: some View {
        Text(status.rawValue.capitalized)
            .font(.caption2)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(background)
            .foregroundColor(.white)
            .cornerRadius(8)
    }

    private var background: Color {
        switch status {
        case .submitted: return .blue
        case .paymentRequired: return .orange
        case .processing: return .yellow
        case .completed: return .green
        case .error: return .red
        }
    }
}
```

## Dependencies

### OpenAgents Dependencies
- **Issue #001**: Nostr Client Library (subscriptions, relay management)
- **Issue #004**: Job Schema Registry (JobKind definitions)
- **Issue #005**: iOS Nostr Identity (active identity for subscriptions)

### System Frameworks
- **SwiftUI**: UI framework
- **Foundation**: Core types

## Testing Requirements

### Unit Tests
- [ ] Parse NIP-89 capability events
- [ ] Calculate provider stats from events
- [ ] Activity feed sorting (newest first)
- [ ] Filter providers by job kind
- [ ] Status badge color logic

### UI Tests
- [ ] Provider list displays correctly
- [ ] Pull-to-refresh updates data
- [ ] Navigation to provider detail
- [ ] Activity feed scrolling
- [ ] Empty states render correctly

### Integration Tests
- [ ] Subscribe to real Nostr relays
- [ ] Receive capability advertisements
- [ ] Receive job activity events
- [ ] Real-time updates in UI

## Apple Compliance Considerations

### App Store Review Guidelines

**ASRG 2.5.2 (No Downloaded Code)**
- ✅ **Compliant**: Nostr events are data (JSON), not code
- ✅ Read-only view (no execution)

**ASRG 5.1.1 (Privacy)**
- ✅ Public Nostr events only (no private data)
- ⚠️ **Note**: Encrypted content not displayed (privacy-safe)

**No IAP issues** (read-only, no purchases)

## Reference Links

### Specifications
- **NIP-89** (Capability Ads): https://github.com/nostr-protocol/nips/blob/master/89.md
- **NIP-90** (DVM): https://github.com/nostr-protocol/nips/blob/master/90.md

### OpenAgents
- **Issue #001**: Nostr Client Library
- **Issue #004**: Job Schema Registry
- **Issue #005**: iOS Nostr Identity

## Success Metrics

- [ ] Discover 5+ providers on public relays
- [ ] Display 10+ job kinds from registry
- [ ] Activity feed updates in real-time
- [ ] UI responsive (<16ms frame time)
- [ ] Published in TestFlight

## Notes

- **Read-Only**: Phase 1 MVP focuses on visibility, not transactions
- **No Authentication**: Public Nostr events only
- **Stats**: Simple counts for MVP (improve in Phase 2)
- **Caching**: Consider local cache for offline viewing

## Future Enhancements (Post-MVP)

- Filter by price range
- Provider ratings/reviews
- Job result previews (decrypted if user has key)
- Notifications for new providers
- Search providers by capability
