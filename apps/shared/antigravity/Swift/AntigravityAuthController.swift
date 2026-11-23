// Firm reason: 6 (cross-Apple Antigravity CLI/ACP authentication controller delegating exclusively to `agyworker --probe-json`, `--connect`, and `--logout` with 0600 secret storage and zero direct Google API calls; stubbed on iOS).
import Foundation
#if os(macOS)
import AppKit
import CryptoKit
#endif

enum AntigravityAuthMethod: String, CaseIterable, Identifiable, Codable {
    case oauthPersonal = "oauth-personal"
    case geminiAPIKey = "gemini-api-key"
    case oauthBusiness = "oauth-business"
    case agentPlatform = "agent-platform"

    var id: String { rawValue }

    var label: String {
        switch self {
        case .oauthPersonal: return "Personal Google Account"
        case .geminiAPIKey: return "Gemini API Key"
        case .oauthBusiness: return "Enterprise / Workspace"
        case .agentPlatform: return "Vertex / Agent Platform"
        }
    }

    var subtitle: String {
        switch self {
        case .oauthPersonal: return "Signs in via `agy` and syncs chats with Google Antigravity"
        case .geminiAPIKey: return "Uses an isolated profile with your Gemini API key (0600 local store)"
        case .oauthBusiness: return "Google Workspace / Code Assist business account (isolated profile)"
        case .agentPlatform: return "Google Cloud Agent Platform / Vertex credentials (isolated profile)"
        }
    }

    /// Personal OAuth defaults to native `~/.gemini` + `antigravity` so chats sync into Google's Antigravity app.
    /// API-key / Enterprise / Vertex methods always enforce a strict isolated `--gemini-dir` profile (#13995).
    var usesIsolatedProfile: Bool {
        self != .oauthPersonal
    }
}

struct AntigravityProbeResult: Equatable {
    let installed: Bool
    let binaryPath: String
    let version: String
    let authStatus: String
    let authMethod: String
    let authLabel: String
    let status: String
    let message: String
    let modelCount: Int

    var isAuthenticated: Bool {
        authStatus == "authenticated"
    }

    static func parse(_ data: Data) -> AntigravityProbeResult? {
        guard let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            return nil
        }
        let models = obj["models"] as? [[String: Any]] ?? []
        return AntigravityProbeResult(
            installed: obj["installed"] as? Bool ?? false,
            binaryPath: obj["binaryPath"] as? String ?? "",
            version: obj["version"] as? String ?? "",
            authStatus: obj["authStatus"] as? String ?? "unknown",
            authMethod: obj["authMethod"] as? String ?? AntigravityAuthMethod.oauthPersonal.rawValue,
            authLabel: obj["authLabel"] as? String ?? "",
            status: obj["status"] as? String ?? "warning",
            message: obj["message"] as? String ?? "",
            modelCount: models.count
        )
    }
}

struct AntigravityAuthError: Error {
    let message: String
}

@MainActor
final class AntigravityAuthController: ObservableObject {
    @Published var selectedMethod: AntigravityAuthMethod = .oauthPersonal
    @Published var probeResult: AntigravityProbeResult?
    @Published var isProbing: Bool = false
    @Published var isAuthenticating: Bool = false
    @Published var pendingAuthURL: String?
    @Published var awaitingManualCode: Bool = false
    @Published var statusMessage: String = ""
    @Published var errorMessage: String?

    #if os(macOS)
    private var connectProcess: Process?
    #endif

    init() {
        #if os(macOS)
        if let saved = Self.readSavedAuthMethod() {
            selectedMethod = saved
        }
        #endif
    }

    var isAuthenticated: Bool {
        probeResult?.isAuthenticated == true
    }

    var summaryLabel: String {
        if isProbing { return "Checking Antigravity sign-in…" }
        guard let res = probeResult else { return "Not probed yet" }
        if !res.installed { return "Antigravity CLI (`agy`) not installed" }
        if res.isAuthenticated {
            let ver = res.version.isEmpty ? "" : " v\(res.version)"
            let models = res.modelCount > 0 ? " · \(res.modelCount) models" : ""
            return "Signed in (\(res.authLabel.isEmpty ? selectedMethod.label : res.authLabel))\(ver)\(models)"
        }
        if !res.message.isEmpty { return res.message }
        return "Sign-in required"
    }

    func selectMethod(_ method: AntigravityAuthMethod) {
        selectedMethod = method
        errorMessage = nil
        #if os(macOS)
        Self.saveAuthMethod(method)
        #endif
    }

    func probe() async {
        #if os(macOS)
        guard !isProbing else { return }
        isProbing = true
        errorMessage = nil
        defer { isProbing = false }

        let method = selectedMethod
        let result = await Task.detached(priority: .userInitiated) {
            Self.runProbeSync(method: method)
        }.value

        switch result {
        case .success(let parsed):
            probeResult = parsed
            if parsed.isAuthenticated {
                statusMessage = summaryLabel
                pendingAuthURL = nil
                awaitingManualCode = false
            } else if !parsed.message.isEmpty {
                statusMessage = parsed.message
            }
        case .failure(let err):
            errorMessage = err.message
            statusMessage = err.message
        }
        #endif
    }

    func saveAPIKeyAndProbe(_ apiKey: String) async {
        #if os(macOS)
        let trimmed = apiKey.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            errorMessage = "Enter a valid Gemini API key."
            return
        }
        selectedMethod = .geminiAPIKey
        Self.saveAuthMethod(.geminiAPIKey)
        guard Self.writeSecretFile(trimmed, to: Self.apiKeySecretURL) else {
            errorMessage = "Could not store API key securely (0600)."
            return
        }
        statusMessage = "Saved Gemini API key (0600). Verifying with `agy`…"
        await probe()
        #endif
    }

    func startOAuthConnect() {
        #if os(macOS)
        cancelConnect()
        guard let worker = Self.findAgyWorkerBinary() else {
            errorMessage = "agyworker binary not found. Run `apps/openagents-macos/install.sh` to bundle it."
            return
        }
        isAuthenticating = true
        errorMessage = nil
        pendingAuthURL = nil
        awaitingManualCode = false
        statusMessage = "Starting Antigravity sign-in…"

        let method = selectedMethod
        Self.saveAuthMethod(method)

        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: worker)
        proc.arguments = Self.workerBaseArgs(method: method, extra: ["--connect"])
        proc.environment = Self.workerEnvironment(method: method)
        let outPipe = Pipe()
        let errPipe = Pipe()
        proc.standardOutput = outPipe
        proc.standardError = errPipe

        let handle = outPipe.fileHandleForReading
        var lineBuffer = Data()
        handle.readabilityHandler = { [weak self] fh in
            let chunk = fh.availableData
            if chunk.isEmpty {
                fh.readabilityHandler = nil
                return
            }
            lineBuffer.append(chunk)
            while let nl = lineBuffer.firstIndex(of: 0x0A) {
                let line = Data(lineBuffer[lineBuffer.startIndex..<nl])
                lineBuffer.removeSubrange(lineBuffer.startIndex...nl)
                Task { @MainActor [weak self] in
                    self?.handleConnectJSONLine(line)
                }
            }
        }

        proc.terminationHandler = { [weak self] p in
            handle.readabilityHandler = nil
            let errText = String(data: errPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)?
                .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            Task { @MainActor [weak self] in
                guard let self else { return }
                if self.connectProcess === p {
                    self.connectProcess = nil
                }
                self.isAuthenticating = false
                if p.terminationStatus == 0 {
                    self.pendingAuthURL = nil
                    self.awaitingManualCode = false
                    await self.probe()
                } else if self.errorMessage == nil && !errText.isEmpty {
                    self.errorMessage = errText
                }
            }
        }

        do {
            try proc.run()
            connectProcess = proc
        } catch {
            isAuthenticating = false
            errorMessage = "Could not start agyworker --connect: \(error.localizedDescription)"
        }
        #endif
    }

    func submitManualAuthCode(_ code: String) async {
        #if os(macOS)
        let trimmed = code.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            errorMessage = "Paste the authorization code or callback URL from your browser."
            return
        }
        guard let worker = Self.findAgyWorkerBinary() else {
            errorMessage = "agyworker binary not found."
            return
        }
        cancelConnect()
        isAuthenticating = true
        errorMessage = nil
        statusMessage = "Completing Antigravity sign-in…"

        let method = selectedMethod
        let outcome: Result<Void, AntigravityAuthError> = await Task.detached(priority: .userInitiated) {
            let tmpDir = FileManager.default.temporaryDirectory
            let codeURL = tmpDir.appendingPathComponent("oa-agy-code-\(UUID().uuidString).secret")
            defer { try? FileManager.default.removeItem(at: codeURL) }
            guard Self.writeSecretFile(trimmed, to: codeURL) else {
                return .failure(AntigravityAuthError(message: "Could not create temporary 0600 auth code file."))
            }
            let args = Self.workerBaseArgs(method: method, extra: ["--connect", "--code", codeURL.path])
            let env = Self.workerEnvironment(method: method)
            let res = Self.runSync(executable: worker, args: args, env: env, timeoutSeconds: 45)
            guard res.exitCode == 0 else {
                let msg = res.stdout.isEmpty ? res.stderr : res.stdout
                return .failure(AntigravityAuthError(message: msg.isEmpty ? "Sign-in code was rejected by Antigravity." : msg))
            }
            return .success(())
        }.value

        isAuthenticating = false
        switch outcome {
        case .success:
            pendingAuthURL = nil
            awaitingManualCode = false
            await probe()
        case .failure(let err):
            errorMessage = err.message
        }
        #endif
    }

    func signOut() async {
        #if os(macOS)
        cancelConnect()
        isAuthenticating = true
        errorMessage = nil
        statusMessage = "Signing out of Antigravity…"
        defer { isAuthenticating = false }

        let method = selectedMethod
        if method == .geminiAPIKey {
            try? FileManager.default.removeItem(at: Self.apiKeySecretURL)
        }
        if let worker = Self.findAgyWorkerBinary() {
            let args = Self.workerBaseArgs(method: method, extra: ["--logout"])
            let env = Self.workerEnvironment(method: method)
            _ = await Task.detached(priority: .userInitiated) {
                Self.runSync(executable: worker, args: args, env: env, timeoutSeconds: 20)
            }.value
        }
        pendingAuthURL = nil
        awaitingManualCode = false
        await probe()
        #endif
    }

    func cancelConnect() {
        #if os(macOS)
        if let proc = connectProcess, proc.isRunning {
            proc.terminate()
        }
        connectProcess = nil
        isAuthenticating = false
        #endif
    }

    #if os(macOS)
    private func handleConnectJSONLine(_ line: Data) {
        guard let obj = try? JSONSerialization.jsonObject(with: line) as? [String: Any] else { return }
        if let urlStr = (obj["url"] as? String) ?? (obj["authUrl"] as? String), !urlStr.isEmpty {
            pendingAuthURL = urlStr
            awaitingManualCode = true
            statusMessage = "Complete Google sign-in in your browser (or paste the callback code below)."
            if let url = URL(string: urlStr) {
                NSWorkspace.shared.open(url)
            }
        }
        if let err = obj["error"] as? String, !err.isEmpty {
            errorMessage = err
        }
        if let msg = obj["message"] as? String, !msg.isEmpty {
            statusMessage = msg
        }
        if (obj["ok"] as? Bool) == true || (obj["status"] as? String) == "authenticated" {
            pendingAuthURL = nil
            awaitingManualCode = false
        }
    }

    // MARK: - Secure storage & process helpers (macOS only)

    nonisolated static var providerRootURL: URL {
        let home = FileManager.default.homeDirectoryForCurrentUser
        return home.appendingPathComponent(".openagents/coder/providers/antigravity", isDirectory: true)
    }

    nonisolated static var apiKeySecretURL: URL {
        providerRootURL.appendingPathComponent("api-key.secret")
    }

    nonisolated static var authMethodURL: URL {
        providerRootURL.appendingPathComponent("auth-method")
    }

    nonisolated static func isolatedProfilePath(for method: AntigravityAuthMethod) -> String {
        let home = FileManager.default.homeDirectoryForCurrentUser
        let digest = SHA256.hash(data: Data("antigravity:\(method.rawValue)".utf8))
        let hex = digest.prefix(8).map { String(format: "%02x", $0) }.joined()
        return home.appendingPathComponent(".openagents/coder/profiles/antigravity-\(hex)", isDirectory: true).path
    }

    nonisolated static func readSavedAuthMethod() -> AntigravityAuthMethod? {
        guard let raw = try? String(contentsOf: authMethodURL, encoding: .utf8) else { return nil }
        return AntigravityAuthMethod(rawValue: raw.trimmingCharacters(in: .whitespacesAndNewlines))
    }

    nonisolated static func saveAuthMethod(_ method: AntigravityAuthMethod) {
        _ = writeSecretFile(method.rawValue + "\n", to: authMethodURL)
    }

    nonisolated static func writeSecretFile(_ content: String, to url: URL) -> Bool {
        let fm = FileManager.default
        let dir = url.deletingLastPathComponent()
        try? fm.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let tmp = url.appendingPathExtension("tmp.\(UUID().uuidString)")
        defer { try? fm.removeItem(at: tmp) }
        guard let data = content.data(using: .utf8),
              fm.createFile(atPath: tmp.path, contents: data, attributes: [.posixPermissions: 0o600]) else {
            return false
        }
        return rename(tmp.path, url.path) == 0
    }

    nonisolated static func findAgyWorkerBinary() -> String? {
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        var extra: [String] = []
        if let res = Bundle.main.resourceURL {
            extra.append(res.appendingPathComponent("agyworker").path)
        }
        extra.append(contentsOf: [
            "\(home)/.openagents/bin/agyworker",
            "/Applications/OpenAgentsMac.app/Contents/Resources/agyworker",
            "/Applications/OpenAgents.app/Contents/Resources/agyworker",
            "/opt/homebrew/bin/agyworker",
            "/usr/local/bin/agyworker",
        ])
        let fm = FileManager.default
        for p in extra where fm.isExecutableFile(atPath: p) { return p }
        let pathEnv = ProcessInfo.processInfo.environment["PATH"] ?? "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"
        for dir in pathEnv.split(separator: ":") {
            let candidate = "\(dir)/agyworker"
            if fm.isExecutableFile(atPath: candidate) { return candidate }
        }
        return nil
    }

    /// Resolves `agy` checking managed runtime `active.json` first, then standard paths (#13995, #14001).
    nonisolated static func findAgyBinary() -> String? {
        let fm = FileManager.default
        let home = fm.homeDirectoryForCurrentUser.path
        let activeJSON = "\(home)/.openagents/runtimes/agy/active.json"
        if let data = try? Data(contentsOf: URL(fileURLWithPath: activeJSON)),
           let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let bin = obj["binaryPath"] as? String ?? obj["path"] as? String,
           fm.isExecutableFile(atPath: bin) {
            return bin
        }
        let extra = [
            "/opt/homebrew/bin/agy",
            "/usr/local/bin/agy",
            "\(home)/.local/bin/agy",
        ]
        for p in extra where fm.isExecutableFile(atPath: p) { return p }
        let pathEnv = ProcessInfo.processInfo.environment["PATH"] ?? "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"
        for dir in pathEnv.split(separator: ":") {
            for name in ["agy", "antigravity"] {
                let candidate = "\(dir)/\(name)"
                if fm.isExecutableFile(atPath: candidate) { return candidate }
            }
        }
        return nil
    }

    nonisolated private static func workerBaseArgs(method: AntigravityAuthMethod, extra: [String]) -> [String] {
        var args = extra
        if method.usesIsolatedProfile {
            args.append(contentsOf: ["--profile", isolatedProfilePath(for: method)])
        }
        return args
    }

    nonisolated private static func workerEnvironment(method: AntigravityAuthMethod) -> [String: String] {
        var env = ProcessInfo.processInfo.environment
        let basePath = env["PATH"] ?? "/usr/bin:/bin"
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        env["PATH"] = "/opt/homebrew/bin:/usr/local/bin:\(home)/.local/bin:\(home)/.openagents/bin:\(basePath)"
        env["CODER_AGY_AUTH_METHOD"] = method.rawValue
        if let agy = findAgyBinary() {
            env["CODER_AGY_BIN"] = agy
        }
        if method == .oauthPersonal {
            // Preserve native Google Antigravity App Sync for personal OAuth sessions
            env["CODER_AGY_APP_DATA_DIR"] = "antigravity"
        } else {
            env["CODER_AGY_PROFILE"] = isolatedProfilePath(for: method)
        }
        if method == .geminiAPIKey,
           let key = try? String(contentsOf: apiKeySecretURL, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines),
           !key.isEmpty {
            env["CODER_AGY_API_KEY"] = key
        }
        return env
    }

    nonisolated private static func runProbeSync(method: AntigravityAuthMethod) -> Result<AntigravityProbeResult, AntigravityAuthError> {
        guard let worker = findAgyWorkerBinary() else {
            let agy = findAgyBinary()
            return .success(AntigravityProbeResult(
                installed: agy != nil,
                binaryPath: agy ?? "",
                version: "",
                authStatus: "unknown",
                authMethod: method.rawValue,
                authLabel: method.label,
                status: "warning",
                message: "agyworker binary not found. Run `apps/openagents-macos/install.sh` to bundle it.",
                modelCount: 0
            ))
        }
        let args = workerBaseArgs(method: method, extra: ["--probe-json"])
        let env = workerEnvironment(method: method)
        let res = runSync(executable: worker, args: args, env: env, timeoutSeconds: 30)
        // Extract the JSON line even if non-JSON diagnostics preceded it (#13995)
        for rawLine in res.stdout.split(separator: "\n").reversed() {
            let trimmed = rawLine.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed.hasPrefix("{"), let data = trimmed.data(using: .utf8),
               let parsed = AntigravityProbeResult.parse(data) {
                return .success(parsed)
            }
        }
        let msg = res.stderr.isEmpty ? "Could not probe Antigravity status." : res.stderr
        return .failure(AntigravityAuthError(message: msg))
    }

    nonisolated private static func runSync(
        executable: String,
        args: [String],
        env: [String: String],
        timeoutSeconds: Int
    ) -> (exitCode: Int32, stdout: String, stderr: String) {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: executable)
        proc.arguments = args
        proc.environment = env
        let outPipe = Pipe()
        let errPipe = Pipe()
        proc.standardOutput = outPipe
        proc.standardError = errPipe

        let done = DispatchSemaphore(value: 0)
        proc.terminationHandler = { _ in done.signal() }
        do {
            try proc.run()
        } catch {
            return (-1, "", error.localizedDescription)
        }
        if done.wait(timeout: .now() + .seconds(max(2, timeoutSeconds))) == .timedOut {
            proc.terminate()
            _ = done.wait(timeout: .now() + .seconds(2))
            return (-2, "", "Command timed out after \(timeoutSeconds)s")
        }
        let outData = outPipe.fileHandleForReading.readDataToEndOfFile()
        let errData = errPipe.fileHandleForReading.readDataToEndOfFile()
        return (
            proc.terminationStatus,
            String(data: outData, encoding: .utf8) ?? "",
            String(data: errData, encoding: .utf8) ?? ""
        )
    }
    #endif
}
