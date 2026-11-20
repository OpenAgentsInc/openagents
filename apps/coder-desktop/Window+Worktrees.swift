// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// Each new chat in a repository gets its own worktree before its first message runs (GitHub #13758).
import AppKit

extension CoderWindow {
    /// A new chat's first message in a repository's main checkout waits while Coder makes the
    /// chat's worktree (Worktree.swift) off the main thread; then the chat's folder is the
    /// worktree and the message is sent from there, so it, its sub-agents (which start in the
    /// run's folder) and every later message work on the chat's own branch. True when the send
    /// waits; act(1) runs again once the worktree is there. When it can't be made the message runs
    /// in the folder chosen, as before, and the shell's guard still refuses shared-checkout stashes.
    func holdForWorktree() -> Bool {
        let r = run
        if r.worktreeSkip { r.worktreeSkip = false; return false }
        if r.preparingWorktree { return true }
        guard ChatWorktree.enabled, !isForemanShown, chat == nil else { return false }
        // made for this message already (the send is coming back from below)
        if let w = r.worktree, store.meta(w.chat) == nil, folder.path == URL(fileURLWithPath: w.path).standardizedFileURL.path { return false }
        guard let repo = ChatWorktree.mainCheckout(of: worktreeSource(folder.path) ?? folder.path) else { return false }
        let id = UUID().uuidString.lowercased()
        r.preparingWorktree = true
        DispatchQueue.global(qos: .userInitiated).async {
            let made = ChatWorktree.create(repo: repo, chat: id)
            DispatchQueue.main.async { [weak self] in
                r.preparingWorktree = false
                guard let self else { return }
                let w = try? made.get()
                // the message was stopped, or the window moved on, while the worktree was made: a fresh one goes again
                guard r === self.run, self.chat == nil, cd_running(self.page) != 0 else {
                    if let w { DispatchQueue.global(qos: .utility).async { _ = ChatWorktree.remove(w) } }
                    return
                }
                if let w {
                    r.worktree = w
                    self.setFolder(URL(fileURLWithPath: w.path, isDirectory: true), keep: false)
                } else {
                    r.worktreeSkip = true
                    if case .failure(.git(let why)) = made { NSLog("Coder: a chat worktree could not be made in %@: %@", repo, why) }
                    cd_toast(self.page, "Couldn't make this chat's worktree, so it runs in the folder you chose", 1); self.wake()
                }
                self.act(1)
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

    /// The same for a chat that starts out of view (one Foreman creates): its run's folder becomes
    /// its worktree once that is made off the main thread, then `then` sends its first message. True
    /// when it waits. When the worktree can't be made, the message runs in the folder asked for.
    func holdRunForWorktree(_ r: CoderRun, then: @escaping () -> Void) -> Bool {
        guard ChatWorktree.enabled, let id = r.chat, id != Foreman.chatID, r.worktree == nil,
              let repo = ChatWorktree.mainCheckout(of: worktreeSource(r.folder.path) ?? r.folder.path) else { return false }
        r.preparingWorktree = true
        DispatchQueue.global(qos: .userInitiated).async {
            let made = try? ChatWorktree.create(repo: repo, chat: id).get()
            DispatchQueue.main.async { [weak self] in
                r.preparingWorktree = false
                if let made {
                    r.worktree = made; r.folder = URL(fileURLWithPath: made.path, isDirectory: true)
                    if let self, r === self.run { self.setFolder(r.folder, keep: false) }   // opened meanwhile
                }
                then()
            }
        }
        return true
    }

    /// The worktree a kept chat's run made, for keep(): only the one made for this chat id.
    func madeWorktree(_ r: CoderRun, for id: String) -> ChatWorktree? {
        guard let w = r.worktree, w.chat == id else { return nil }
        return w
    }
    /// The id a new chat is kept under: its worktree's, when one was made for its first message.
    func pendingChatID(_ r: CoderRun) -> String? {
        guard let w = r.worktree, store.meta(w.chat) == nil else { return nil }
        return w.chat
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
