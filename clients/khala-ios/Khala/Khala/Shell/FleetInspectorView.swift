import SwiftUI

struct FleetInspectorView: View {
    let status: FleetInspectorStatus?
    let isLoading: Bool
    let errorText: String?
    let onRefresh: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            Divider()
            ScrollView {
                VStack(alignment: .leading, spacing: 18) {
                    if let errorText {
                        notice(icon: "exclamationmark.triangle.fill", title: "Inspector unavailable", text: errorText, color: .orange)
                    }
                    if let status {
                        identitySection(status)
                        pylonSection(status)
                        appleSection(status.appleFM)
                        refsSection("Capacity", icon: "bolt.horizontal.circle", refs: status.capacityRefs)
                        refsSection("Load", icon: "tray.2", refs: status.loadRefs)
                        recentSection(status)
                        refsSection("Proofs", icon: "checkmark.seal", refs: status.proofRefs)
                        refsSection("Blockers", icon: "xmark.octagon", refs: status.blockerRefs)
                    } else if isLoading {
                        ProgressView("Loading fleet status...")
                            .frame(maxWidth: .infinity, alignment: .center)
                            .padding(.top, 36)
                    } else {
                        notice(
                            icon: "network",
                            title: "No fleet snapshot",
                            text: "OpenAgents identity, Pylon readiness, provider accounts, Apple FM status, capacity, load, and proof refs appear here after refresh.",
                            color: .secondary
                        )
                    }
                }
                .padding(16)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(.regularMaterial)
    }

    private var header: some View {
        HStack(spacing: 10) {
            Image(systemName: "rectangle.righthalf.inset.filled")
                .font(.title3)
            VStack(alignment: .leading, spacing: 2) {
                Text("Fleet")
                    .font(.headline)
                Text(status.map { "Updated \($0.fetchedAt.formatted(date: .omitted, time: .shortened))" } ?? "Node readiness")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer()
            Button(action: onRefresh) {
                Image(systemName: "arrow.clockwise")
                    .frame(width: 34, height: 34)
            }
            .disabled(isLoading)
            .accessibilityLabel("Refresh fleet status")
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 14)
    }

    private func identitySection(_ status: FleetInspectorStatus) -> some View {
        section("Identity", icon: "person.crop.circle.badge.checkmark") {
            detailRow("OpenAgents", status.connectedIdentity ?? "Not published")
            detailRow("Local agent", status.localAgentIdentity ?? "Not published")
        }
    }

    private func pylonSection(_ status: FleetInspectorStatus) -> some View {
        section("Pylon", icon: "antenna.radiowaves.left.and.right") {
            HStack {
                detailRow("Readiness", status.pylonReadiness.label)
                Spacer()
                badge(status.pylonReadiness)
            }
            detailRow("Pylon ref", status.pylonRef ?? "Not published")
            if let heartbeatObservedAt = status.heartbeatObservedAt {
                detailRow("Heartbeat", heartbeatObservedAt)
            }
            if let heartbeatFresh = status.heartbeatFresh {
                HStack {
                    detailRow("Freshness", heartbeatFresh ? "Fresh" : "Stale")
                    Spacer()
                    badge(heartbeatFresh ? .available : .stale)
                }
            }
            if status.providerAccounts.isEmpty {
                detailRow("Providers", "No connected provider accounts published")
            } else {
                ForEach(status.providerAccounts) { account in
                    HStack(alignment: .firstTextBaseline) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text("\(account.provider) \(account.ref)")
                                .font(.footnote.weight(.semibold))
                                .foregroundStyle(.primary)
                            if let detail = account.detail {
                                Text(detail)
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(2)
                            }
                        }
                        Spacer()
                        badge(account.readiness)
                    }
                }
            }
        }
    }

    private func appleSection(_ apple: FleetInspectorStatus.AppleFM) -> some View {
        section("Apple FM", icon: "apple.logo") {
            HStack {
                detailRow("Readiness", apple.summary)
                Spacer()
                badge(apple.readiness)
            }
            ForEach(apple.blockerRefs, id: \.self) { ref in
                refPill(ref, tone: .blocked)
            }
        }
    }

    @ViewBuilder
    private func refsSection(_ title: String, icon: String, refs: [String]) -> some View {
        section(title, icon: icon) {
            if refs.isEmpty {
                detailRow(title, "No refs published")
            } else {
                FlowStack(items: refs) { ref in
                    refPill(ref)
                }
            }
        }
    }

    private func recentSection(_ status: FleetInspectorStatus) -> some View {
        section("Recent Work", icon: "clock.arrow.circlepath") {
            if status.recentRefs.isEmpty {
                detailRow("Closeouts", "No recent assignment or closeout refs published")
            } else {
                ForEach(status.recentRefs) { ref in
                    VStack(alignment: .leading, spacing: 3) {
                        Text(ref.kind.capitalized)
                            .font(.caption.weight(.semibold))
                            .foregroundStyle(.secondary)
                        refPill(ref.value)
                    }
                }
            }
        }
    }

    private func section<Content: View>(_ title: String, icon: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Label(title, systemImage: icon)
                .font(.subheadline.weight(.semibold))
            content()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func detailRow(_ label: String, _ value: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(label)
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
            Text(value)
                .font(.footnote)
                .foregroundStyle(.primary)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func badge(_ availability: FleetInspectorStatus.Availability) -> some View {
        Label(availability.label, systemImage: badgeIcon(availability))
            .font(.caption.weight(.semibold))
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .background(badgeColor(availability).opacity(0.16), in: Capsule())
            .foregroundStyle(badgeColor(availability))
    }

    private func refPill(_ ref: String, tone: FleetInspectorStatus.Availability = .unknown) -> some View {
        Text(ref)
            .font(.caption.monospaced())
            .foregroundStyle(.primary)
            .lineLimit(3)
            .textSelection(.enabled)
            .padding(.horizontal, 8)
            .padding(.vertical, 6)
            .background(badgeColor(tone).opacity(tone == .unknown ? 0.10 : 0.16), in: RoundedRectangle(cornerRadius: 7))
    }

    private func notice(icon: String, title: String, text: String, color: Color) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Label(title, systemImage: icon)
                .font(.subheadline.weight(.semibold))
                .foregroundStyle(color)
            Text(text)
                .font(.footnote)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func badgeIcon(_ availability: FleetInspectorStatus.Availability) -> String {
        switch availability {
        case .available: return "checkmark.circle.fill"
        case .stale: return "clock.badge.exclamationmark.fill"
        case .blocked: return "xmark.octagon.fill"
        case .unknown: return "questionmark.circle.fill"
        }
    }

    private func badgeColor(_ availability: FleetInspectorStatus.Availability) -> Color {
        switch availability {
        case .available: return .green
        case .stale: return .orange
        case .blocked: return .red
        case .unknown: return .secondary
        }
    }
}

private struct FlowStack<Item: Hashable, Content: View>: View {
    let items: [Item]
    let content: (Item) -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            ForEach(items, id: \.self) { item in
                content(item)
            }
        }
    }
}
