// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Per-chat git worktree isolation, fast APFS CoW hydration, and shared build caches (GitHub #13758).
import Foundation
import CryptoKit

struct CoderWorktreeInfo {
    let path: String
    let branch: String
    let repoRoot: String
}

private struct CoderWorktreeRepoConfig: Decodable {
    var clone: [String]? = nil
    var symlink: [String]? = nil
}

enum CoderWorktree {
    /// Root directory where per-chat worktrees live: ~/.openagents/worktrees
    static var baseDir: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/worktrees", isDirectory: true)
    }

    /// Stable, human-readable slug for a repository root: `<name>-<8-hex>`.
    static func repoSlug(for repoRoot: String) -> String {
        let normalized = URL(fileURLWithPath: repoRoot).standardizedFileURL.path
        let baseName = URL(fileURLWithPath: normalized).lastPathComponent
        let cleanName = baseName.map { ch -> Character in
            (ch.isASCII && (ch.isLetter || ch.isNumber || ch == "-" || ch == "_")) ? ch : "-"
        }
        let digest = SHA256.hash(data: Data(normalized.utf8))
        let hashHex = digest.prefix(4).map { String(format: "%02x", $0) }.joined()
        let prefix = String(cleanName).trimmingCharacters(in: CharacterSet(charactersIn: "-"))
        return (prefix.isEmpty ? "repo" : prefix) + "-" + hashHex
    }

    /// Short branch suffix derived from a chat ID (e.g. first 8 hex/alnum chars).
    static func shortID(for chatID: String) -> String {
        let cleaned = chatID.lowercased().filter { $0.isASCII && ($0.isLetter || $0.isNumber) }
        let short = String(cleaned.prefix(8))
        return short.isEmpty ? "chat" : short
    }

    /// Returns true if `path` is inside `~/.openagents/worktrees/`.
    static func isManagedWorktree(_ path: String) -> Bool {
        let base = baseDir.standardizedFileURL.path
        let target = URL(fileURLWithPath: path).standardizedFileURL.path
        return target.hasPrefix(base + "/")
    }

    /// Resolves the primary git repository root for `folder`, or nil if `folder` is not inside a git repo.
    /// Works whether `folder` is a primary checkout or an existing linked worktree.
    static func primaryRepoRoot(for folder: URL) -> String? {
        guard let common = gitOutput(in: folder.path, args: ["rev-parse", "--path-format=absolute", "--git-common-dir"]) ??
              gitOutput(in: folder.path, args: ["rev-parse", "--git-common-dir"]) else {
            return nil
        }
        let trimmed = common.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        let commonURL: URL
        if trimmed.hasPrefix("/") {
            commonURL = URL(fileURLWithPath: trimmed).standardizedFileURL
        } else {
            commonURL = folder.appendingPathComponent(trimmed).standardizedFileURL
        }
        if commonURL.lastPathComponent == ".git" {
            return commonURL.deletingLastPathComponent().path
        }
        // Bare repo fallback: use show-toplevel if available
        if let top = gitOutput(in: folder.path, args: ["rev-parse", "--show-toplevel"])?.trimmingCharacters(in: .whitespacesAndNewlines), !top.isEmpty {
            return URL(fileURLWithPath: top).standardizedFileURL.path
        }
        return nil
    }

    /// Ensures an isolated git worktree exists for `chatID` in the git repo containing `folder`.
    /// Returns nil when `folder` is not a git repo or worktree creation fails.
    static func ensureWorktree(
        chatID: String,
        folder: URL,
        existingPath: String? = nil,
        existingBranch: String? = nil,
        existingRepoRoot: String? = nil
    ) -> CoderWorktreeInfo? {
        let fm = FileManager.default
        // 1. Reconnect to an existing persisted worktree if it still exists on disk.
        if let existingPath, !existingPath.isEmpty {
            let dotGit = URL(fileURLWithPath: existingPath).appendingPathComponent(".git").path
            if fm.fileExists(atPath: dotGit) {
                let root = existingRepoRoot ?? primaryRepoRoot(for: URL(fileURLWithPath: existingPath)) ?? folder.path
                let branch = existingBranch ?? CoderWindow.gitBranch(URL(fileURLWithPath: existingPath))
                return CoderWorktreeInfo(path: existingPath, branch: branch.isEmpty ? "coder/\(shortID(for: chatID))" : branch, repoRoot: root)
            }
        }

        // 2. Locate primary git repository root.
        guard let repoRoot = existingRepoRoot ?? primaryRepoRoot(for: folder) else { return nil }
        let slug = repoSlug(for: repoRoot)
        let wtURL = baseDir.appendingPathComponent(slug, isDirectory: true).appendingPathComponent(chatID, isDirectory: true)
        let branch = existingBranch ?? "coder/\(shortID(for: chatID))"

        // 3. If the worktree path already exists with a .git file, reuse it.
        if fm.fileExists(atPath: wtURL.appendingPathComponent(".git").path) {
            hydrate(worktree: wtURL, repoRoot: URL(fileURLWithPath: repoRoot))
            let actualBranch = CoderWindow.gitBranch(wtURL)
            return CoderWorktreeInfo(path: wtURL.path, branch: actualBranch.isEmpty ? branch : actualBranch, repoRoot: repoRoot)
        }

        try? fm.createDirectory(at: wtURL.deletingLastPathComponent(), withIntermediateDirectories: true)

        // 4. Pick base ref: origin/main if present, otherwise current HEAD (works offline).
        let baseRef: String
        if gitStatus(in: repoRoot, args: ["rev-parse", "--verify", "--quiet", "refs/remotes/origin/main"]) == 0 {
            baseRef = "origin/main"
        } else {
            baseRef = "HEAD"
        }

        // 5. Create the worktree and branch.
        var finalBranch = branch
        var rc = gitStatus(in: repoRoot, args: ["worktree", "add", "-b", finalBranch, wtURL.path, baseRef])
        if rc != 0 {
            // Branch may already exist: try attaching to it or using a unique suffix.
            if gitStatus(in: repoRoot, args: ["worktree", "add", wtURL.path, finalBranch]) == 0 {
                rc = 0
            } else {
                let suffix = String(UUID().uuidString.lowercased().prefix(4))
                finalBranch = "\(branch)-\(suffix)"
                rc = gitStatus(in: repoRoot, args: ["worktree", "add", "-b", finalBranch, wtURL.path, baseRef])
            }
        }
        guard rc == 0 else { return nil }

        // 6. Fast zero-rebuild hydration (APFS CoW clones + optional repo config).
        hydrate(worktree: wtURL, repoRoot: URL(fileURLWithPath: repoRoot))
        return CoderWorktreeInfo(path: wtURL.path, branch: finalBranch, repoRoot: repoRoot)
    }

    /// Hydrates a fresh worktree using O(1) APFS Copy-on-Write clones (`cp -cR`) and optional `.coder/` config.
    static func hydrate(worktree wtURL: URL, repoRoot: URL) {
        let fm = FileManager.default
        // Default fast CoW clones for Coax compiler & web tool builds
        let defaultClones = ["coax/bin", "coax/web/build"]
        for rel in defaultClones {
            cowCloneIfMissing(rel: rel, from: repoRoot, to: wtURL, fm: fm)
        }

        // Optional extensible repo config: .coder/worktree.json
        let configURL = repoRoot.appendingPathComponent(".coder/worktree.json")
        if let data = try? Data(contentsOf: configURL),
           let cfg = try? JSONDecoder().decode(CoderWorktreeRepoConfig.self, from: data) {
            for rel in cfg.clone ?? [] {
                cowCloneIfMissing(rel: rel, from: repoRoot, to: wtURL, fm: fm)
            }
            for rel in cfg.symlink ?? [] {
                symlinkIfMissing(rel: rel, from: repoRoot, to: wtURL, fm: fm)
            }
        }

        // Optional extensible repo hook: .coder/worktree-setup.sh
        let hookURL = repoRoot.appendingPathComponent(".coder/worktree-setup.sh")
        if fm.fileExists(atPath: hookURL.path) {
            let p = Process()
            p.executableURL = URL(fileURLWithPath: "/bin/sh")
            p.arguments = [hookURL.path, wtURL.path, repoRoot.path]
            p.currentDirectoryURL = wtURL
            p.standardInput = FileHandle.nullDevice
            p.standardOutput = FileHandle.nullDevice
            p.standardError = FileHandle.nullDevice
            try? p.run()
            p.waitUntilExit()
        }
    }

    private static func cowCloneIfMissing(rel: String, from repoRoot: URL, to wtURL: URL, fm: FileManager) {
        guard !rel.isEmpty, !rel.hasPrefix("/"), !rel.contains("..") else { return }
        let src = repoRoot.appendingPathComponent(rel)
        let dst = wtURL.appendingPathComponent(rel)
        guard fm.fileExists(atPath: src.path), !fm.fileExists(atPath: dst.path) else { return }
        try? fm.createDirectory(at: dst.deletingLastPathComponent(), withIntermediateDirectories: true)
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/bin/cp")
        p.arguments = ["-cR", src.path, dst.path]
        p.standardInput = FileHandle.nullDevice
        p.standardOutput = FileHandle.nullDevice
        p.standardError = FileHandle.nullDevice
        try? p.run()
        p.waitUntilExit()
    }

    private static func symlinkIfMissing(rel: String, from repoRoot: URL, to wtURL: URL, fm: FileManager) {
        guard !rel.isEmpty, !rel.hasPrefix("/"), !rel.contains("..") else { return }
        let src = repoRoot.appendingPathComponent(rel)
        let dst = wtURL.appendingPathComponent(rel)
        guard fm.fileExists(atPath: src.path), !fm.fileExists(atPath: dst.path) else { return }
        try? fm.createDirectory(at: dst.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? fm.createSymbolicLink(at: dst, withDestinationURL: src)
    }

    /// Returns true if the worktree has uncommitted or untracked changes.
    static func isDirty(_ worktreePath: String) -> Bool {
        guard let out = gitOutput(in: worktreePath, args: ["status", "--porcelain"]) else { return false }
        return !out.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    /// Removes a managed per-chat worktree and prunes git worktree metadata.
    @discardableResult
    static func removeWorktree(path: String, repoRoot: String?, force: Bool = false) -> Bool {
        guard isManagedWorktree(path) else { return false }
        let root = repoRoot ?? primaryRepoRoot(for: URL(fileURLWithPath: path))
        var ok = false
        if let root, !root.isEmpty {
            var args = ["worktree", "remove"]
            if force { args.append("--force") }
            args.append(path)
            ok = gitStatus(in: root, args: args) == 0
            if !ok && force {
                try? FileManager.default.removeItem(atPath: path)
                ok = true
            }
            _ = gitStatus(in: root, args: ["worktree", "prune"])
        } else if force {
            try? FileManager.default.removeItem(atPath: path)
            ok = true
        }
        // Clean up isolated CODER_BUILD dir if present
        let buildDir = coderBuildDir(forWorktreePath: path)
        try? FileManager.default.removeItem(at: buildDir)
        return ok
    }

    /// Isolated per-worktree build directory for `apps/coder-desktop/build.sh`.
    static func coderBuildDir(forWorktreePath path: String) -> URL {
        let name = URL(fileURLWithPath: path).lastPathComponent
        let cacheBase = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".cache/coder/builds", isDirectory: true)
        return cacheBase.appendingPathComponent(name.isEmpty ? "default" : name, isDirectory: true)
    }

    /// Shared build cache environment variables injected into worker processes (#13758).
    static func sharedCacheEnv(worktreePath: String?, repoRoot: String?) -> [String: String] {
        let home = FileManager.default.homeDirectoryForCurrentUser
        let inherited = ProcessInfo.processInfo.environment
        var env: [String: String] = [:]
        if inherited["COAX_TOOL_CACHE"] == nil {
            env["COAX_TOOL_CACHE"] = home.appendingPathComponent(".cache/coax-tools").path
        }
        if inherited["CARGO_TARGET_DIR"] == nil, let repoRoot, !repoRoot.isEmpty {
            let slug = repoSlug(for: repoRoot)
            env["CARGO_TARGET_DIR"] = home.appendingPathComponent(".cache/coder/cargo-target/\(slug)").path
        }
        if inherited["CODER_BUILD"] == nil, let worktreePath, !worktreePath.isEmpty {
            env["CODER_BUILD"] = coderBuildDir(forWorktreePath: worktreePath).path
        }
        return env
    }

    private static func gitStatus(in dir: String, args: [String]) -> Int32 {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        p.arguments = ["-C", dir] + args
        p.standardInput = FileHandle.nullDevice
        p.standardOutput = FileHandle.nullDevice
        p.standardError = FileHandle.nullDevice
        do {
            try p.run()
            p.waitUntilExit()
            return p.terminationStatus
        } catch {
            return -1
        }
    }

    private static func gitOutput(in dir: String, args: [String]) -> String? {
        let p = Process()
        let pipe = Pipe()
        p.executableURL = URL(fileURLWithPath: "/usr/bin/git")
        p.arguments = ["-C", dir] + args
        p.standardInput = FileHandle.nullDevice
        p.standardOutput = pipe
        p.standardError = FileHandle.nullDevice
        do {
            try p.run()
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            p.waitUntilExit()
            guard p.terminationStatus == 0 else { return nil }
            return String(data: data, encoding: .utf8)
        } catch {
            return nil
        }
    }
}
