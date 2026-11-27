// Firm reason: 6 (~/.ssh/config + Boats discovery, SSH socket forwarding, and Cloud connection preferences over page/environments.coax; GitHub #13883).
import AppKit
import Foundation

/// Manages an SSH local UNIX socket tunnel to a remote `coder-host` daemon.
final class RemoteEnvironmentTunnel {
    static let shared = RemoteEnvironmentTunnel()

    private let queue = DispatchQueue(label: "openagents.coder.ssh-tunnel", qos: .userInitiated)
    private var process: Process?
    private(set) var activeAlias: String?
    private(set) var localSocketPath: String?
    private(set) var remoteFolders: [String: String] = [:]

    func setRemoteFolder(_ folder: String, for alias: String) {
        let trimmed = folder.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty {
            remoteFolders.removeValue(forKey: alias)
        } else {
            remoteFolders[alias] = trimmed
        }
    }

    func remoteFolder(for alias: String) -> String {
        remoteFolders[alias] ?? "~"
    }

    func connect(alias: String, completion: @escaping (Bool, String?) -> Void) {
        queue.async { [weak self] in
            guard let self else { return }
            self.disconnectLocked()

            let home = FileManager.default.homeDirectoryForCurrentUser
            let tunnelsDir = home.appendingPathComponent(".openagents/desktop/tunnels", isDirectory: true)
            try? FileManager.default.createDirectory(at: tunnelsDir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
            let safeAlias = alias.map { ($0.isLetter || $0.isNumber || $0 == "-" || $0 == "_") ? $0 : "_" }
            let sockURL = tunnelsDir.appendingPathComponent(String(safeAlias) + ".sock")
            try? FileManager.default.removeItem(at: sockURL)

            // Resolve remote home socket path (~/.openagents/coder-host/service.sock) via SSH stream forward.
            let p = Process()
            let errPipe = Pipe()
            p.executableURL = URL(fileURLWithPath: "/usr/bin/ssh")
            p.arguments = [
                "-N", "-T",
                "-o", "BatchMode=yes",
                "-o", "ExitOnForwardFailure=yes",
                "-o", "ConnectTimeout=8",
                "-o", "ServerAliveInterval=30",
                "-L", "\(sockURL.path):~/.openagents/coder-host/service.sock",
                alias
            ]
            p.standardInput = FileHandle.nullDevice
            p.standardOutput = FileHandle.nullDevice
            p.standardError = errPipe

            do {
                try p.run()
            } catch {
                DispatchQueue.main.async { completion(false, error.localizedDescription) }
                return
            }

            self.process = p
            self.activeAlias = alias
            self.localSocketPath = sockURL.path

            // Wait briefly to see if ssh exits immediately with an auth or host error.
            let deadline = Date().addingTimeInterval(1.2)
            while p.isRunning && Date() < deadline {
                if FileManager.default.fileExists(atPath: sockURL.path) { break }
                Thread.sleep(forTimeInterval: 0.08)
            }

            if !p.isRunning {
                let errData = errPipe.fileHandleForReading.readDataToEndOfFile()
                let errStr = String(decoding: errData, as: UTF8.self)
                    .split(whereSeparator: \.isNewline)
                    .last
                    .map(String.init) ?? "SSH exited with status \(p.terminationStatus)"
                self.disconnectLocked()
                DispatchQueue.main.async { completion(false, errStr) }
                return
            }

            DispatchQueue.main.async { completion(true, nil) }
        }
    }

    func disconnect() {
        queue.async { [weak self] in self?.disconnectLocked() }
    }

    private func disconnectLocked() {
        if let p = process, p.isRunning {
            p.terminate()
        }
        process = nil
        if let sock = localSocketPath {
            try? FileManager.default.removeItem(atPath: sock)
        }
        localSocketPath = nil
        activeAlias = nil
    }
}

/// Paired mobile device entry loaded from the resident daemon's host directory.
struct AuthorizedMobileDevice: Codable {
    var id: String
    var name: String
    var detail: String
}

enum AuthorizedDeviceStore {
    private static var revokedURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/desktop/revoked-devices.json")
    }

    static func loadRevoked() -> Set<String> {
        guard let data = try? Data(contentsOf: revokedURL),
              let ids = try? JSONDecoder().decode([String].self, from: data) else {
            return []
        }
        return Set(ids)
    }

    static func revoke(id: String) {
        var ids = loadRevoked()
        ids.insert(id)
        let dir = revokedURL.deletingLastPathComponent()
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        if let data = try? JSONEncoder().encode(Array(ids).sorted()) {
            try? data.write(to: revokedURL, options: .atomic)
        }
        // Best-effort notify bundled coder-host if it supports device revocation.
        if let host = Bundle.main.path(forResource: "coder-host", ofType: nil) {
            DispatchQueue.global(qos: .utility).async {
                runHelper(host, ["revoke", id], timeout: 4)
            }
        }
    }

    private static func runHelper(_ path: String, _ args: [String], timeout: TimeInterval) {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: path)
        p.arguments = args
        p.standardOutput = FileHandle.nullDevice
        p.standardError = FileHandle.nullDevice
        p.standardInput = FileHandle.nullDevice
        var env = ProcessInfo.processInfo.environment
        env["BROWSER"] = "/usr/bin/true"
        env["SSH_CONNECTION"] = "1"
        p.environment = env
        do { try p.run() } catch { return }
        let killer = DispatchWorkItem { if p.isRunning { p.terminate() } }
        DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + timeout, execute: killer)
        p.waitUntilExit()
        killer.cancel()
    }

    static func loadDevices(daemonClient: CoderDaemonClient) -> [AuthorizedMobileDevice] {
        let revoked = loadRevoked()
        let hostDir = URL(fileURLWithPath: daemonClient.hostDirPath, isDirectory: true)
        var out: [AuthorizedMobileDevice] = []
        for filename in ["devices.json", "grants.json", "peers.json"] {
            let url = hostDir.appendingPathComponent(filename)
            guard let data = try? Data(contentsOf: url),
                  let raw = try? JSONSerialization.jsonObject(with: data) else { continue }
            let items: [[String: Any]]
            if let arr = raw as? [[String: Any]] {
                items = arr
            } else if let dict = raw as? [String: Any], let arr = (dict["devices"] ?? dict["grants"] ?? dict["peers"]) as? [[String: Any]] {
                items = arr
            } else {
                continue
            }
            for item in items {
                let id = (item["id"] as? String) ?? (item["pubkey"] as? String) ?? (item["npub"] as? String) ?? ""
                guard !id.isEmpty, !revoked.contains(id) else { continue }
                let name = (item["name"] as? String) ?? (item["label"] as? String) ?? "Paired device"
                let detail = (item["detail"] as? String) ?? (item["transport"] as? String) ?? "Authorized mobile device"
                if !out.contains(where: { $0.id == id }) {
                    out.append(AuthorizedMobileDevice(id: id, name: name, detail: detail))
                }
            }
        }
        return out
    }
}

enum ServerExposurePrefs {
    static let modeKey = "serverExposureMode"

    /// 0 = Local-only, 1 = Cloud (default). Legacy values (> 1) map to Cloud.
    static var mode: Int32 {
        get {
            if let v = UserDefaults.standard.object(forKey: modeKey) as? Int {
                return v == 0 ? 0 : 1
            }
            return 1
        }
        set {
            let clamped = newValue == 0 ? 0 : 1
            UserDefaults.standard.set(clamped, forKey: modeKey)
        }
    }
}

extension CoderWindow {
    /// Syncs ~/.ssh/config hosts, Boats, Server Exposure status, authorized mobile devices, and Keep-Awake state.
    func syncEnvironments() {
        let activeBefore = envNameAt(cd_env_active(page))
        cd_env_clear_remote(page)

        // 1. Parse ~/.ssh/config into Coax environment catalog.
        let sshConfigURL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".ssh/config")
        if let text = try? String(contentsOf: sshConfigURL, encoding: .utf8) {
            _ = cd_env_load_ssh_config(page, text)
        }

        // 2. Restore active selection & tunnel connection state if still present.
        if !activeBefore.isEmpty && activeBefore != "Local" {
            let count = cd_env_count(page)
            for i in 0..<count {
                if envNameAt(i) == activeBefore {
                    _ = cd_env_select(page, i)
                    if RemoteEnvironmentTunnel.shared.activeAlias == activeBefore {
                        _ = cd_env_set_state(page, i, 3, "") // CP_ENV_ST_CONNECTED
                    }
                    break
                }
            }
        }

        // 3. Sync Connection mode (Local-only vs Cloud).
        let mode = ServerExposurePrefs.mode
        cd_env_expose_set(page, mode, exposureStatus(mode))

        // 4. Sync authorized mobile devices.
        cd_env_device_clear(page)
        for dev in AuthorizedDeviceStore.loadDevices(daemonClient: daemonClient) {
            _ = cd_env_device_add(page, dev.id, dev.name, dev.detail)
        }

        // 5. Sync Keep-Awake preference & live IOKit assertion state.
        cd_env_keep_awake_set(page, KeepAwakeManager.enabled ? 1 : 0)
        cd_env_awake_active_set(page, KeepAwakeManager.shared.isActive ? 1 : 0)
        syncKeepAwake()
        wake()
    }

    private func exposureStatus(_ mode: Int32) -> String {
        if mode == 0 {
            return "Local-only (127.0.0.1) — cloud sync disabled."
        }
        if FeatureFlags.isOn(.account) && MacAccount.shared.loggedIn {
            let acct = MacAccount.shared.title.isEmpty ? "Signed in" : MacAccount.shared.title
            return "Cloud — connected via OpenAgents Cloud (\(acct))."
        }
        return "Cloud — sign in to OpenAgents Cloud in Settings -> Account."
    }

    private func envNameAt(_ idx: Int32) -> String {
        var buf = [CChar](repeating: 0, count: 128)
        let n = cd_env_name(page, idx, &buf, Int32(buf.count))
        return n > 0 ? String(cString: buf) : ""
    }

    /// Updates macOS `IOPMAssertion` Keep-Awake state based on active foreground/background runs and `/goal`.
    func syncKeepAwake() {
        let bgRunning = runs.values.filter { cd_running($0.page) != 0 }.count
        let needed = cd_env_should_keep_awake(page, Int32(bgRunning)) != 0
        let wasActive = KeepAwakeManager.shared.isActive
        KeepAwakeManager.shared.update(active: needed)
        let nowActive = KeepAwakeManager.shared.isActive
        if wasActive != nowActive {
            cd_env_awake_active_set(page, nowActive ? 1 : 0)
            wake()
        }
    }

    /// Handles CP_ASK_ENV_SELECT (80): switches active environment and connects SSH tunnel if remote.
    func envSelectChanged() {
        let idx = cd_env_active(page)
        let kind = cd_env_kind(page, idx)
        if kind == 0 { // CP_ENV_LOCAL
            RemoteEnvironmentTunnel.shared.disconnect()
            cd_toast(page, "Switched to Local environment", 0)
            wake()
            return
        }
        let alias = envNameAt(idx)
        guard !alias.isEmpty else { return }
        connectRemoteEnvironment(idx: idx, alias: alias)
    }

    /// Handles CP_ASK_ENV_CONNECT (82): connects SSH tunnel or edits remote folder if already connected.
    func envConnectOrFolder() {
        let idx = cd_env_target(page)
        guard idx > 0 else { return }
        let alias = envNameAt(idx)
        guard !alias.isEmpty else { return }
        if RemoteEnvironmentTunnel.shared.activeAlias == alias {
            chooseRemoteFolder(alias: alias)
        } else {
            connectRemoteEnvironment(idx: idx, alias: alias)
        }
    }

    private func connectRemoteEnvironment(idx: Int32, alias: String) {
        _ = cd_env_set_state(page, idx, 2, "") // CP_ENV_ST_CONNECTING
        wake()
        RemoteEnvironmentTunnel.shared.connect(alias: alias) { [weak self] ok, err in
            guard let self else { return }
            if ok {
                _ = cd_env_set_state(self.page, idx, 3, "") // CP_ENV_ST_CONNECTED
                cd_toast(self.page, "Connected to \(alias)", 0)
            } else {
                _ = cd_env_set_state(self.page, idx, 4, err ?? "Connection failed") // CP_ENV_ST_ERROR
                cd_toast(self.page, "SSH \(alias): \(err ?? "Connection failed")", 1)
            }
            self.wake()
        }
    }

    private func chooseRemoteFolder(alias: String) {
        var d = CoderDialog(size: .md, title: "Remote Working Folder", body: "Enter the working directory on \(alias) where chats and worktrees should run.")
        d.fields = [.init(label: "Remote path", value: RemoteEnvironmentTunnel.shared.remoteFolder(for: alias), hint: "~/openagents")]
        d.buttons = [.init(title: "Cancel"), .init(title: "Save", kind: .primary)]
        d.cancel = 0; d.confirm = 1
        present(d) { [weak self] answer, values in
            guard let self else { return }
            if answer == 1, let path = values.first {
                RemoteEnvironmentTunnel.shared.setRemoteFolder(path, for: alias)
                cd_toast(self.page, "Remote folder for \(alias) set to \(path)", 0)
            }
            self.refocus()
        }
    }

    /// Handles CP_ASK_ENV_EXPOSE (93): persists Cloud / Local-only connection mode.
    func envExposureChanged() {
        let mode = cd_env_expose_mode(page)
        ServerExposurePrefs.mode = mode
        syncEnvironments()
    }

    /// Handles CP_ASK_ENV_PAIR (94): explains OpenAgents Cloud linking for OpenAgents iOS.
    func envPairMobileDevice() {
        if FeatureFlags.isOn(.account) && MacAccount.shared.loggedIn {
            cd_env_pair_code_set(page, "Cloud synced")
            wake()
            present(.notice("OpenAgents Cloud Sync", "Your desktop connects through OpenAgents Cloud. Sign in with the same OpenAgents account in OpenAgents iOS to access your chats from your phone."))
            return
        }
        cd_env_pair_code_set(page, "Sign in required")
        wake()
        var d = CoderDialog(size: .md, title: "Sign In to OpenAgents Cloud", body: "OpenAgents connects Desktop and iOS through OpenAgents Cloud. Sign in to your OpenAgents account to sync across devices.")
        d.buttons = [.init(title: "Cancel"), .init(title: "Log In…", kind: .primary)]
        d.cancel = 0; d.confirm = 1
        present(d) { [weak self] answer, _ in
            if answer == 1 {
                Task { @MainActor in await MacAccount.shared.showLogin() }
            }
            self?.refocus()
        }
    }

    /// Handles CP_ASK_ENV_REVOKE (85): revokes an authorized mobile device.
    func envRevokeDevice() {
        let idx = cd_env_device_target(page)
        guard idx >= 0 else { return }
        var idBuf = [CChar](repeating: 0, count: 128)
        guard cd_env_device_id(page, idx, &idBuf, Int32(idBuf.count)) > 0 else { return }
        let devID = String(cString: idBuf)
        AuthorizedDeviceStore.revoke(id: devID)
        _ = cd_env_device_revoke(page, idx)
        cd_toast(page, "Revoked mobile device", 0)
        wake()
    }

    /// Handles CP_ASK_ENV_KEEP_AWAKE (86): persists Keep-Awake preference and immediately updates IOKit assertion.
    func envKeepAwakeChanged() {
        let on = cd_env_keep_awake(page) != 0
        KeepAwakeManager.enabled = on
        syncKeepAwake()
        wake()
    }

    /// Releases Keep-Awake assertion and tears down any SSH tunnel on app quit.
    func retireEnvironments() {
        RemoteEnvironmentTunnel.shared.disconnect()
        KeepAwakeManager.shared.release()
    }
}
