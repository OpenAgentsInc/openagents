// Firm reason: 6 (Swift-to-C host wrapper over coax/lib/agent/desktop/store.coax; GitHub #13794).
import AppKit

#if !OA_FULL
enum Foreman {
    static let chatID = "foreman"
    static let title = "Foreman"
    static let managedTag = "foreman"
}
#endif

/// One kept chat in the list.
struct CoderChat: Codable {
    var id: String
    var title: String
    var created: Double? = nil
    var updated: Double
    var folder: String
    var renamed: Bool? = nil
    var pinned: Int? = nil
    var managedBy: String? = nil
    var worktreePath: String? = nil
    var worktreeBranch: String? = nil
    var repoRoot: String? = nil
    var archived: Bool? = nil
    var snoozedUntilMs: Int64? = nil

    enum CodingKeys: String, CodingKey {
        case id, title, created, updated, folder, renamed, pinned, archived
        case managedBy = "managed_by"
        case worktreePath = "worktree_path"
        case worktreeBranch = "worktree_branch"
        case repoRoot = "repo_root"
        case snoozedUntilMs = "snoozed_until_ms"
    }
    var worktree: ChatWorktree? {
        guard let worktreePath, let repoRoot, let worktreeBranch else { return nil }
        return ChatWorktree(chat: id, path: worktreePath, source: repoRoot, branch: worktreeBranch)
    }
}

/// Sidecar sidebar metadata (archived, snoozedUntilMs) persisted
/// beside index.json so oa_dc_store_* rewrites never drop sidebar state (#13875).
private struct SidebarMetaStore: Codable {
    var archived: Set<String> = []
    var snoozedUntilMs: [String: Int64] = [:]
}

/// One ranked full-text hit across stored chats (GitHub #13876).
struct ChatContentHit {
    let chatId: String
    let title: String
    let turnIndex: Int
    let snippet: String
    let score: Int
}

/// The chats kept on this Mac, persisted and sorted by coax/lib/agent/desktop/store.coax.
final class CoderStore {
    private let dirPath: String
    private let dir: URL
    private(set) var list: [CoderChat] = []
    private var sidebarMeta = SidebarMetaStore()
    private struct IndexedTurn {
        let turnIndex: Int
        let ask: String
        let reply: String
        let tools: [String]
    }
    private struct IndexedChat {
        let mtime: Double
        let turns: [IndexedTurn]
    }
    private var searchCache: [String: IndexedChat] = [:]
    private let searchLock = NSLock()

    init() {
        let p = DesktopCore.callString(4096) { oa_dc_chats_dir($0) }
        self.dirPath = p
        self.dir = URL(fileURLWithPath: p, isDirectory: true)
        loadSidebarMeta()
        reload()
    }

    private var sidebarMetaURL: URL { dir.appendingPathComponent("sidebar-meta.json") }

    private func loadSidebarMeta() {
        Self.lock.lock(); defer { Self.lock.unlock() }
        guard let data = try? Data(contentsOf: sidebarMetaURL),
              let decoded = try? JSONDecoder().decode(SidebarMetaStore.self, from: data) else { return }
        sidebarMeta = decoded
    }

    private func saveSidebarMeta() {
        Self.lock.lock(); defer { Self.lock.unlock() }
        if let data = try? JSONEncoder().encode(sidebarMeta) {
            try? data.write(to: sidebarMetaURL, options: .atomic)
        }
    }

    /// Every store call, in every window, one at a time: two saves at once shared index.json's
    /// temp file, so a read met a half-written index and a save wrote back one chat (the list wiped).
    private static let lock = NSRecursiveLock()
    private func core<R>(_ body: (coax_string) -> R) -> R {
        Self.lock.lock(); defer { Self.lock.unlock() }
        return DesktopCore.withCoaxString(dirPath, body)
    }

    private func reload() {
        let json = core { d in
            DesktopCore.callString(512 * 1024) { out in oa_dc_store_index_read(d, out) }
        }
        guard !json.isEmpty, let data = json.data(using: .utf8),
              let kept = try? JSONDecoder().decode([CoderChat].self, from: data) else {
            NSLog("Coder: the chat list could not be read; keeping the %d chats shown", list.count)
            return
        }
        list = kept.enumerated().sorted { a, b in
            let ta = a.element.updated > 0 ? a.element.updated : (a.element.created ?? 0)
            let tb = b.element.updated > 0 ? b.element.updated : (b.element.created ?? 0)
            return ta != tb ? ta > tb : a.offset < b.offset
        }.map { pair in
            var c = pair.element
            if sidebarMeta.archived.contains(c.id) { c.archived = true }
            if let s = sidebarMeta.snoozedUntilMs[c.id], s > 0 { c.snoozedUntilMs = s }
            return c
        }
    }

    static func cleanTitle(_ raw: String) -> String {
        DesktopCore.withCoaxString(raw) { r in
            DesktopCore.callString(512) { out in oa_dc_clean_title(r, out) }
        }
    }

    private func saveIndex() {
        Self.lock.lock(); defer { Self.lock.unlock() }
        if !list.isEmpty, let data = try? JSONEncoder().encode(list) {
            try? data.write(to: dir.appendingPathComponent("index.json"), options: .atomic)
        }
    }

    var pinnedList: [CoderChat] { list.filter { $0.id != Foreman.chatID && $0.pinned != nil }.sorted { ($0.pinned ?? 0) < ($1.pinned ?? 0) } }
    var recentList: [CoderChat] { list.filter { $0.id != Foreman.chatID && $0.pinned == nil } }
    var orderedList: [CoderChat] { pinnedList + recentList }
    var lines: String { orderedList.map { $0.id + "\t" + $0.title }.joined(separator: "\n") }

    func text(_ id: String) -> String? {
        // sized from the file: a long chat outgrew a fixed 4 MB and could not be opened
        let path = dir.appendingPathComponent(id + ".json").path
        let size = ((try? FileManager.default.attributesOfItem(atPath: path))?[.size] as? NSNumber)?.intValue ?? 0
        var buf = [UInt8](repeating: 0, count: max(4 * 1024 * 1024, size + 64 * 1024))
        let n: Int64 = core { d in
            DesktopCore.withCoaxString(id) { cid in
                DesktopCore.withMutableBytes(&buf) { out in
                    oa_dc_store_text(d, cid, out)
                }
            }
        }
        if n < 0 { return nil }
        return String(decoding: buf.prefix(Int(n)), as: UTF8.self)
    }

    func folder(_ id: String) -> String? { list.first { $0.id == id }?.folder }
    func title(_ id: String) -> String? { list.first { $0.id == id }?.title }
    func meta(_ id: String) -> CoderChat? { list.first { $0.id == id } }
    func isPinned(_ id: String) -> Bool { list.first { $0.id == id }?.pinned != nil }
    func isManagedByForeman(_ id: String) -> Bool { list.first { $0.id == id }?.managedBy == Foreman.managedTag }

    func togglePin(_ id: String) {
        _ = core { d in
            DesktopCore.withCoaxString(id) { cid in
                oa_dc_store_toggle_pin(d, cid)
            }
        }
        reload()
    }

    func movePinned(_ id: String, to index: Int) {
        var ids = pinnedList.map(\.id)
        guard let from = ids.firstIndex(of: id), !ids.isEmpty else { return }
        let to = max(0, min(ids.count - 1, index))
        guard from != to else { return }
        let moved = ids.remove(at: from)
        ids.insert(moved, at: to)
        for (order, cid) in ids.enumerated() {
            if let i = list.firstIndex(where: { $0.id == cid }) { list[i].pinned = order }
        }
        saveIndex()
    }

    func rename(_ id: String, to title: String) {
        _ = core { d in
            DesktopCore.withCoaxString(id) { cid in
                DesktopCore.withCoaxString(title) { t in
                    oa_dc_store_rename(d, cid, t)
                }
            }
        }
        reload()
    }

    func remove(_ id: String) {
        _ = core { d in
            DesktopCore.withCoaxString(id) { cid in
                oa_dc_store_remove(d, cid)
            }
        }
        searchLock.lock(); searchCache.removeValue(forKey: id); searchLock.unlock()
        reload()
    }

    func keep(id: String, title: String, folder: String, text: String, managedBy: String? = nil, worktree: ChatWorktree? = nil) {
        let wtPath = (worktree?.chat == id ? worktree?.path : nil) ?? ""
        let wtBranch = (worktree?.chat == id ? worktree?.branch : nil) ?? ""
        let repo = (worktree?.chat == id ? worktree?.source : nil) ?? ""
        let managed = managedBy ?? ""
        _ = core { d in
            DesktopCore.withCoaxString(id) { cid in
                DesktopCore.withCoaxString(title) { t in
                    DesktopCore.withCoaxString(folder) { f in
                        DesktopCore.withCoaxString(text) { tx in
                            DesktopCore.withCoaxString(managed) { m in
                                DesktopCore.withCoaxString(wtPath) { wp in
                                    DesktopCore.withCoaxString(wtBranch) { wb in
                                        DesktopCore.withCoaxString(repo) { rp in
                                            oa_dc_store_keep(d, cid, t, f, tx, m, wp, wb, rp)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        indexChatJSON(id: id, text: text, mtime: Date().timeIntervalSince1970)
        reload()
    }

    private func parseIndexedTurns(_ text: String) -> [IndexedTurn] {
        guard let data = text.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let turns = obj["turns"] as? [[String: Any]] else { return [] }
        var out: [IndexedTurn] = []
        out.reserveCapacity(turns.count)
        for (i, t) in turns.enumerated() {
            let ask = (t["ask"] as? String) ?? ""
            let reply = (t["reply"] as? String) ?? ""
            var tools: [String] = []
            if let cmds = t["cmds"] as? [[String: Any]] {
                for c in cmds {
                    if let ct = c["text"] as? String, !ct.isEmpty { tools.append(ct) }
                }
            }
            out.append(IndexedTurn(turnIndex: i, ask: ask, reply: reply, tools: tools))
        }
        return out
    }

    private func indexChatJSON(id: String, text: String, mtime: Double) {
        let turns = parseIndexedTurns(text)
        searchLock.lock()
        searchCache[id] = IndexedChat(mtime: mtime, turns: turns)
        searchLock.unlock()
    }

    private static func snippetAround(_ text: String, range: Range<String.Index>) -> String {
        let start = text.index(range.lowerBound, offsetBy: -36, limitedBy: text.startIndex) ?? text.startIndex
        let end = text.index(range.upperBound, offsetBy: 72, limitedBy: text.endIndex) ?? text.endIndex
        let raw = String(text[start..<end])
            .replacingOccurrences(of: "\n", with: " ")
            .replacingOccurrences(of: "\r", with: " ")
            .replacingOccurrences(of: "\t", with: " ")
        let collapsed = raw.split(separator: " ", omittingEmptySubsequences: true).joined(separator: " ")
        let prefix = start > text.startIndex ? "…" : ""
        let suffix = end < text.endIndex ? "…" : ""
        return prefix + collapsed + suffix
    }

    /// Incremental full-text search across all stored chats (prompts, replies, tool titles), ranked by field weight & recency.
    func searchContent(query: String, limit: Int = 32) -> [ChatContentHit] {
        let q = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !q.isEmpty else { return [] }
        let snapshot = orderedList
        let fm = FileManager.default
        var hits: [(hit: ChatContentHit, order: Int)] = []
        for (order, chat) in snapshot.enumerated() {
            let url = dir.appendingPathComponent(chat.id + ".json")
            let mtime = ((try? fm.attributesOfItem(atPath: url.path))?[.modificationDate] as? Date)?.timeIntervalSince1970 ?? 0
            searchLock.lock()
            let cached = searchCache[chat.id]
            searchLock.unlock()
            let turns: [IndexedTurn]
            if let cached, cached.mtime >= mtime {
                turns = cached.turns
            } else if let raw = try? String(contentsOf: url, encoding: .utf8) {
                let parsed = parseIndexedTurns(raw)
                searchLock.lock()
                searchCache[chat.id] = IndexedChat(mtime: mtime, turns: parsed)
                searchLock.unlock()
                turns = parsed
            } else {
                continue
            }
            var chatMatches = 0
            for t in turns {
                if chatMatches >= 3 { break }
                if let r = t.ask.range(of: q, options: [.caseInsensitive, .diacriticInsensitive]) {
                    let snip = Self.snippetAround(t.ask, range: r)
                    hits.append((ChatContentHit(chatId: chat.id, title: chat.title, turnIndex: t.turnIndex, snippet: snip, score: 100), order))
                    chatMatches += 1
                    continue
                }
                if let r = t.reply.range(of: q, options: [.caseInsensitive, .diacriticInsensitive]) {
                    let snip = Self.snippetAround(t.reply, range: r)
                    hits.append((ChatContentHit(chatId: chat.id, title: chat.title, turnIndex: t.turnIndex, snippet: snip, score: 80), order))
                    chatMatches += 1
                    continue
                }
                for tool in t.tools {
                    if let r = tool.range(of: q, options: [.caseInsensitive, .diacriticInsensitive]) {
                        let snip = Self.snippetAround(tool, range: r)
                        hits.append((ChatContentHit(chatId: chat.id, title: chat.title, turnIndex: t.turnIndex, snippet: snip, score: 60), order))
                        chatMatches += 1
                        break
                    }
                }
            }
        }
        hits.sort { a, b in
            if a.hit.score != b.hit.score { return a.hit.score > b.hit.score }
            return a.order < b.order
        }
        return Array(hits.prefix(limit).map(\.hit))
    }

    func worktree(_ id: String) -> ChatWorktree? { list.first { $0.id == id }?.worktree }

    // ---- Sidebar archiving, snoozing & undo restore (#13875)

    func isArchived(_ id: String) -> Bool {
        sidebarMeta.archived.contains(id) || (list.first { $0.id == id }?.archived == true)
    }

    func setArchived(_ id: String, _ archived: Bool) {
        if archived {
            sidebarMeta.archived.insert(id)
            sidebarMeta.snoozedUntilMs.removeValue(forKey: id)
        } else {
            sidebarMeta.archived.remove(id)
        }
        if let i = list.firstIndex(where: { $0.id == id }) {
            list[i].archived = archived ? true : nil
            if archived { list[i].snoozedUntilMs = nil }
        }
        saveSidebarMeta()
        saveIndex()
    }

    func snoozedUntil(_ id: String) -> Int64 {
        sidebarMeta.snoozedUntilMs[id] ?? (list.first { $0.id == id }?.snoozedUntilMs ?? 0)
    }

    func setSnoozed(_ id: String, untilMs: Int64) {
        if untilMs > 0 {
            sidebarMeta.snoozedUntilMs[id] = untilMs
        } else {
            sidebarMeta.snoozedUntilMs.removeValue(forKey: id)
        }
        if let i = list.firstIndex(where: { $0.id == id }) {
            list[i].snoozedUntilMs = untilMs > 0 ? untilMs : nil
        }
        saveSidebarMeta()
        saveIndex()
    }

    func wakeExpiredSnoozes(nowMs: Int64) -> Bool {
        var woke = false
        for (id, wakeAt) in sidebarMeta.snoozedUntilMs where wakeAt > 0 && nowMs >= wakeAt {
            sidebarMeta.snoozedUntilMs.removeValue(forKey: id)
            if let i = list.firstIndex(where: { $0.id == id }) { list[i].snoozedUntilMs = nil }
            woke = true
        }
        if woke {
            saveSidebarMeta()
            saveIndex()
        }
        return woke
    }

    func effectiveFolder(_ chat: CoderChat) -> String {
        let raw = chat.repoRoot ?? chat.folder
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return "" }
        return URL(fileURLWithPath: trimmed).standardizedFileURL.path
    }

    func restoreDeleted(meta: CoderChat, text: String) {
        keep(id: meta.id, title: meta.title, folder: meta.folder, text: text, managedBy: meta.managedBy, worktree: meta.worktree)
        if meta.renamed == true { rename(meta.id, to: meta.title) }
        if meta.pinned != nil && !isPinned(meta.id) { togglePin(meta.id) }
        if meta.archived == true { setArchived(meta.id, true) }
        if let wake = meta.snoozedUntilMs, wake > 0 { setSnoozed(meta.id, untilMs: wake) }
    }

    func registerChat(id: String, title: String, folder: String, managedBy: String? = nil) {
        let managed = managedBy ?? ""
        _ = core { d in
            DesktopCore.withCoaxString(id) { cid in
                DesktopCore.withCoaxString(title) { t in
                    DesktopCore.withCoaxString(folder) { f in
                        DesktopCore.withCoaxString(managed) { m in
                            oa_dc_store_register(d, cid, t, f, m)
                        }
                    }
                }
            }
        }
        reload()
    }
}
