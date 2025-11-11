import Foundation
import OpenAgentsCore
import Combine
import os.log

#if os(macOS)
/// ViewModel for orchestration sidebar status
/// Tracks scheduler state, next run time, and recent cycles
@MainActor
public class OrchestrationViewModel: ObservableObject {
    // MARK: - Published State

    @Published public var isEnabled: Bool = false
    @Published public var schedulerState: SchedulerService.State = .idle
    @Published public var nextRunTime: Date?
    @Published public var recentCycles: [OrchestrationCycle] = []
    @Published public var currentConfig: OrchestrationConfig?

    // MARK: - Cycle Model

    public struct OrchestrationCycle: Identifiable {
        public let id: String
        public let startTime: Date
        public let endTime: Date?
        public let status: Status
        public let configId: String

        public enum Status: String {
            case running = "Running"
            case completed = "Completed"
            case failed = "Failed"
            case skipped = "Skipped"
        }

        public var duration: TimeInterval? {
            guard let end = endTime else { return nil }
            return end.timeIntervalSince(startTime)
        }
    }

    // MARK: - Dependencies

    private let scheduler: SchedulerService
    private var rpc: JSONRPCSending?
    private var cancellables = Set<AnyCancellable>()
    private var pollCancellable: AnyCancellable?

    // MARK: - Initialization

    public init(scheduler: SchedulerService = SchedulerService()) {
        self.scheduler = scheduler
        startMonitoring()
    }

    /// Bind a JSON-RPC client to pull scheduler/coordinator status from the Desktop server
    /// Access level intentionally internal to avoid exposing internal protocol type
    func setRPC(_ rpc: JSONRPCSending?) {
        self.rpc = rpc
        // Start polling remote status immediately
        startStatePolling()
        Task { await self.fetchRemoteSchedulerStatus() }
    }

    // MARK: - Public Methods

    /// Load orchestration configuration from file
    public func loadConfig(from path: String) {
        do {
            let loader = ConfigLoader()
            let config = try loader.load(from: path)
            self.currentConfig = config
            OpenAgentsLog.orchestration.info("Loaded config: \(config.id)")
        } catch {
            OpenAgentsLog.orchestration.error("Failed to load config: \(error)")
        }
    }

    /// Start orchestration with current config
    public func startOrchestration(trigger: @escaping () async -> Void) async {
        guard let config = currentConfig else {
            OpenAgentsLog.orchestration.warning("Cannot start: no config loaded")
            return
        }

        await scheduler.configure(config: config, trigger: trigger)
        await scheduler.start()
        isEnabled = true

        // Start polling for state updates
        startStatePolling()
    }

    /// Stop orchestration
    public func stopOrchestration() async {
        await scheduler.stop()
        isEnabled = false
    }

    /// Get current metrics
    public func refreshMetrics() async {
        let metrics = await scheduler.metrics()

        if let cycleCount = metrics["cycle_count"] as? Int {
            // Refresh recent cycles from metrics
            // In a real implementation, this would fetch from persistent storage
            OpenAgentsLog.orchestration.debug("Cycle count: \(cycleCount)")
        }
    }

    // MARK: - Private Methods

    private func startMonitoring() {
        // Visible runtime activity: listen for orchestration.cycle.* notifications
        NotificationCenter.default.publisher(for: .init("orchestration.cycle.started"))
            .sink { [weak self] notification in
                guard let self else { return }
                Task { @MainActor in
                    let userInfo = notification.userInfo ?? [:]
                    let cid = (userInfo["cycle_id"] as? String) ?? UUID().uuidString
                    let start = (userInfo["start"] as? Date) ?? Date()
                    let cfg = (userInfo["config_id"] as? String) ?? "(unknown)"
                    let cycle = OrchestrationCycle(id: cid, startTime: start, endTime: nil, status: .running, configId: cfg)
                    self.addCycle(cycle)
                    self.schedulerState = .running(nextWake: self.nextRunTime)
                }
            }
            .store(in: &cancellables)

        NotificationCenter.default.publisher(for: .init("orchestration.cycle.completed"))
            .sink { [weak self] notification in
                guard let self else { return }
                Task { @MainActor in
                    let userInfo = notification.userInfo ?? [:]
                    guard let cid = userInfo["cycle_id"] as? String else { return }
                    let end = (userInfo["end"] as? Date) ?? Date()
                    let statusStr = (userInfo["status"] as? String) ?? "completed"
                    let status: OrchestrationCycle.Status = {
                        switch statusStr.lowercased() {
                        case "failed": return .failed
                        case "skipped": return .skipped
                        default: return .completed
                        }
                    }()
                    if let idx = self.recentCycles.firstIndex(where: { $0.id == cid }) {
                        let existing = self.recentCycles[idx]
                        let updated = OrchestrationCycle(id: existing.id, startTime: existing.startTime, endTime: end, status: status, configId: existing.configId)
                        self.recentCycles[idx] = updated
                    } else {
                        // If we missed the start event, append a completed cycle
                        let cfg = (userInfo["config_id"] as? String) ?? "(unknown)"
                        let fallback = OrchestrationCycle(id: cid, startTime: Date(timeIntervalSinceNow: -1), endTime: end, status: status, configId: cfg)
                        self.addCycle(fallback)
                    }
                }
            }
            .store(in: &cancellables)
    }

    private func startStatePolling() {
        // Cancel any existing poller to avoid duplicate timers
        pollCancellable?.cancel()
        pollCancellable = Timer.publish(every: 5.0, on: .main, in: .common)
            .autoconnect()
            .sink { [weak self] _ in
                guard let self else { return }
                Task { await self.fetchRemoteSchedulerStatus() }
            }
    }

    private func fetchRemoteSchedulerStatus() async {
        struct Empty: Codable {}
        struct Status: Codable { let running: Bool; let active_config_id: String?; let next_wake_time: Int?; let message: String }
        guard let rpc = self.rpc else { return }
        // Fetch scheduler status
        rpc.sendJSONRPC(method: ACPRPC.orchestrateSchedulerStatus, params: Empty(), id: "sidebar-sched-status-\(UUID().uuidString)") { (resp: Status?) in
            guard let r = resp else { return }
            Task { @MainActor in
                if r.running {
                    let next = r.next_wake_time.map { Date(timeIntervalSince1970: TimeInterval($0)) }
                    self.schedulerState = .running(nextWake: next)
                    self.nextRunTime = next
                    self.isEnabled = true
                } else {
                    self.schedulerState = .idle
                    self.isEnabled = false
                }
            }
        }
    }

    private func addCycle(_ cycle: OrchestrationCycle) {
        recentCycles.insert(cycle, at: 0)

        // Keep only last 10 cycles
        if recentCycles.count > 10 {
            recentCycles = Array(recentCycles.prefix(10))
        }
    }
}
#endif
