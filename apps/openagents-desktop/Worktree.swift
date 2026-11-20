// Firm reason: 6 (Swift-to-C host wrapper over coax/lib/agent/desktop/worktree.coax; GitHub #13794).
import Foundation

/// A chat's own git worktree, backed by coax/lib/agent/desktop/worktree.coax.
struct ChatWorktree: Codable, Equatable {
    var chat: String
    var path: String
    var source: String
    var branch: String

    static var enabled: Bool {
        if let v = UserDefaults.standard.object(forKey: "chatWorktrees") as? Bool { return v }
        return oa_dc_worktrees_on() != 0
    }

    static var home: String {
        let env = ProcessInfo.processInfo.environment["HOME"] ?? ""
        return env.hasPrefix("/") ? env : FileManager.default.homeDirectoryForCurrentUser.path
    }

    static var root: String {
        DesktopCore.callString(4096) { oa_dc_wt_root($0) }
    }

    static func caches(for folder: String) -> [String: String] {
        let raw = DesktopCore.withCoaxString(folder) { f in
            DesktopCore.callString(8192) { out in oa_dc_wt_cache_env(f, out) }
        }
        var result: [String: String] = [:]
        for line in raw.split(separator: "\n") {
            if let eq = line.firstIndex(of: "=") {
                let k = String(line[..<eq])
                let v = String(line[line.index(after: eq)...])
                if !k.isEmpty { result[k] = v }
            }
        }
        return result
    }

    static func repository(of folder: String) -> String? {
        let s = DesktopCore.withCoaxString(folder) { f in
            DesktopCore.callString(4096) { out in oa_dc_wt_repository(f, out) }
        }
        return s.isEmpty ? nil : (s as NSString).standardizingPath
    }

    static func slug(_ given: String) -> String {
        DesktopCore.withCoaxString(given) { g in
            DesktopCore.callString(256) { out in oa_dc_wt_slug(g, out) }
        }
    }

    static func real(_ path: String) -> String {
        DesktopCore.withCoaxString(path) { p in
            DesktopCore.callString(4096) { out in oa_dc_wt_real(p, out) }
        }
    }

    static func clean(_ chat: String) -> String {
        let br = branch(chat)
        return br.hasPrefix("coder/") ? String(br.dropFirst(6)) : br
    }

    static func branch(_ chat: String) -> String {
        DesktopCore.withCoaxString(chat) { c in
            DesktopCore.callString(256) { out in oa_dc_wt_branch(c, out) }
        }
    }

    static func path(repo: String, chat: String, root: String = ChatWorktree.root) -> String {
        DesktopCore.withCoaxString(repo) { r in
            DesktopCore.withCoaxString(chat) { c in
                DesktopCore.withCoaxString(root) { rt in
                    DesktopCore.callString(4096) { out in oa_dc_wt_path(r, c, rt, out) }
                }
            }
        }
    }

    static func shown(_ branch: String) -> String {
        DesktopCore.withCoaxString(branch) { b in
            DesktopCore.callString(256) { out in oa_dc_wt_shown(b, out) }
        }
    }

    static func mainCheckout(of folder: String) -> String? {
        let s = DesktopCore.withCoaxString(folder) { f in
            DesktopCore.callString(4096) { out in oa_dc_wt_main_checkout(f, out) }
        }
        return s.isEmpty ? nil : (s as NSString).standardizingPath
    }

    enum Failure: Error, Equatable { case git(String), notARepository }

    static func create(repo: String, chat: String, root: String = ChatWorktree.root) -> Result<ChatWorktree, Failure> {
        var pBuf = [UInt8](repeating: 0, count: 4096)
        var sBuf = [UInt8](repeating: 0, count: 4096)
        var bBuf = [UInt8](repeating: 0, count: 256)
        var wBuf = [UInt8](repeating: 0, count: 2048)
        let rc: Int64 = DesktopCore.withCoaxString(repo) { r in
            DesktopCore.withCoaxString(chat) { c in
                DesktopCore.withCoaxString(root) { rt in
                    DesktopCore.withMutableBytes(&pBuf) { p in
                        DesktopCore.withMutableBytes(&sBuf) { s in
                            DesktopCore.withMutableBytes(&bBuf) { b in
                                DesktopCore.withMutableBytes(&wBuf) { w in
                                    oa_dc_wt_create(r, c, rt, p, s, b, w)
                                }
                            }
                        }
                    }
                }
            }
        }
        if rc == 0 {
            return .success(ChatWorktree(
                chat: chat,
                path: DesktopCore.cString(from: pBuf),
                source: DesktopCore.cString(from: sBuf),
                branch: DesktopCore.cString(from: bBuf)
            ))
        }
        if rc == 1 { return .failure(.notARepository) }
        return .failure(.git(DesktopCore.cString(from: wBuf)))
    }

    static func make(_ w: ChatWorktree) -> Result<ChatWorktree, Failure> {
        var pBuf = [UInt8](repeating: 0, count: 4096)
        var sBuf = [UInt8](repeating: 0, count: 4096)
        var bBuf = [UInt8](repeating: 0, count: 256)
        var wBuf = [UInt8](repeating: 0, count: 2048)
        let rc: Int64 = DesktopCore.withCoaxString(w.chat) { c in
            DesktopCore.withCoaxString(w.path) { p in
                DesktopCore.withCoaxString(w.source) { s in
                    DesktopCore.withCoaxString(w.branch) { b in
                        DesktopCore.withMutableBytes(&pBuf) { op in
                            DesktopCore.withMutableBytes(&sBuf) { os in
                                DesktopCore.withMutableBytes(&bBuf) { ob in
                                    DesktopCore.withMutableBytes(&wBuf) { ow in
                                        oa_dc_wt_make(c, p, s, b, op, os, ob, ow)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if rc == 0 {
            return .success(ChatWorktree(
                chat: w.chat,
                path: DesktopCore.cString(from: pBuf),
                source: DesktopCore.cString(from: sBuf),
                branch: DesktopCore.cString(from: bBuf)
            ))
        }
        if rc == 1 { return .failure(.notARepository) }
        return .failure(.git(DesktopCore.cString(from: wBuf)))
    }

    static func hydrate(_ w: ChatWorktree) {
        _ = DesktopCore.withCoaxString(w.chat) { c in
            DesktopCore.withCoaxString(w.path) { p in
                DesktopCore.withCoaxString(w.source) { s in
                    DesktopCore.withCoaxString(w.branch) { b in
                        oa_dc_wt_hydrate(c, p, s, b)
                    }
                }
            }
        }
    }

    static func changes(_ w: ChatWorktree) -> [String]? {
        let n: Int64 = DesktopCore.withCoaxString(w.chat) { c in
            DesktopCore.withCoaxString(w.path) { p in
                DesktopCore.withCoaxString(w.source) { s in
                    DesktopCore.withCoaxString(w.branch) { b in
                        oa_dc_wt_changes(c, p, s, b)
                    }
                }
            }
        }
        if n < 0 { return nil }
        return Array(repeating: "changed", count: Int(n))
    }

    enum Removal: Equatable { case removed, gone, kept(changes: Int) }

    static func remove(_ w: ChatWorktree) -> Removal {
        var changes: [Int64] = [-1]
        let rc: Int64 = DesktopCore.withCoaxString(w.chat) { c in
            DesktopCore.withCoaxString(w.path) { p in
                DesktopCore.withCoaxString(w.source) { s in
                    DesktopCore.withCoaxString(w.branch) { b in
                        DesktopCore.withMutableS64(&changes) { ch in
                            oa_dc_wt_remove(c, p, s, b, ch)
                        }
                    }
                }
            }
        }
        switch rc {
        case 1: return .removed
        case 2: return .gone
        default: return .kept(changes: Int(changes[0]))
        }
    }

    static func clonePaths(repo: String) -> [String] {
        var buf = [UInt8](repeating: 0, count: 65536)
        let n: Int64 = DesktopCore.withCoaxString(repo) { r in
            DesktopCore.withMutableBytes(&buf) { o in oa_dc_wt_clone_paths(r, o) }
        }
        guard n > 0 else { return [] }
        let text = String(decoding: buf.prefix(Int(n)), as: UTF8.self)
        return text.split(separator: "\n").map(String.init)
    }

    @discardableResult
    static func run(_ exe: String, _ args: [String], in dir: String? = nil) -> (status: Int32, output: String) {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: exe)
        p.arguments = args
        if let dir { p.currentDirectoryURL = URL(fileURLWithPath: dir) }
        let pipe = Pipe()
        p.standardOutput = pipe
        p.standardError = pipe
        do { try p.run() } catch { return (-1, "") }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        let out = (String(data: data, encoding: .utf8) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        return (p.terminationStatus, out)
    }

    @discardableResult
    static func git(_ args: [String], in dir: String) -> (status: Int32, output: String) {
        run("/usr/bin/git", args, in: dir)
    }
}
