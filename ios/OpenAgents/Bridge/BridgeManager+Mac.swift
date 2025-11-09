import Foundation
import Combine
import OpenAgentsCore

#if os(macOS)
extension BridgeManager {
    private static let workingDirectoryKey = "oa.bridge.working_directory"

    func start() {
        loadWorkingDirectory()
        let conn = DesktopConnectionManager()
        conn.workingDirectoryURL = workingDirectory
        connection = conn

        // Connection events
        conn.statusPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] st in self?.status = st }
            .store(in: &subscriptions)
        conn.logPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] line in self?.log("conn", line) }
            .store(in: &subscriptions)
        conn.connectedClientCountPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] count in self?.connectedClientCount = count }
            .store(in: &subscriptions)

        // Forward session/update notifications into TimelineStore
        conn.notificationPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] evt in
                guard let self = self else { return }
                if evt.method == ACPRPC.sessionUpdate {
                    self.timeline.applySessionUpdatePayload(evt.payload)
                } else if let s = String(data: evt.payload, encoding: .utf8) {
                    self.log("client", "notify \(evt.method): \(s)")
                }
            }
            .store(in: &subscriptions)

        // Mirror timeline state to published fields
        timeline.updatesPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.updates = $0 }
            .store(in: &subscriptions)
        timeline.availableCommandsPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.availableCommands = $0 }
            .store(in: &subscriptions)
        timeline.currentModePublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.currentMode = $0 }
            .store(in: &subscriptions)
        timeline.toolCallNamesPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.toolCallNames = $0 }
            .store(in: &subscriptions)
        timeline.rawJSONByCallIdPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.rawJSONByCallId = $0 }
            .store(in: &subscriptions)
        timeline.outputJSONByCallIdPublisher
            .receive(on: RunLoop.main)
            .sink { [weak self] in self?.outputJSONByCallId = $0 }
            .store(in: &subscriptions)

        // Initialize dispatcher with local RPC client
        dispatcher = PromptDispatcher(rpc: conn.rpcClient, timeline: timeline)
        conn.start()
    }

    func stop() { connection?.stop(); connection = nil }

    func setWorkingDirectory(_ url: URL) {
        workingDirectory = url
        saveWorkingDirectory(url)
        (connection as? DesktopConnectionManager)?.workingDirectoryURL = url
    }

    func loadWorkingDirectory() {
        if let path = UserDefaults.standard.string(forKey: Self.workingDirectoryKey) {
            let url = URL(fileURLWithPath: path)
            if FileManager.default.fileExists(atPath: path) {
                workingDirectory = url
                log("workdir", "Loaded working directory: \(path)")
            }
        }
    }

    private func saveWorkingDirectory(_ url: URL) {
        UserDefaults.standard.set(url.path, forKey: Self.workingDirectoryKey)
        log("workdir", "Saved working directory: \(url.path)")
    }

    // Published properties are defined in BridgeManager
}
#endif
