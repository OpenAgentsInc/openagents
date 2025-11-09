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

        // TODO(mac chat state): wire server notifications -> timeline when
        // DesktopConnectionManager exposes notificationPublisher.

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

        // TODO(mac chat state): initialize dispatcher with LocalJsonRpcClient when
        // DesktopConnectionManager exposes a local rpcClient compatible with JSONRPCSending.
        dispatcher = nil
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
