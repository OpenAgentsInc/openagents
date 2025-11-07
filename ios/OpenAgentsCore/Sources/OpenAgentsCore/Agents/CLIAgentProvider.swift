#if os(macOS)
import Foundation

/// Base class for CLI-based agent providers.
/// Provides shared functionality for discovering binaries, launching processes,
/// and handling stdout/stderr streams.
open class CLIAgentProvider: AgentProvider, @unchecked Sendable {
    // MARK: - AgentProvider Requirements

    public let id: ACPSessionModeId
    public let displayName: String
    public let capabilities: AgentCapabilities

    // MARK: - CLI Configuration

    /// Binary name to search for (e.g., "claude", "codex")
    public let binaryName: String

    /// Environment variable for binary override (e.g., "OPENAGENTS_CODEX_CLI")
    public let envOverride: String?

    /// Additional search paths beyond standard locations
    public let additionalSearchPaths: [String]

    // MARK: - State (Process Management)

    /// Active processes by session ID
    private var activeProcesses: [String: Process] = [:]

    /// Stdout remainder buffers for JSONL parsing
    private var stdoutRemainders: [String: Data] = [:]

    // MARK: - Initialization

    public init(
        id: ACPSessionModeId,
        displayName: String,
        binaryName: String,
        envOverride: String? = nil,
        additionalSearchPaths: [String] = [],
        capabilities: AgentCapabilities? = nil
    ) {
        self.id = id
        self.displayName = displayName
        self.binaryName = binaryName
        self.envOverride = envOverride
        self.additionalSearchPaths = additionalSearchPaths
        self.capabilities = capabilities ?? AgentCapabilities(
            executionMode: .cli,
            streamingMode: .jsonl,
            supportsResume: true,
            supportsWorkingDirectory: true,
            requiresExternalBinary: true,
            supportsMCP: false
        )
    }

    // MARK: - AgentProvider Implementation

    public func isAvailable() async -> Bool {
        return findBinary() != nil
    }

    public func start(
        sessionId: ACPSessionId,
        prompt: String,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws -> AgentHandle {
        let sidStr = sessionId.value

        // Check for existing process
        if let existing = activeProcesses[sidStr], existing.isRunning {
            print("[\(displayName)] Session \(sidStr) already has running process")
            throw AgentProviderError.startFailed("Session already has a running process")
        }

        // Find binary
        guard let execPath = findBinary() else {
            throw AgentProviderError.binaryNotFound("\(displayName) CLI not found. Please install the \(binaryName) CLI.")
        }

        print("[\(displayName)] Launching (session=\(sidStr)): \(execPath)")

        // Create process
        let process = Process()
        process.executableURL = URL(fileURLWithPath: execPath)

        // Build arguments (delegated to subclass)
        process.arguments = buildStartArguments(
            sessionId: sessionId,
            prompt: prompt,
            context: context
        )

        // Set up environment
        var environment = ProcessInfo.processInfo.environment
        let binDir = (execPath as NSString).deletingLastPathComponent
        let fnmNodePath = (binDir as NSString).deletingLastPathComponent

        if let existingPath = environment["PATH"] {
            environment["PATH"] = "\(binDir):\(fnmNodePath):\(existingPath)"
        } else {
            environment["PATH"] = "\(binDir):\(fnmNodePath):/usr/local/bin:/usr/bin:/bin"
        }

        process.environment = environment

        // Set working directory
        if let wd = context.workingDirectory {
            process.currentDirectoryURL = wd
            print("[\(displayName)] Working directory: \(wd.path)")
        }

        // Set up pipes
        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe

        // Store process
        activeProcesses[sidStr] = process

        // Start process
        do {
            try process.run()
            print("[\(displayName)] Process started (pid=\(process.processIdentifier))")

            // Set up output handlers
            setupStdoutHandler(
                pipe: stdoutPipe,
                sessionId: sessionId,
                updateHub: updateHub
            )
            setupStderrHandler(
                pipe: stderrPipe,
                sessionId: sessionId,
                updateHub: updateHub
            )

            // Build handle
            let handle = AgentHandle(
                sessionId: sessionId,
                mode: id,
                processId: process.processIdentifier,
                threadId: nil,
                isStarted: true,
                metadata: [:]
            )

            return handle
        } catch {
            activeProcesses.removeValue(forKey: sidStr)
            throw AgentProviderError.startFailed("Failed to start process: \(error.localizedDescription)")
        }
    }

    public func resume(
        sessionId: ACPSessionId,
        prompt: String,
        handle: AgentHandle,
        context: AgentContext,
        updateHub: SessionUpdateHub
    ) async throws {
        // For CLI agents, resume typically means launching a new process with resume args
        // Subclasses can override for custom behavior
        let sidStr = sessionId.value

        guard let execPath = findBinary() else {
            throw AgentProviderError.binaryNotFound("\(displayName) CLI not found")
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: execPath)

        // Build resume arguments (delegated to subclass)
        process.arguments = buildResumeArguments(
            sessionId: sessionId,
            prompt: prompt,
            handle: handle,
            context: context
        )

        // Environment setup (same as start)
        var environment = ProcessInfo.processInfo.environment
        let binDir = (execPath as NSString).deletingLastPathComponent
        let fnmNodePath = (binDir as NSString).deletingLastPathComponent

        if let existingPath = environment["PATH"] {
            environment["PATH"] = "\(binDir):\(fnmNodePath):\(existingPath)"
        } else {
            environment["PATH"] = "\(binDir):\(fnmNodePath):/usr/local/bin:/usr/bin:/bin"
        }

        process.environment = environment

        if let wd = context.workingDirectory {
            process.currentDirectoryURL = wd
        }

        // Set up pipes
        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe

        // Store process
        activeProcesses[sidStr] = process

        do {
            try process.run()
            print("[\(displayName)] Process resumed (pid=\(process.processIdentifier))")

            setupStdoutHandler(pipe: stdoutPipe, sessionId: sessionId, updateHub: updateHub)
            setupStderrHandler(pipe: stderrPipe, sessionId: sessionId, updateHub: updateHub)
        } catch {
            activeProcesses.removeValue(forKey: sidStr)
            throw AgentProviderError.resumeFailed("Failed to resume: \(error.localizedDescription)")
        }
    }

    public func cancel(
        sessionId: ACPSessionId,
        handle: AgentHandle
    ) async {
        let sidStr = sessionId.value

        if let process = activeProcesses[sidStr] {
            if process.isRunning {
                process.terminate()
                print("[\(displayName)] Terminated process for session \(sidStr)")
            }
            activeProcesses.removeValue(forKey: sidStr)
        }

        stdoutRemainders.removeValue(forKey: sidStr)
    }

    // MARK: - Binary Discovery

    /// Find the CLI binary on the system
    /// - Returns: Path to binary, or nil if not found
    public func findBinary() -> String? {
        // 1. Check environment variable override
        if let envVar = envOverride,
           let override = ProcessInfo.processInfo.environment[envVar],
           !override.isEmpty,
           FileManager.default.fileExists(atPath: override) {
            print("[\(displayName)] Using override: \(override)")
            return override
        }

        // 2. Search fnm-managed paths
        let fnmPaths = [
            "\(NSHomeDirectory())/.local/state/fnm_multishells",
            "\(NSHomeDirectory())/.fnm/node-versions"
        ]

        for basePath in fnmPaths {
            if let found = searchForBinaryRecursive(in: basePath, maxDepth: 4) {
                print("[\(displayName)] Found in fnm: \(found)")
                return found
            }
        }

        // 3. Check common locations
        let commonPaths = [
            "/usr/local/bin/\(binaryName)",
            "/opt/homebrew/bin/\(binaryName)",
            "\(NSHomeDirectory())/bin/\(binaryName)",
            "\(NSHomeDirectory())/.local/bin/\(binaryName)",
            "\(NSHomeDirectory())/.npm-global/bin/\(binaryName)",
            "/usr/bin/\(binaryName)"
        ] + additionalSearchPaths

        for path in commonPaths {
            if FileManager.default.fileExists(atPath: path) {
                print("[\(displayName)] Found at: \(path)")
                return path
            }
        }

        // 4. Try shell `which` command
        let shells = ["/bin/zsh", "/bin/bash"]
        for shell in shells {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: shell)
            process.arguments = ["-l", "-c", "which \(binaryName)"]
            let pipe = Pipe()
            process.standardOutput = pipe
            process.standardError = Pipe()

            do {
                try process.run()
                process.waitUntilExit()

                if process.terminationStatus == 0,
                   let data = try? pipe.fileHandleForReading.readToEnd(),
                   let path = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines),
                   !path.isEmpty {
                    print("[\(displayName)] Found via \(shell): \(path)")
                    return path
                }
            } catch {
                continue
            }
        }

        print("[\(displayName)] Binary '\(binaryName)' not found")
        return nil
    }

    /// Recursively search for binary in a directory
    private func searchForBinaryRecursive(in basePath: String, maxDepth: Int) -> String? {
        guard maxDepth > 0,
              FileManager.default.fileExists(atPath: basePath) else {
            return nil
        }

        // Check if binary exists at this level
        let candidatePath = "\(basePath)/bin/\(binaryName)"
        if FileManager.default.fileExists(atPath: candidatePath) {
            return candidatePath
        }

        // Recursively search subdirectories
        guard let contents = try? FileManager.default.contentsOfDirectory(atPath: basePath) else {
            return nil
        }

        for item in contents {
            let fullPath = "\(basePath)/\(item)"
            var isDirectory: ObjCBool = false
            if FileManager.default.fileExists(atPath: fullPath, isDirectory: &isDirectory),
               isDirectory.boolValue {
                if let found = searchForBinaryRecursive(in: fullPath, maxDepth: maxDepth - 1) {
                    return found
                }
            }
        }

        return nil
    }

    // MARK: - Subclass Extension Points

    /// Build command-line arguments for starting a new session
    /// Subclasses MUST override this to provide agent-specific arguments
    open func buildStartArguments(
        sessionId: ACPSessionId,
        prompt: String,
        context: AgentContext
    ) -> [String] {
        fatalError("Subclasses must implement buildStartArguments")
    }

    /// Build command-line arguments for resuming a session
    /// Subclasses MUST override this to provide agent-specific arguments
    open func buildResumeArguments(
        sessionId: ACPSessionId,
        prompt: String,
        handle: AgentHandle,
        context: AgentContext
    ) -> [String] {
        fatalError("Subclasses must implement buildResumeArguments")
    }

    /// Process a line of stdout (for JSONL-based agents)
    /// Subclasses can override to parse provider-specific output
    open func processStdoutLine(
        _ line: String,
        sessionId: ACPSessionId,
        updateHub: SessionUpdateHub
    ) async {
        // Default: no-op (subclasses override)
    }

    /// Process stderr output
    /// Subclasses can override for custom error handling
    open func processStderr(
        _ text: String,
        sessionId: ACPSessionId,
        updateHub: SessionUpdateHub
    ) async {
        // Default: send as error message to UI
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }

        // Skip common noise
        if trimmed.contains("stdout is not a terminal") { return }

        print("[\(displayName)] stderr: \(trimmed)")

        // Send to UI as error chunk
        let chunk = ACP.Client.ContentChunk(content: .text(.init(text: "❌ \(trimmed)")))
        await updateHub.sendSessionUpdate(
            sessionId: sessionId,
            update: .agentMessageChunk(chunk)
        )
    }

    // MARK: - Stream Handling

    private func setupStdoutHandler(
        pipe: Pipe,
        sessionId: ACPSessionId,
        updateHub: SessionUpdateHub
    ) {
        let sidStr = sessionId.value

        pipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            guard let self = self else { return }
            let data = handle.availableData
            guard !data.isEmpty else { return }

            // Buffer handling for JSONL parsing
            var remainder = self.stdoutRemainders[sidStr] ?? Data()
            remainder.append(data)

            guard var text = String(data: remainder, encoding: .utf8) else { return }

            var keep = Data()
            if !text.hasSuffix("\n"), let lastNewline = text.lastIndex(of: "\n") {
                let rem = String(text[text.index(after: lastNewline)..<text.endIndex])
                keep = Data(rem.utf8)
                text = String(text[..<text.index(after: lastNewline)])
            }

            self.stdoutRemainders[sidStr] = keep

            let lines = text.split(separator: "\n", omittingEmptySubsequences: true).map(String.init)
            for line in lines {
                Task {
                    await self.processStdoutLine(line, sessionId: sessionId, updateHub: updateHub)
                }
            }
        }
    }

    private func setupStderrHandler(
        pipe: Pipe,
        sessionId: ACPSessionId,
        updateHub: SessionUpdateHub
    ) {
        pipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            guard let self = self else { return }
            let data = handle.availableData
            guard !data.isEmpty, let text = String(data: data, encoding: .utf8) else { return }

            Task {
                await self.processStderr(text, sessionId: sessionId, updateHub: updateHub)
            }
        }
    }
}
#endif
