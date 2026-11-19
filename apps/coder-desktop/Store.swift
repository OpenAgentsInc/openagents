// Firm reason: 6 (AppKit lifecycle, the window, the platform editor and the worker process over the compiled page).
// The chats kept on this Mac: their list and their files.
import AppKit

/// One kept chat in the list: its id (its file's name), its title, when it was created, when it last changed,
/// its working folder, and whether the user gave it a custom title that should not be overwritten on subsequent saves.
struct CoderChat: Codable {
    var id: String
    var title: String
    var created: Double? = nil
    var updated: Double
    var folder: String
    var renamed: Bool? = nil
    var pinned: Int? = nil
    var managedBy: String? = nil

    enum CodingKeys: String, CodingKey {
        case id, title, created, updated, folder, renamed, pinned
        case managedBy = "managed_by"
    }
}
/// The chats kept on this Mac: ~/Library/Application Support/Coder/chats, one file a chat (ID.json,
/// the page's text) and index.json, the list, newest created first.
final class CoderStore {
    private let dir: URL
    private(set) var list: [CoderChat] = []
    init() {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first ?? FileManager.default.homeDirectoryForCurrentUser
        dir = base.appendingPathComponent("Coder/chats")
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        if let data = try? Data(contentsOf: dir.appendingPathComponent("index.json")), let kept = try? JSONDecoder().decode([CoderChat].self, from: data) {
            list = kept.enumerated().sorted { a, b in
                let ta = a.element.created ?? a.element.updated
                let tb = b.element.created ?? b.element.updated
                return ta != tb ? ta > tb : a.offset < b.offset
            }.map(\.element)
        }
    }
    /// One-line title: collapses whitespace/tabs/newlines and clamps cleanly to 120 UTF-8 bytes (CP_CHAT_TITLE).
    static func cleanTitle(_ raw: String) -> String {
        let collapsed = raw.components(separatedBy: .whitespacesAndNewlines).filter { !$0.isEmpty }.joined(separator: " ")
        var out = ""
        var bytes = 0
        for ch in collapsed {
            let n = ch.utf8.count
            if bytes + n > 120 { break }
            out.append(ch)
            bytes += n
        }
        return out
    }
    private func saveIndex() {
        if let data = try? JSONEncoder().encode(list) { try? data.write(to: dir.appendingPathComponent("index.json"), options: .atomic) }
    }
    var pinnedList: [CoderChat] { list.filter { $0.id != Foreman.chatID && $0.pinned != nil }.sorted { ($0.pinned ?? 0) < ($1.pinned ?? 0) } }
    var recentList: [CoderChat] { list.filter { $0.id != Foreman.chatID && $0.pinned == nil } }
    var orderedList: [CoderChat] { pinnedList + recentList }
    /// The list as the page reads it: one chat a line, its id, a tab, its title.
    var lines: String { orderedList.map { $0.id + "\t" + $0.title }.joined(separator: "\n") }
    func text(_ id: String) -> String? { try? String(contentsOf: dir.appendingPathComponent(id + ".json"), encoding: .utf8) }
    func folder(_ id: String) -> String? { list.first { $0.id == id }?.folder }
    func title(_ id: String) -> String? { list.first { $0.id == id }?.title }
    func meta(_ id: String) -> CoderChat? { list.first { $0.id == id } }
    func isPinned(_ id: String) -> Bool { list.first { $0.id == id }?.pinned != nil }
    func isManagedByForeman(_ id: String) -> Bool { list.first { $0.id == id }?.managedBy == Foreman.managedTag }
    private func normalizePins() {
        let ids = pinnedList.map(\.id)
        for (order, id) in ids.enumerated() {
            if let i = list.firstIndex(where: { $0.id == id }) { list[i].pinned = order }
        }
    }
    /// Pins or unpins a chat, persisting its state and order in index.json.
    func togglePin(_ id: String) {
        guard let i = list.firstIndex(where: { $0.id == id }) else { return }
        if list[i].pinned != nil {
            list[i].pinned = nil
            normalizePins()
        } else {
            let next = (list.compactMap(\.pinned).max() ?? -1) + 1
            list[i].pinned = next
        }
        saveIndex()
    }
    /// Reorders a pinned chat to the given 0-based index in the Pinned section.
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
    /// Renames a kept chat without changing its place in the list.
    func rename(_ id: String, to title: String) {
        let clean = Self.cleanTitle(title)
        guard !clean.isEmpty, let i = list.firstIndex(where: { $0.id == id }) else { return }
        list[i].title = clean
        list[i].renamed = true
        saveIndex()
    }
    /// Deletes a chat: its file and its place in the list. It can't be brought back.
    func remove(_ id: String) {
        try? FileManager.default.removeItem(at: dir.appendingPathComponent(id + ".json"))
        list.removeAll { $0.id == id }
        normalizePins()
        saveIndex()
    }
    /// Keeps a chat's text. A new chat joins the top of the list; an existing chat updates in place
    /// so chats stay ordered chronologically by creation time (newest created first).
    func keep(id: String, title: String, folder: String, text: String, managedBy: String? = nil) {
        guard (try? text.write(to: dir.appendingPathComponent(id + ".json"), atomically: true, encoding: .utf8)) != nil else { return }
        let now = Date().timeIntervalSince1970
        if let i = list.firstIndex(where: { $0.id == id }) {
            if list[i].created == nil { list[i].created = list[i].updated }
            if list[i].renamed != true && id != Foreman.chatID { list[i].title = title }
            list[i].updated = now
            list[i].folder = folder
            if let managedBy { list[i].managedBy = managedBy }
        } else {
            let effectiveTitle = id == Foreman.chatID ? Foreman.title : title
            list.insert(CoderChat(id: id, title: effectiveTitle, created: now, updated: now, folder: folder, managedBy: managedBy), at: 0)
        }
        saveIndex()
    }
    func registerChat(id: String, title: String, folder: String, managedBy: String? = nil) {
        let now = Date().timeIntervalSince1970
        let clean = Self.cleanTitle(title)
        let effectiveTitle = clean.isEmpty ? "New chat" : clean
        if let i = list.firstIndex(where: { $0.id == id }) {
            list[i].title = effectiveTitle
            list[i].renamed = true
            list[i].updated = now
            list[i].folder = folder
            if let managedBy { list[i].managedBy = managedBy }
        } else {
            list.insert(CoderChat(id: id, title: effectiveTitle, created: now, updated: now, folder: folder, renamed: true, managedBy: managedBy), at: 0)
        }
        saveIndex()
    }
}
