import Foundation

/// A lightweight scheduler that triggers orchestration runs based on an OrchestrationConfig.Schedule.
/// Uses SchedulePreview to compute the next run time and sleeps until then.
/// The clock/compute policy is intentionally simple and testable.
public actor SchedulerService {
    public enum State: Equatable { case idle, running(nextWake: Date?), stopped }

    private var state: State = .idle
    private var task: Task<Void, Never>? = nil
    private var config: OrchestrationConfig? = nil
    private var trigger: (() async -> Void)? = nil

    public init() {}

    public func configure(config: OrchestrationConfig, trigger: @escaping () async -> Void) {
        self.config = config
        self.trigger = trigger
    }

    public func start() {
        guard task == nil, let cfg = config, let trigger = trigger else { return }
        task = Task { [weak self] in
            guard let self else { return }
            while !Task.isCancelled {
                let next = SchedulerService.nextWake(for: cfg.schedule, from: Date())
                await self.setState(.running(nextWake: next))
                if let next {
                    let delay = max(0, next.timeIntervalSinceNow)
                    do { try await Task.sleep(nanoseconds: UInt64(delay * 1_000_000_000)) } catch { break }
                }
                if Task.isCancelled { break }
                await trigger()
            }
            await self.setState(.stopped)
        }
    }

    public func stop() {
        task?.cancel()
        task = nil
        state = .stopped
    }

    public func status() -> State { state }

    private func setState(_ s: State) { self.state = s }

    /// Compute the next wake based on schedule. Returns nil if cannot determine.
    public static func nextWake(for schedule: OrchestrationConfig.Schedule, from: Date) -> Date? {
        SchedulePreview.nextRuns(schedule: schedule, count: 1, from: from).first
    }
}

