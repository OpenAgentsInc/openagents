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

### Provider Discovery (NIP-89)
- [ ] Subscribe to NIP-89 capability advertisement events (kind:31990)
- [ ] Display list of active providers (pubkey, supported job kinds, pricing)
- [ ] Filter providers by job kind
- [ ] Sort providers by reputation, price, availability
- [ ] Provider detail view (capabilities, stats, recent jobs)

### Job Browser
- [ ] List available job kinds from registry (issue #004)
- [ ] Job kind detail view (description, params, pricing range)
- [ ] Filter by category (text, code, image, etc.)
- [ ] Search job kinds by keyword

### Activity Feed
- [ ] Subscribe to recent job requests (kind:5000-5999)
- [ ] Subscribe to job results (kind:6000-6999)
- [ ] Subscribe to feedback events (kind:7000)
- [ ] Display activity timeline (jobs submitted → processing → completed)
- [ ] Filter by job kind or provider
- [ ] Encrypted content indicator (don't show private params)

### Provider Stats
- [ ] Calculate stats from Nostr events:
  - Jobs completed (count of kind:6000-6999 results)
  - Success rate (success feedback / total jobs)
  - Avg turnaround time (estimate from timestamps)
  - Active since (first capability ad)
- [ ] Display stats on provider detail view
- [ ] Cache stats (refresh periodically)

### UI/UX
- [ ] Marketplace tab in bottom navigation
- [ ] Pull-to-refresh for latest data
- [ ] Empty states (no providers, no activity)
- [ ] Loading indicators during Nostr subscription
- [ ] Error states (relay disconnected, no data)

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
