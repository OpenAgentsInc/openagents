// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Pre-warmed standby git worktrees so starting a chat never blocks inference (GitHub #13758).
import AppKit

/// Keeps at most one clean, pre-warmed standby worktree per repository off the main thread so
/// sending a message claims an isolated worktree in <1 ms instead of blocking 3+ s on `git worktree add`.
/// Persists unclaimed standby entries across restarts so they are reused rather than leaked.
final class StandbyWorktreePool {
    static let shared = StandbyWorktreePool()

    private let defaultsKey = "standbyWorktrees"
    private let lock = NSLock()
    private let queue = DispatchQueue(label: "openagents.coder.worktree-pool", qos: .utility)
    private var ready: [String: ChatWorktree] = [:]
    private var warming: Set<String> = []
    private var refreshed: Set<String> = []

    private init() {
        if let data = UserDefaults.standard.data(forKey: defaultsKey),
           let saved = try? JSONDecoder().decode([String: ChatWorktree].self, from: data) {
            ready = saved
        }
    }

    private func persistLocked() {
        if let data = try? JSONEncoder().encode(ready) {
            UserDefaults.standard.set(data, forKey: defaultsKey)
        }
    }

    /// Synchronously claims a ready standby worktree for `repo` (<1 ms, zero subprocesses).
    /// Returns `nil` if none is ready yet so the caller never blocks sending a message.
    func claimIfReady(repo: String) -> ChatWorktree? {
        let key = URL(fileURLWithPath: repo).standardizedFileURL.path
        lock.lock()
        guard let w = ready.removeValue(forKey: key) else {
            lock.unlock()
            return nil
        }
        refreshed.remove(key)
        persistLocked()
        lock.unlock()
        guard FileManager.default.fileExists(atPath: w.path + "/.git") else { return nil }
        return w
    }

    /// Ensures a clean standby worktree exists for the repository containing `folder` (if any),
    /// doing all git I/O asynchronously on a utility queue.
    func warm(folder: String) {
        guard ChatWorktree.enabled else { return }
        queue.async { [weak self] in
            guard let self, let repo = ChatWorktree.mainCheckout(of: folder) else { return }
            self.warmRepoSync(repo)
        }
    }

    /// Replenishes a standby worktree for a known main checkout `repo` asynchronously.
    func warm(repo: String) {
        guard ChatWorktree.enabled else { return }
        queue.async { [weak self] in
            self?.warmRepoSync(repo)
        }
    }

    private func warmRepoSync(_ repo: String) {
        let key = URL(fileURLWithPath: repo).standardizedFileURL.path
        lock.lock()
        if warming.contains(key) { lock.unlock(); return }
        if let existing = ready[key] {
            if refreshed.contains(key), FileManager.default.fileExists(atPath: existing.path + "/.git") {
                lock.unlock()
                return
            }
            // Temporarily check out the cached standby to verify/fast-forward it off-thread.
            ready.removeValue(forKey: key)
            warming.insert(key)
            persistLocked()
            lock.unlock()
            if refreshCleanStandby(existing, repo: key) {
                lock.lock()
                ready[key] = existing
                refreshed.insert(key)
                warming.remove(key)
                persistLocked()
                lock.unlock()
                return
            }
            // Existing standby was missing or dirty — fall through to create a fresh one.
        } else {
            warming.insert(key)
            lock.unlock()
        }

        let id = UUID().uuidString.lowercased()
        let made = try? ChatWorktree.create(repo: key, chat: id).get()
        lock.lock()
        warming.remove(key)
        if let made {
            ready[key] = made
            refreshed.insert(key)
            persistLocked()
        }
        lock.unlock()
    }

    private func refreshCleanStandby(_ w: ChatWorktree, repo: String) -> Bool {
        guard FileManager.default.fileExists(atPath: w.path + "/.git") else { return false }
        guard let changes = ChatWorktree.changes(w), changes.isEmpty else { return false }
        let base = gitRev(in: repo, ref: "origin/main^{commit}") ?? gitRev(in: repo, ref: "HEAD^{commit}")
        if let base, gitRev(in: w.path, ref: "HEAD^{commit}") != base {
            guard runGit(in: w.path, args: ["reset", "--hard", "--quiet", base]) else { return false }
        }
        ChatWorktree.hydrate(w)
        return true
    }

    private func gitRev(in dir: String, ref: String) -> String? {
        let p = Process()
        let out = Pipe()
        p.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        p.arguments = ["git", "-C", dir, "rev-parse", "--verify", "--quiet", ref]
        p.environment = ProcessInfo.processInfo.environment.merging(["GIT_TERMINAL_PROMPT": "0"]) { _, new in new }
        p.standardOutput = out
        p.standardError = FileHandle.nullDevice
        do { try p.run() } catch { return nil }
        let data = out.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        guard p.terminationStatus == 0 else { return nil }
        let s = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return s.isEmpty ? nil : s
    }

    private func runGit(in dir: String, args: [String]) -> Bool {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        p.arguments = ["git", "-C", dir] + args
        p.environment = ProcessInfo.processInfo.environment.merging(["GIT_TERMINAL_PROMPT": "0"]) { _, new in new }
        p.standardOutput = FileHandle.nullDevice
        p.standardError = FileHandle.nullDevice
        do { try p.run() } catch { return false }
        p.waitUntilExit()
        return p.terminationStatus == 0
    }
}

extension CoderWindow {
    /// Pre-warms a standby worktree in the background for `url` when it is inside a Git repository.
    func prewarmWorktree(for url: URL) {
        guard ChatWorktree.enabled else { return }
        StandbyWorktreePool.shared.warm(folder: url.path)
    }

    /// Creates or claims an isolated git worktree when the user opted in via the "New worktree"
    /// checkbox (default false). Claims a pre-warmed standby immediately (<1 ms) when available,
    /// otherwise creates the requested worktree asynchronously before starting the worker.
    func holdForWorktree() -> Bool {
        let r = run
        if r.worktreeSkip { r.worktreeSkip = false; return false }
        guard ChatWorktree.enabled, chat == nil else { return false }
        if r.preparingWorktree { return true }
        // already assigned a worktree for this unsaved chat
        if let w = r.worktree, store.meta(w.chat) == nil, folder.path == URL(fileURLWithPath: w.path).standardizedFileURL.path { return false }
        guard let repo = ChatWorktree.mainCheckout(of: worktreeSource(folder.path) ?? folder.path) else { return false }
        if let w = StandbyWorktreePool.shared.claimIfReady(repo: repo) {
            r.worktree = w
            r.pendingChat = w.chat
            r.folder = URL(fileURLWithPath: w.path, isDirectory: true)
            setFolder(r.folder, keep: false)
            StandbyWorktreePool.shared.warm(repo: repo)
            return false
        }
        // User explicitly opted into a new worktree, and no standby is ready yet: create it off the main thread.
        r.preparingWorktree = true
        let gen = r.startGen
        let id = UUID().uuidString.lowercased()
        r.pendingChat = id
        DispatchQueue.global(qos: .userInitiated).async { [weak self, weak r] in
            let made = ChatWorktree.create(repo: repo, chat: id)
            DispatchQueue.main.async {
                guard let self, let r, r.startGen == gen else { return }
                r.preparingWorktree = false
                guard cd_running(r.page) != 0 else {
                    if case .success(let w) = made {
                        DispatchQueue.global(qos: .utility).async { _ = ChatWorktree.remove(w) }
                    }
                    return
                }
                switch made {
                case .success(let w):
                    r.worktree = w
                    r.pendingChat = w.chat
                    r.folder = URL(fileURLWithPath: w.path, isDirectory: true)
                    if r === self.run { self.setFolder(r.folder, keep: false) }
                    StandbyWorktreePool.shared.warm(repo: repo)
                    self.wake()
                    self.sendTask(r)
                case .failure(let err):
                    // Never silently fall back to editing the shared main checkout when a worktree was requested.
                    r.pendingChat = nil
                    cd_failed(r.page, "Couldn’t create this chat’s git worktree (\(err.localizedDescription)). Stopped so the main checkout isn’t modified.")
                    self.wake()
                }
            }
        }
        return true
    }

    /// The main checkout a chat's worktree folder was made from, when folder is one.
    func worktreeSource(_ folder: String) -> String? {
        store.list.first { $0.worktreePath.map { URL(fileURLWithPath: $0).standardizedFileURL.path } == folder }?.repoRoot
    }
    /// A new chat starts in the main checkout the shown chat's worktree came from, not in that worktree.
    func leaveWorktreeFolder() {
        guard let source = worktreeSource(folder.path), FileManager.default.fileExists(atPath: source) else { return }
        setFolder(URL(fileURLWithPath: source, isDirectory: true), keep: false)
    }

    /// Worktree claim for background chats (e.g. Foreman): uses a ready standby worktree immediately,
    /// or creates one asynchronously instead of silently falling back to the shared main checkout.
    func holdRunForWorktree(_ r: CoderRun, then: @escaping () -> Void) -> Bool {
        guard ChatWorktree.enabled, let id = r.chat, r.worktree == nil,
              let repo = ChatWorktree.mainCheckout(of: worktreeSource(r.folder.path) ?? r.folder.path) else { return false }
        if var made = StandbyWorktreePool.shared.claimIfReady(repo: repo) {
            made.chat = id
            r.worktree = made
            r.folder = URL(fileURLWithPath: made.path, isDirectory: true)
            if r === self.run { self.setFolder(r.folder, keep: false) }
            StandbyWorktreePool.shared.warm(repo: repo)
            return false
        }
        r.preparingWorktree = true
        let gen = r.startGen
        DispatchQueue.global(qos: .userInitiated).async { [weak self, weak r] in
            let made = ChatWorktree.create(repo: repo, chat: id)
            DispatchQueue.main.async {
                guard let self, let r, r.startGen == gen else { return }
                r.preparingWorktree = false
                switch made {
                case .success(let w):
                    r.worktree = w
                    r.folder = URL(fileURLWithPath: w.path, isDirectory: true)
                    if r === self.run { self.setFolder(r.folder, keep: false) }
                    StandbyWorktreePool.shared.warm(repo: repo)
                    self.wake()
                    then()
                case .failure(let err):
                    cd_failed(r.page, "Couldn’t create this chat’s git worktree (\(err.localizedDescription)). Stopped so the main checkout isn’t modified.")
                    self.wake()
                }
            }
        }
        return true
    }

    /// The worktree a kept chat's run made, for keep(): only the one made for this chat id.
    func madeWorktree(_ r: CoderRun, for id: String) -> ChatWorktree? {
        guard let w = r.worktree, w.chat == id else { return nil }
        return w
    }
    /// The id a new chat is kept under: its worktree's, when one was made or is being made for its first message.
    func pendingChatID(_ r: CoderRun) -> String? {
        if let w = r.worktree, store.meta(w.chat) == nil { return w.chat }
        if let id = r.pendingChat, store.meta(id) == nil { return id }
        return nil
    }

    /// An opened chat whose worktree folder is gone (removed by hand) gets it back from its branch
    /// before its next message, so it goes on where it was; the folder is set when it is there.
    func restoreWorktree(_ id: String) {
        guard let w = store.worktree(id), !FileManager.default.fileExists(atPath: w.path),
              FileManager.default.fileExists(atPath: w.source + "/.git") else { return }
        DispatchQueue.global(qos: .userInitiated).async {
            let made = try? ChatWorktree.make(w).get()
            DispatchQueue.main.async { [weak self] in
                guard let self, let made, self.chat == id, cd_running(self.page) == 0 else { return }
                self.setFolder(URL(fileURLWithPath: made.path, isDirectory: true), keep: false)
            }
        }
    }

    /// A deleted chat's worktree goes when it has no changes. One with changes, or one git can't
    /// read, stays where it is, and Coder says so: a dirty worktree is never removed silently.
    func removeWorktree(_ w: ChatWorktree, title: String) {
        DispatchQueue.global(qos: .utility).async {
            let gone = ChatWorktree.remove(w)
            guard case .kept(let n) = gone else { return }
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                let what = n > 0 ? (n == 1 ? "1 file that was changed or added" : "\(n) files that were changed or added") : "changes Coder could not check"
                self.present(.notice("The chat’s files were kept.", "“" + title + "” was deleted, but its folder " + w.path + " (branch " + w.branch + ") has " + what + ", so it stays on this Mac."))
            }
        }
    }
}
