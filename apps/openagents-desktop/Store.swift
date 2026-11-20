// Firm reason: 6 (Swift-to-C host wrapper over coax/lib/agent/desktop/store.coax; GitHub #13794).
import AppKit

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

    enum CodingKeys: String, CodingKey {
        case id, title, created, updated, folder, renamed, pinned
        case managedBy = "managed_by"
        case worktreePath = "worktree_path"
        case worktreeBranch = "worktree_branch"
        case repoRoot = "repo_root"
    }
    var worktree: ChatWorktree? {
        guard let worktreePath, let repoRoot, let worktreeBranch else { return nil }
        return ChatWorktree(chat: id, path: worktreePath, source: repoRoot, branch: worktreeBranch)
    }
}

/// The chats kept on this Mac, persisted and sorted by coax/lib/agent/desktop/store.coax.
final class CoderStore {
    private let dirPath: String
    private let dir: URL
    private(set) var list: [CoderChat] = []

    init() {
        let p = DesktopCore.callString(4096) { oa_dc_chats_dir($0) }
        self.dirPath = p
        self.dir = URL(fileURLWithPath: p, isDirectory: true)
        reload()
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
            let ta = a.element.created ?? a.element.updated
            let tb = b.element.created ?? b.element.updated
            return ta != tb ? ta > tb : a.offset < b.offset
        }.map(\.element)
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
        reload()
    }

    func worktree(_ id: String) -> ChatWorktree? { list.first { $0.id == id }?.worktree }

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
