import SwiftUI
import Combine
import OpenAgentsCore

#if os(macOS)
#if DEBUG
struct NostrEventFeedView: View {
    @StateObject private var feedManager: NostrEventFeedManager
    private let relayManagerRef: NostrRelayManager
    @State private var isExpanded: Bool = UserDefaults.standard.bool(forKey: "dev.nostr.feed.expanded")

    init(relayManager: NostrRelayManager) {
        self.relayManagerRef = relayManager
        _feedManager = StateObject(wrappedValue: NostrEventFeedManager(relayManager: relayManager))
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            header

            if isExpanded {
                VStack(alignment: .leading, spacing: 16) {
                    filterBar
                    NostrRelayStatusView(relayManager: relayManagerRef)
                    content
                }
                .frame(maxHeight: 600)
            }
        }
        .padding(20)
        .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(OATheme.Colors.border.opacity(0.3)))
        .task { await feedManager.startFeed() }
        .onDisappear { feedManager.stopFeed() }
    }

    private var header: some View {
        HStack(spacing: 10) {
            Image(systemName: "antenna.radiowaves.left.and.right")
                .foregroundStyle(OATheme.Colors.accent)
            Text("Nostr Event Feed (Dev)")
                .font(OAFonts.ui(.headline, 16))
                .foregroundStyle(OATheme.Colors.textSecondary)
            Spacer()
            Button(isExpanded ? "Collapse" : "Expand") {
                isExpanded.toggle()
                UserDefaults.standard.set(isExpanded, forKey: "dev.nostr.feed.expanded")
                if isExpanded {
                    Task { await feedManager.startFeed() }
                } else {
                    feedManager.stopFeed()
                }
            }
            .buttonStyle(.plain)
            .font(OAFonts.mono(.body, 12))
        }
    }

    private var filterBar: some View {
        HStack(spacing: 12) {
            Picker("Kinds", selection: $feedManager.selectedKindFilter) {
                Text("All NIP-90").tag(NostrEventFeedManager.KindFilter.all)
                Text("Job Requests").tag(NostrEventFeedManager.KindFilter.requests)
                Text("Job Results").tag(NostrEventFeedManager.KindFilter.results)
                Text("Job Feedback").tag(NostrEventFeedManager.KindFilter.feedback)
                Text("OpenAgents Custom").tag(NostrEventFeedManager.KindFilter.customOnly)
            }
            .pickerStyle(.menu)
            .font(OAFonts.mono(.body, 12))

            Picker("Time", selection: $feedManager.timeFilter) {
                Text("Last Hour").tag(NostrEventFeedManager.TimeFilter.lastHour)
                Text("Last 24 Hours").tag(NostrEventFeedManager.TimeFilter.last24Hours)
                Text("Last 7 Days").tag(NostrEventFeedManager.TimeFilter.last7Days)
                Text("Last Month").tag(NostrEventFeedManager.TimeFilter.lastMonth)
                Text("All Time").tag(NostrEventFeedManager.TimeFilter.allTime)
            }
            .pickerStyle(.menu)
            .font(OAFonts.mono(.body, 12))

            Spacer()

            Button {
                Task { await feedManager.refreshFeed() }
            } label: {
                Label("Refresh", systemImage: "arrow.clockwise")
            }
            .buttonStyle(.bordered)
            .font(OAFonts.mono(.body, 12))
        }
        // Ensure Berkeley Mono applies to nested controls and menu labels.
        .environment(\.font, OAFonts.mono(.body, 12))
    }

    private var content: some View {
        Group {
            if feedManager.isLoading {
                HStack(spacing: 8) {
                    ProgressView()
                    Text("Subscribing to Nostr relays...")
                }
                .font(OAFonts.ui(.body, 14))
                .foregroundStyle(OATheme.Colors.textSecondary)
            } else if let err = feedManager.error {
                VStack(alignment: .leading, spacing: 8) {
                    Text("Error: \(err.description)")
                        .foregroundStyle(OATheme.Colors.danger)
                    Button("Retry") { Task { await feedManager.startFeed() } }
                }
            } else if feedManager.events.isEmpty {
                Text("No events yet. Waiting for marketplace activity...")
                    .font(OAFonts.ui(.body, 14))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 10) {
                        ForEach(feedManager.events) { item in
                            NostrEventCard(item: item, feedManager: feedManager)
                        }
                    }
                }
            }
        }
    }
}

#endif
#endif
