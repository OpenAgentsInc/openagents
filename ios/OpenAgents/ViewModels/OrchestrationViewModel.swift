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
    private var cancellables = Set<AnyCancellable>()

    // MARK: - Initialization

    public init(scheduler: SchedulerService = SchedulerService()) {
        self.scheduler = scheduler
        startMonitoring()
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
        // Monitor for session updates that might be orchestration-related
        NotificationCenter.default.publisher(for: .init("OrchestrationCycleStarted"))
            .sink { [weak self] notification in
                guard let self else { return }
                Task { @MainActor in
                    if let cycle = notification.object as? OrchestrationCycle {
                        self.addCycle(cycle)
                    }
                }
            }
            .store(in: &cancellables)
    }

    private func startStatePolling() {
        // Poll scheduler state every 5 seconds
        Timer.publish(every: 5.0, on: .main, in: .common)
            .autoconnect()
            .sink { [weak self] _ in
                guard let self else { return }
                Task {
                    await self.pollSchedulerState()
                }
            }
            .store(in: &cancellables)
    }

    private func pollSchedulerState() async {
        let state = await scheduler.status()
        await MainActor.run {
            self.schedulerState = state

            // Extract next wake time from running state
            if case .running(let nextWake) = state {
                self.nextRunTime = nextWake
            }
        }

        // Refresh metrics
        await refreshMetrics()
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
