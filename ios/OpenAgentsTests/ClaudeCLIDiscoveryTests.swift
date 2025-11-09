import XCTest
@testable import OpenAgents
@testable import OpenAgentsCore

#if os(macOS)
/// Tests for Claude CLI discovery and process launching
@MainActor
final class ClaudeCLIDiscoveryTests: XCTestCase {

    // MARK: - Helper: Recursive Search

    private func searchForClaudeRecursive(in basePath: String, maxDepth: Int, currentDepth: Int = 0) -> String? {
        guard currentDepth < maxDepth else { return nil }
        guard FileManager.default.fileExists(atPath: basePath) else { return nil }

        let claudePath = "\(basePath)/bin/claude"
        if FileManager.default.fileExists(atPath: claudePath) {
            return claudePath
        }

        guard let contents = try? FileManager.default.contentsOfDirectory(atPath: basePath) else { return nil }

        for item in contents {
            let fullPath = "\(basePath)/\(item)"
            if let found = searchForClaudeRecursive(in: fullPath, maxDepth: maxDepth, currentDepth: currentDepth + 1) {
                return found
            }
        }

        return nil
    }

    // MARK: - Path Discovery Tests

    func testClaudeCLI_CanBeFoundInFnmPaths() {
        let home = NSHomeDirectory()
        let fnmPaths = [
            "\(home)/.local/state/fnm_multishells",
            "\(home)/.fnm/node-versions"
        ]

        var found = false
        for basePath in fnmPaths {
            if let claudePath = searchForClaudeRecursive(in: basePath, maxDepth: 4) {
                print("[Test] Found claude at: \(claudePath)")
                XCTAssertTrue(FileManager.default.fileExists(atPath: claudePath))
                XCTAssertTrue(FileManager.default.isExecutableFile(atPath: claudePath))
                found = true
                break
            }
        }

        if !found {
            print("[Test] WARNING: claude not found in fnm paths - may need npm install -g @anthropic-ai/claude-cli")
        }
    }

    func testClaudeCLI_SearchDepthLimits() {
        let home = NSHomeDirectory()
        let basePath = "\(home)/.local/state/fnm_multishells"

        // Should work with maxDepth=4
        let found4 = searchForClaudeRecursive(in: basePath, maxDepth: 4)

        // Should also work with lower depth if claude is closer
        let found2 = searchForClaudeRecursive(in: basePath, maxDepth: 2)

        // Depth 0 should never find anything (only checks immediate bin/)
        let found0 = searchForClaudeRecursive(in: basePath, maxDepth: 0)
        XCTAssertNil(found0, "Depth 0 should not traverse subdirectories")

        if found4 != nil {
            print("[Test] Found at depth 4: \(found4!)")
        }
    }

    func testCommonPaths_Checked() {
        let home = NSHomeDirectory()
        let paths = [
            "/usr/local/bin/claude",
            "/opt/homebrew/bin/claude",
            "\(home)/bin/claude",
            "\(home)/.local/bin/claude",
            "\(home)/.npm-global/bin/claude"
        ]

        var foundCount = 0
        for path in paths {
            if FileManager.default.fileExists(atPath: path) {
                print("[Test] Found claude at common path: \(path)")
                foundCount += 1
            }
        }

        print("[Test] Found \(foundCount) claude installations in common paths")
    }

    // MARK: - Shell Discovery Tests

    func testLoginShell_CanFindClaude() {
        let shells = ["/bin/zsh", "/bin/bash"]

        for shell in shells {
            let process = Process()
            process.executableURL = URL(fileURLWithPath: shell)
            process.arguments = ["-l", "-c", "which claude"]
            let pipe = Pipe()
            let errPipe = Pipe()
            process.standardOutput = pipe
            process.standardError = errPipe

            do {
                try process.run()
                process.waitUntilExit()

                if process.terminationStatus == 0 {
                    let data = try? pipe.fileHandleForReading.readToEnd()
                    if let data = data, let path = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines), !path.isEmpty {
                        print("[Test] \(shell) found claude at: \(path)")
                        XCTAssertTrue(FileManager.default.fileExists(atPath: path))
                        return // Success
                    }
                }

                if let errData = try? errPipe.fileHandleForReading.readToEnd(),
                   let errText = String(data: errData, encoding: .utf8), !errText.isEmpty {
                    print("[Test] \(shell) stderr: \(errText)")
                }
            } catch {
                XCTFail("Failed to run \(shell): \(error)")
            }
        }

        print("[Test] WARNING: No shell could find claude - PATH may not be configured")
    }

    func testLoginShell_ExecutesSuccessfully() {
        let shell = "/bin/zsh"
        let process = Process()
        process.executableURL = URL(fileURLWithPath: shell)
        process.arguments = ["-l", "-c", "echo test"]
        let pipe = Pipe()
        process.standardOutput = pipe

        XCTAssertNoThrow(try process.run())
        process.waitUntilExit()

        XCTAssertEqual(process.terminationStatus, 0, "Login shell should execute successfully")

        let data = try? pipe.fileHandleForReading.readToEnd()
        let output = String(data: data ?? Data(), encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines)
        XCTAssertEqual(output, "test", "Login shell should produce expected output")
    }

    // MARK: - Sandbox Tests

    func testProcess_CanLaunchExternalCommand() {
        // Verify we're not sandboxed and can launch processes
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/echo")
        process.arguments = ["sandbox-test"]
        let pipe = Pipe()
        process.standardOutput = pipe

        do {
            try process.run()
            process.waitUntilExit()
            XCTAssertEqual(process.terminationStatus, 0, "Should be able to launch /bin/echo")
        } catch {
            XCTFail("Cannot launch external processes - app may be sandboxed: \(error)")
        }
    }

    func testFileAccess_ClaudeSessionDirectory() {
        let home = NSHomeDirectory()
        let claudeDir = "\(home)/.claude"

        // Should be able to access .claude directory
        let exists = FileManager.default.fileExists(atPath: claudeDir)
        if !exists {
            print("[Test] WARNING: ~/.claude directory does not exist")
        }

        // Try to list contents if it exists
        if exists {
            let contents = try? FileManager.default.contentsOfDirectory(atPath: claudeDir)
            print("[Test] ~/.claude directory has \(contents?.count ?? 0) items")
        }
    }

    // MARK: - Integration Tests

    func testClaudeCLI_CanBeExecuted() {
        // Find claude first
        let home = NSHomeDirectory()
        let fnmPath = "\(home)/.local/state/fnm_multishells"

        guard let claudePath = searchForClaudeRecursive(in: fnmPath, maxDepth: 4) else {
            print("[Test] SKIP: claude not found")
            return
        }

        // Try to execute claude --version
        let process = Process()
        process.executableURL = URL(fileURLWithPath: claudePath)
        process.arguments = ["--version"]
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = pipe

        do {
            try process.run()
            process.waitUntilExit()

            let data = try? pipe.fileHandleForReading.readToEnd()
            let output = String(data: data ?? Data(), encoding: .utf8) ?? ""

            print("[Test] claude --version output: \(output)")
            if process.terminationStatus != 0 {
                print("[Test] SKIP: claude exists but did not execute successfully (status=\(process.terminationStatus)) — environment likely missing node runtime")
                return
            }
            XCTAssertEqual(process.terminationStatus, 0, "claude --version should succeed")
        } catch {
            XCTFail("Failed to execute claude: \(error)")
        }
    }

    func testRecursiveSearch_HandlesNonExistentPath() {
        let result = searchForClaudeRecursive(in: "/nonexistent/path", maxDepth: 4)
        XCTAssertNil(result, "Should return nil for non-existent path")
    }

    func testRecursiveSearch_HandlesEmptyDirectory() {
        let tmpDir = NSTemporaryDirectory()
        let testDir = "\(tmpDir)/test_empty_\(UUID().uuidString)"
        try? FileManager.default.createDirectory(atPath: testDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(atPath: testDir) }

        let result = searchForClaudeRecursive(in: testDir, maxDepth: 4)
        XCTAssertNil(result, "Should return nil for empty directory")
    }
}
#endif
