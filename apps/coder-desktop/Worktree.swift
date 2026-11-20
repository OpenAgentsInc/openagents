// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Each chat's own worktree (GitHub #13758): where it is, its branch, making it quickly, and removing it only when clean.
import Foundation

/// A chat's own git worktree. A new chat started in a repository's main checkout runs in
/// ~/.openagents/worktrees/REPO/CHAT on the branch coder/CHAT, made from origin/main (HEAD when the
/// repository has no origin/main), so two chats never edit, stash or reset each other's files.
/// The folders a repository lists in .coder/worktree.json ("clone") are cloned in from the main
/// checkout with APFS copy-on-write (cp -c): the Coax compiler and built tools are there at once,
/// each worktree's copy its own the moment it is rebuilt. The workers share one tool cache and one
/// Cargo target folder (sharedCaches); apps/coder-desktop/build.sh builds into the worktree's own build/.
struct ChatWorktree: Codable, Equatable {
    var chat: String       // the chat's id, the worktree folder's name
    var path: String       // the worktree
    var source: String     // the main checkout it was made from
    var branch: String     // coder/CHAT

    /// Off with `defaults write com.openagents.coder chatWorktrees -bool NO`: new chats then run in the folder chosen.
    static var enabled: Bool { UserDefaults.standard.object(forKey: "chatWorktrees") as? Bool ?? true }

    static var home: String {
        let env = ProcessInfo.processInfo.environment["HOME"] ?? ""
        return env.hasPrefix("/") ? env : FileManager.default.homeDirectoryForCurrentUser.path
    }
    /// Where worktrees go: $CODER_WORKTREES, else ~/.openagents/worktrees.
    static var root: String {
        let set = ProcessInfo.processInfo.environment["CODER_WORKTREES"] ?? ""
        return set.hasPrefix("/") ? set : home + "/.openagents/worktrees"
    }
    /// The caches every worker shares, whatever folder it works in: compiled Coax tools
    /// (coax/web/tool.sh) and Rust builds. A value already in Coder's environment wins.
    static var sharedCaches: [String: String] {
        ["COAX_TOOL_CACHE": home + "/.cache/coax-tools", "CARGO_TARGET_DIR": home + "/.cache/coder/cargo-target"]
    }

    /// A repository's folder name in the worktrees folder: its last path part in lowercase letters,
    /// digits, dots and dashes, then 8 hex digits of its whole path, so two repositories with one
    /// name never share a folder.
    static func slug(_ repo: String) -> String {
        let name = (repo as NSString).lastPathComponent.lowercased()
        var out = ""
        for u in name.unicodeScalars {
            let ok = (u >= "a" && u <= "z") || (u >= "0" && u <= "9") || u == "." || u == "-" || u == "_"
            out.unicodeScalars.append(ok ? u : "-")
        }
        while out.hasPrefix(".") || out.hasPrefix("-") { out.removeFirst() }
        if out.isEmpty { out = "repo" }
        var hash: UInt64 = 0xcbf29ce484222325   // FNV-1a
        for b in repo.utf8 { hash = (hash ^ UInt64(b)) &* 0x100000001b3 }
        return String(out.prefix(48)) + "-" + String(String(hash, radix: 16).suffix(8)).leftPadded(8)
    }
    /// The chat id as a branch and folder name: lowercase letters, digits and dashes only.
    static func clean(_ chat: String) -> String {
        let kept = String(chat.lowercased().unicodeScalars.filter { ($0 >= "a" && $0 <= "z") || ($0 >= "0" && $0 <= "9") || $0 == "-" }.prefix(64))
        return kept.isEmpty ? "chat" : kept
    }
    static func branch(_ chat: String) -> String { "coder/" + clean(chat) }
    static func path(repo: String, chat: String, root: String = ChatWorktree.root) -> String { root + "/" + slug(repo) + "/" + clean(chat) }
    /// The branch as the composer shows it: coder/ and the chat id's first 8 characters.
    static func shown(_ branch: String) -> String {
        guard branch.hasPrefix("coder/"), branch.count > 14 else { return branch }
        return String(branch.prefix(14))
    }

    /// The main checkout a folder is in: walking up, the first .git found is a folder. Nil outside a
    /// repository, and for a folder in a worktree already (its .git is a file).
    static func mainCheckout(of folder: String) -> String? {
        var dir = (folder as NSString).standardizingPath
        let fm = FileManager.default
        for _ in 0..<256 {
            var isDir: ObjCBool = false
            if fm.fileExists(atPath: dir + "/.git", isDirectory: &isDir) { return isDir.boolValue ? dir : nil }
            if dir == "/" || dir.isEmpty { return nil }
            dir = (dir as NSString).deletingLastPathComponent
        }
        return nil
    }

    enum Failure: Error, Equatable { case git(String), notARepository }

    /// Makes chat's worktree of repo (or finds the one made before), then clones in what the
    /// repository lists. Runs git: call it off the main thread.
    static func create(repo: String, chat: String, root: String = ChatWorktree.root) -> Result<ChatWorktree, Failure> {
        guard mainCheckout(of: repo) == repo else { return .failure(.notARepository) }
        return make(ChatWorktree(chat: chat, path: path(repo: repo, chat: chat, root: root), source: repo, branch: branch(chat)))
    }
    /// Makes w where it says, on its branch (made from origin/main, else HEAD, when it isn't there
    /// yet), or finds it there. A kept chat's worktree comes back this way at the path and branch it had.
    static func make(_ w: ChatWorktree) -> Result<ChatWorktree, Failure> {
        let repo = w.source
        guard mainCheckout(of: repo) == repo else { return .failure(.notARepository) }
        let fm = FileManager.default
        if fm.fileExists(atPath: w.path + "/.git") { return .success(w) }   // made before: the chat goes on in it
        try? fm.createDirectory(atPath: (w.path as NSString).deletingLastPathComponent, withIntermediateDirectories: true)
        let base = git(["rev-parse", "--verify", "--quiet", "origin/main^{commit}"], in: repo).status == 0 ? "origin/main" : "HEAD"
        _ = git(["worktree", "prune"], in: repo)   // a folder removed by hand still holds its branch until pruned
        let hasBranch = git(["rev-parse", "--verify", "--quiet", "refs/heads/" + w.branch], in: repo).status == 0
        let made = hasBranch ? git(["worktree", "add", "--quiet", w.path, w.branch], in: repo)
                             : git(["worktree", "add", "--quiet", "--no-track", "-b", w.branch, w.path, base], in: repo)
        guard made.status == 0 else { return .failure(.git(made.output)) }
        hydrate(w)
        return .success(w)
    }

    /// The folders repo's .coder/worktree.json lists under "clone", each a path inside the
    /// repository: relative, no "..", no leading dot-dot or slash.
    static func clonePaths(repo: String) -> [String] {
        guard let data = FileManager.default.contents(atPath: repo + "/.coder/worktree.json"),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let list = obj["clone"] as? [String] else { return [] }
        return list.filter { p in
            !p.isEmpty && !p.hasPrefix("/") && !p.split(separator: "/").contains("..") && !p.contains("\0")
        }
    }
    /// Clones each listed folder from the main checkout into the worktree with APFS copy-on-write
    /// (cp -c: no bytes copied, and a change on either side stays on that side). A folder already
    /// in the worktree is left alone, and a symbolic link that points back into the main checkout
    /// is removed, so nothing in the worktree writes through to it.
    static func hydrate(_ w: ChatWorktree) {
        let fm = FileManager.default
        for rel in clonePaths(repo: w.source) {
            let from = w.source + "/" + rel, to = w.path + "/" + rel
            guard fm.fileExists(atPath: from), !fm.fileExists(atPath: to) else { continue }
            try? fm.createDirectory(atPath: (to as NSString).deletingLastPathComponent, withIntermediateDirectories: true)
            if run("/bin/cp", ["-c", "-R", "-p", from, to]).status != 0 {
                try? fm.removeItem(atPath: to)
                _ = run("/bin/cp", ["-R", "-p", from, to])   // not APFS: a plain copy
            }
            cutLinks(under: to, into: w.source)
        }
    }
    static func cutLinks(under dir: String, into source: String) {
        let fm = FileManager.default
        var isDir: ObjCBool = false
        guard fm.fileExists(atPath: dir, isDirectory: &isDir) else { return }
        let all = isDir.boolValue ? (fm.subpaths(atPath: dir) ?? []).map { dir + "/" + $0 } : [dir]
        let realSource = (source as NSString).resolvingSymlinksInPath
        for p in all {
            guard let dest = try? fm.destinationOfSymbolicLink(atPath: p) else { continue }
            let abs = dest.hasPrefix("/") ? dest : ((p as NSString).deletingLastPathComponent + "/" + dest)
            let real = (abs as NSString).standardizingPath
            if real == source || real.hasPrefix(source + "/") || real == realSource || real.hasPrefix(realSource + "/") { try? fm.removeItem(atPath: p) }
        }
    }

    /// The worktree's files that are changed or new and not ignored (git status --porcelain), one a
    /// line; nil when git could not tell, which counts as changed.
    static func changes(_ w: ChatWorktree) -> [String]? {
        let r = git(["status", "--porcelain", "--untracked-files=normal"], in: w.path)
        guard r.status == 0 else { return nil }
        return r.output.split(separator: "\n").map(String.init).filter { !$0.isEmpty }
    }

    enum Removal: Equatable { case removed, gone, kept(changes: Int) }
    /// Removes a clean worktree (git worktree remove, then prune). One with changes, or one git
    /// can't read, is never removed: it stays, and the caller says so. The branch goes too when
    /// everything on it is in another branch; else it is kept with its commits.
    static func remove(_ w: ChatWorktree) -> Removal {
        let fm = FileManager.default
        guard fm.fileExists(atPath: w.path) else { _ = git(["worktree", "prune"], in: w.source); return .gone }
        guard let left = changes(w) else { return .kept(changes: -1) }
        guard left.isEmpty else { return .kept(changes: left.count) }
        let r = git(["worktree", "remove", w.path], in: w.source)
        guard r.status == 0 else { return .kept(changes: -1) }
        _ = git(["worktree", "prune"], in: w.source)
        _ = git(["branch", "-d", "--quiet", w.branch], in: w.source)   // refused when it has commits nowhere else
        return .removed
    }

    @discardableResult
    static func git(_ args: [String], in dir: String) -> (status: Int32, output: String) {
        run("/usr/bin/git", ["-C", dir] + args)
    }
    static func run(_ program: String, _ args: [String]) -> (status: Int32, output: String) {
        let p = Process(), out = Pipe()
        p.executableURL = URL(fileURLWithPath: program)
        p.arguments = args
        var env = ProcessInfo.processInfo.environment
        env["GIT_TERMINAL_PROMPT"] = "0"
        p.environment = env
        p.standardInput = FileHandle.nullDevice
        p.standardOutput = out; p.standardError = out
        do { try p.run() } catch { return (-1, "\(error)") }
        let data = out.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        return (p.terminationStatus, String(decoding: data, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines))
    }
}

private extension String {
    func leftPadded(_ n: Int) -> String { count >= n ? self : String(repeating: "0", count: n - count) + self }
}
