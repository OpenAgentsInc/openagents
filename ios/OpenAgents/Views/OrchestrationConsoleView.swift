import SwiftUI
import OpenAgentsCore

#if os(macOS)

/// macOS Orchestration Console - manage overnight orchestration configs
struct OrchestrationConsoleView: View {
    @EnvironmentObject var tinyvex: TinyvexManager
    @EnvironmentObject var bridgeManager: BridgeManager

    @State private var configs: [OrchestrationConfig] = []
    @State private var selectedConfig: OrchestrationConfig?
    @State private var showingEditor = false
    @State private var editingConfig: OrchestrationConfig?
    @State private var isLoading = false
    @State private var errorMessage: String?
    // Live status
    @State private var schedulerRunning = false
    @State private var schedulerNextWake: Int? = nil
    @State private var schedulerMessage: String = ""
    @State private var activeConfigId: String? = nil
    @State private var lastCoordinatorStatus: (cycles: Int, executed: Int, completed: Int, failed: Int, cancelled: Int, lastTs: Int64?) = (0,0,0,0,0,nil)
    @State private var statusTimer = Timer.publish(every: 5.0, on: .main, in: .common).autoconnect()

    var body: some View {
        VStack(spacing: 0) {
            // Header
            header

            Divider()

            // Content
            if isLoading {
                loadingView
            } else if configs.isEmpty {
                emptyStateView
            } else {
                configListView
            }
        }
        .frame(minWidth: 600, minHeight: 400)
        .background(OATheme.Colors.background)
        .task {
            await loadConfigs()
            await refreshStatuses()
        }
        .onReceive(statusTimer) { _ in
            Task { await refreshStatuses() }
        }
        .sheet(isPresented: $showingEditor) {
            OrchestrationConfigEditor(
                config: editingConfig,
                onSave: { config in
                    await saveConfig(config)
                }
            )
        }
    }

    // MARK: - Header

    @ViewBuilder
    private var header: some View {
        HStack {
            VStack(alignment: .leading, spacing: 4) {
                Text("Orchestration Console")
                    .font(OAFonts.ui(.title, 20))
                    .fontWeight(.semibold)
                    .foregroundStyle(OATheme.Colors.textPrimary)

                Text("\(configs.count) config\(configs.count == 1 ? "" : "s") • Active: \(activeConfigId ?? "—") • Scheduler: \(schedulerRunning ? "running" : "stopped")")
                    .font(OAFonts.ui(.caption, 13))
                    .foregroundStyle(OATheme.Colors.textSecondary)
                if let next = schedulerNextWake {
                    Text("Next: \(Date(timeIntervalSince1970: TimeInterval(next))) • \(schedulerMessage)")
                        .font(OAFonts.ui(.caption, 12))
                        .foregroundStyle(OATheme.Colors.textTertiary)
                }
                let metrics = lastCoordinatorStatus
                Text("Cycles: \(metrics.cycles) • Executed: \(metrics.executed) • Completed: \(metrics.completed) • Failed: \(metrics.failed)")
                    .font(OAFonts.ui(.caption, 12))
                    .foregroundStyle(OATheme.Colors.textTertiary)
            }

            Spacer()

            HStack(spacing: 12) {
                Button {
                    Task { await reloadScheduler() }
                } label: {
                    Label("Reload", systemImage: "arrow.clockwise.circle.fill")
                }
                .buttonStyle(.bordered)
                Button {
                    Task { await runNow() }
                } label: {
                    Label("Run Now", systemImage: "play.circle.fill")
                }
                .buttonStyle(.borderedProminent)
                .tint(OATheme.Colors.accent)
                Button {
                    editingConfig = nil
                    showingEditor = true
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "plus.circle.fill")
                        Text("New Config")
                    }
                    .font(OAFonts.ui(.body, 14))
                    .foregroundStyle(.white)
                    .padding(.horizontal, 16)
                    .padding(.vertical, 8)
                    .background(
                        Capsule()
                            .fill(OATheme.Colors.accent)
                    )
                }
                .buttonStyle(.plain)
            }
        }
        .padding(20)
    }

    // MARK: - Loading View

    @ViewBuilder
    private var loadingView: some View {
        VStack(spacing: 16) {
            ProgressView()
                .scaleEffect(1.2)

            Text("Loading configurations...")
                .font(OAFonts.ui(.body, 14))
                .foregroundStyle(OATheme.Colors.textSecondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Empty State

    @ViewBuilder
    private var emptyStateView: some View {
        VStack(spacing: 20) {
            Image(systemName: "moon.stars")
                .font(.system(size: 48))
                .foregroundStyle(OATheme.Colors.textSecondary.opacity(0.5))

            VStack(spacing: 8) {
                Text("No Orchestration Configs")
                    .font(OAFonts.ui(.headline, 16))
                    .foregroundStyle(OATheme.Colors.textPrimary)

                Text("Create your first config to enable overnight orchestration")
                    .font(OAFonts.ui(.body, 14))
                    .foregroundStyle(OATheme.Colors.textSecondary)
                    .multilineTextAlignment(.center)
            }

            Button {
                editingConfig = nil
                showingEditor = true
            } label: {
                HStack(spacing: 6) {
                    Image(systemName: "plus.circle.fill")
                    Text("Create Config")
                }
                .font(OAFonts.ui(.body, 14))
                .foregroundStyle(.white)
                .padding(.horizontal, 20)
                .padding(.vertical, 10)
                .background(
                    Capsule()
                        .fill(OATheme.Colors.accent)
                )
            }
            .buttonStyle(.plain)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Config List

    @ViewBuilder
    private var configListView: some View {
        ScrollView {
            VStack(spacing: 16) {
                ForEach(configs, id: \.id) { config in
                    ConfigRowView(
                        config: config,
                        isSelected: selectedConfig?.id == config.id,
                        onSelect: { selectedConfig = config },
                        onEdit: {
                            editingConfig = config
                            showingEditor = true
                        },
                        onDelete: { await deleteConfig(config) },
                        onActivate: { await activateConfig(config) },
                        onRunPlan: { await runPlan(config) }
                    )
                }
            }
            .padding(20)
        }
    }

    // MARK: - Data Operations

    private func loadConfigs() async {
        isLoading = true
        defer { isLoading = false }

        do {
            guard !tinyvex.dbPath.isEmpty else {
                errorMessage = "Database not available"
                return
            }

            let db = try TinyvexDbLayer(path: tinyvex.dbPath)
            let jsonConfigs = try await db.listAllOrchestrationConfigs()
            configs = try jsonConfigs.compactMap { jsonString in
                guard let data = jsonString.data(using: .utf8) else { return nil }
                return try? JSONDecoder().decode(OrchestrationConfig.self, from: data)
            }
        } catch {
            errorMessage = "Failed to load configs: \(error.localizedDescription)"
        }
    }

    private func saveConfig(_ config: OrchestrationConfig) async {
        do {
            guard !tinyvex.dbPath.isEmpty else { return }

            var updatedConfig = config
            updatedConfig.updatedAt = Int64(Date().timeIntervalSince1970 * 1000)

            let json = try JSONEncoder().encode(updatedConfig)
            guard let jsonString = String(data: json, encoding: .utf8) else { return }

            let db = try TinyvexDbLayer(path: tinyvex.dbPath)
            try await db.insertOrUpdateOrchestrationConfig(
                jsonString,
                id: updatedConfig.id,
                workspaceRoot: updatedConfig.workspaceRoot,
                updatedAt: updatedConfig.updatedAt
            )

            await loadConfigs()
            showingEditor = false
        } catch {
            errorMessage = "Failed to save config: \(error.localizedDescription)"
        }
    }

    private func deleteConfig(_ config: OrchestrationConfig) async {
        do {
            guard !tinyvex.dbPath.isEmpty else { return }

            let db = try TinyvexDbLayer(path: tinyvex.dbPath)
            try await db.deleteOrchestrationConfig(
                id: config.id,
                workspaceRoot: config.workspaceRoot
            )

            if selectedConfig?.id == config.id {
                selectedConfig = nil
            }

            await loadConfigs()
        } catch {
            errorMessage = "Failed to delete config: \(error.localizedDescription)"
        }
    }

    // MARK: - Status Refresh
    private func refreshStatuses() async {
        await fetchSchedulerStatus()
        await fetchCoordinatorStatus()
    }

    private func fetchSchedulerStatus() async {
        struct Empty: Codable {}
        struct Status: Codable { let running: Bool; let active_config_id: String?; let next_wake_time: Int?; let message: String }
        guard let rpc = bridgeManager.connection?.rpcClient else { return }
        rpc.sendJSONRPC(method: ACPRPC.orchestrateSchedulerStatus, params: Empty(), id: "sched-status-\(UUID().uuidString)") { (resp: Status?) in
            guard let r = resp else { return }
            schedulerRunning = r.running
            schedulerNextWake = r.next_wake_time
            schedulerMessage = r.message
            activeConfigId = r.active_config_id
        }
    }

    private func fetchCoordinatorStatus() async {
        struct Empty: Codable {}
        struct Coord: Codable { let cycles_run: Int; let tasks_executed: Int; let tasks_completed: Int; let tasks_failed: Int; let tasks_cancelled: Int; let last_cycle_ts: Int64? }
        guard let rpc = bridgeManager.connection?.rpcClient else { return }
        rpc.sendJSONRPC(method: ACPRPC.orchestrateCoordinatorStatus, params: Empty(), id: "coord-status-\(UUID().uuidString)") { (resp: Coord?) in
            guard let r = resp else { return }
            lastCoordinatorStatus = (r.cycles_run, r.tasks_executed, r.tasks_completed, r.tasks_failed, r.tasks_cancelled, r.last_cycle_ts)
        }
    }

    // MARK: - Actions
    private func reloadScheduler() async {
        struct Empty: Codable {}
        struct Reload: Codable { let success: Bool; let message: String }
        guard let rpc = bridgeManager.connection?.rpcClient else { return }
        rpc.sendJSONRPC(method: ACPRPC.orchestrateSchedulerReload, params: Empty(), id: "sched-reload-\(UUID().uuidString)") { (_: Reload?) in
            Task { await refreshStatuses() }
        }
    }

    private func runNow() async {
        struct Empty: Codable {}
        struct RunNow: Codable { let started: Bool; let message: String; let session_id: String? }
        guard let rpc = bridgeManager.connection?.rpcClient else { return }
        rpc.sendJSONRPC(method: ACPRPC.orchestrateSchedulerRunNow, params: Empty(), id: "sched-run-\(UUID().uuidString)") { (_: RunNow?) in
            Task { await refreshStatuses() }
        }
    }

    // MARK: - RPC actions (Coordinator/Scheduler)
    private func activateConfig(_ config: OrchestrationConfig) async {
        struct BindParams: Codable { let config_id: String; let workspace_root: String }
        guard let rpc = bridgeManager.connection?.rpcClient else { return }
        rpc.sendJSONRPC(method: ACPRPC.orchestrateSchedulerBind, params: BindParams(config_id: config.id, workspace_root: config.workspaceRoot), id: "bind-\(UUID().uuidString)") { (_: [String: AnyCodable]?) in }
    }

    private func runPlan(_ config: OrchestrationConfig) async {
        struct RunParams: Codable { let config_id: String?; let config_inline: OrchestrationConfig? }
        guard let rpc = bridgeManager.connection?.rpcClient else { return }
        rpc.sendJSONRPC(method: ACPRPC.orchestrateCoordinatorRunOnce, params: RunParams(config_id: config.id, config_inline: nil), id: "run-once-\(UUID().uuidString)") { (_: [String: AnyCodable]?) in }
    }
}

// MARK: - Config Row View

private struct ConfigRowView: View {
    let config: OrchestrationConfig
    let isSelected: Bool
    let onSelect: () -> Void
    let onEdit: () -> Void
    let onDelete: () async -> Void
    let onActivate: () async -> Void
    let onRunPlan: () async -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            // Header row
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text(config.id)
                        .font(OAFonts.ui(.headline, 15))
                        .foregroundStyle(OATheme.Colors.textPrimary)

                    Text(config.workspaceRoot)
                        .font(OAFonts.mono(.caption, 12))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                }

                Spacer()

                HStack(spacing: 8) {
                    Button {
                        Task { await onActivate() }
                    } label: {
                        Label("Activate", systemImage: "bolt.horizontal.circle.fill")
                            .font(OAFonts.ui(.footnote, 12))
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(OATheme.Colors.accent)

                    Button {
                        Task { await onRunPlan() }
                    } label: {
                        Label("Run Plan", systemImage: "play.circle.fill")
                            .font(OAFonts.ui(.footnote, 12))
                    }
                    .buttonStyle(.bordered)
                    Button {
                        onEdit()
                    } label: {
                        Image(systemName: "pencil.circle.fill")
                            .font(.system(size: 20))
                            .foregroundStyle(OATheme.Colors.accent)
                    }
                    .buttonStyle(.plain)

                    Button {
                        Task {
                            await onDelete()
                        }
                    } label: {
                        Image(systemName: "trash.circle.fill")
                            .font(.system(size: 20))
                            .foregroundStyle(Color.red)
                    }
                    .buttonStyle(.plain)
                }
            }

            // Goals
            if !config.goals.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Goals")
                        .font(OAFonts.ui(.caption, 11))
                        .foregroundStyle(OATheme.Colors.textSecondary)

                    FlowLayout(spacing: 6) {
                        ForEach(config.goals.prefix(5), id: \.self) { goal in
                            Text(goal)
                                .font(OAFonts.ui(.caption, 12))
                                .foregroundStyle(OATheme.Colors.textPrimary)
                                .padding(.horizontal, 8)
                                .padding(.vertical, 4)
                                .background(
                                    Capsule()
                                        .fill(OATheme.Colors.border.opacity(0.3))
                                )
                        }

                        if config.goals.count > 5 {
                            Text("+\(config.goals.count - 5) more")
                                .font(OAFonts.ui(.caption, 11))
                                .foregroundStyle(OATheme.Colors.textSecondary)
                                .padding(.horizontal, 8)
                                .padding(.vertical, 4)
                        }
                    }
                }
            }

            // Schedule info
            HStack(spacing: 16) {
                Label {
                    Text(config.schedule.expression)
                        .font(OAFonts.mono(.caption, 12))
                } icon: {
                    Image(systemName: "clock")
                }
                .foregroundStyle(OATheme.Colors.textSecondary)

                Label {
                    Text("\(config.timeBudgetSec / 60)m")
                        .font(OAFonts.ui(.caption, 12))
                } icon: {
                    Image(systemName: "timer")
                }
                .foregroundStyle(OATheme.Colors.textSecondary)
            }
        }
        .padding(16)
        .background(
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .fill(isSelected ? OATheme.Colors.border.opacity(0.2) : OATheme.Colors.card)
                .overlay(
                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                        .stroke(isSelected ? OATheme.Colors.accent : OATheme.Colors.border, lineWidth: 1)
                )
        )
        .contentShape(Rectangle())
        .onTapGesture {
            onSelect()
        }
    }
}

// MARK: - Flow Layout Helper

private struct FlowLayout: Layout {
    let spacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? 0
        var height: CGFloat = 0
        var lineHeight: CGFloat = 0
        var currentX: CGFloat = 0

        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)

            if currentX + size.width > width && currentX > 0 {
                height += lineHeight + spacing
                currentX = 0
                lineHeight = 0
            }

            currentX += size.width + spacing
            lineHeight = max(lineHeight, size.height)
        }

        height += lineHeight
        return CGSize(width: width, height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var currentX = bounds.minX
        var currentY = bounds.minY
        var lineHeight: CGFloat = 0

        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)

            if currentX + size.width > bounds.maxX && currentX > bounds.minX {
                currentY += lineHeight + spacing
                currentX = bounds.minX
                lineHeight = 0
            }

            subview.place(at: CGPoint(x: currentX, y: currentY), proposal: .unspecified)
            currentX += size.width + spacing
            lineHeight = max(lineHeight, size.height)
        }
    }
}

#endif
