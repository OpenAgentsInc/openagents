import Carbon
import Foundation

/// Native macOS local command sandbox & Computer Use safety probes (#14147).
/// Pairs with the #authority policy in coax/lib/agent/desktop/browser-policy.coax.
enum MacSandbox {
    enum Mode: Int {
        case unsandboxed = 0
        case seatbelt = 1
        case microVM = 2
    }

    /// Hard Computer Use safety invariant #1: refuse automation whenever macOS Secure Event Input
    /// is active (password fields, 1Password, Keychain dialogs, sudo prompts).
    static var isSecureInputActive: Bool {
        IsSecureEventInputEnabled()
    }

    /// Hard Computer Use safety invariant #2: detect recent human keyboard activity within `windowSec`.
    static func isHumanTyping(within windowSec: TimeInterval = 0.75) -> Bool {
        let idle = CGEventSource.secondsSinceLastEventType(.combinedSessionState, eventType: .keyDown)
        return idle < windowSec
    }

    /// Returns true if `/usr/bin/sandbox-exec` is available on this macOS host.
    static var hasSeatbelt: Bool {
        FileManager.default.isExecutableFile(atPath: "/usr/bin/sandbox-exec")
    }

    /// Validates a workspace root for Seatbelt confinement (mirrors mac_sandbox_validate_root).
    static func isSafeWorkspaceRoot(_ path: String) -> Bool {
        guard path.count >= 2, path.count <= 512, path.hasPrefix("/") else { return false }
        let blocked: Set<String> = ["/", "/System", "/Library", "/Users"]
        if blocked.contains(path) || path.contains("..") { return false }
        for scalar in path.unicodeScalars {
            let v = scalar.value
            if v < 0x20 || v == 0x7f || scalar == "\"" || scalar == "\\" || scalar == "(" || scalar == ")" {
                return false
            }
        }
        return true
    }

    /// Generates the Seatbelt workspace-write profile confining writes to `workspaceRoot` + `/tmp`
    /// and blocking reads of `~/.ssh`, `~/.aws`, and `~/.gnupg`.
    static func seatbeltProfile(workspaceRoot: String, allowNetwork: Bool) -> String? {
        guard isSafeWorkspaceRoot(workspaceRoot) else { return nil }
        var lines = [
            "(version 1)",
            "(deny default)",
            "(allow process*)",
            "(allow sysctl-read)",
            "(allow file-read*)",
            #"(deny file-read* (regex #"/\.ssh(/|$)") (regex #"/\.aws(/|$)") (regex #"/\.gnupg(/|$)"))"#,
            "(allow file-write* (subpath \"\(workspaceRoot)\") (subpath \"/tmp\") (subpath \"/private/tmp\"))",
        ]
        if allowNetwork { lines.append("(allow network*)") }
        return lines.joined(separator: "\n") + "\n"
    }

    /// Wraps a command + arguments with `/usr/bin/sandbox-exec -p <profile>` when Seatbelt is active.
    static func wrapCommand(_ executable: String, args: [String], workspaceRoot: String, allowNetwork: Bool) -> (executable: String, args: [String]) {
        guard hasSeatbelt, let profile = seatbeltProfile(workspaceRoot: workspaceRoot, allowNetwork: allowNetwork) else {
            return (executable, args)
        }
        return ("/usr/bin/sandbox-exec", ["-p", profile, executable] + args)
    }
}
