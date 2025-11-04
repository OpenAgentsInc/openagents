import Foundation
import Darwin

struct LocalClaudeDiscovery {
    static func defaultBases() -> [URL] {
        let fm = FileManager.default
        var out: [URL] = []
        let env = ProcessInfo.processInfo.environment
        if let p = env["CLAUDE_PROJECTS_DIR"], !p.isEmpty {
            let u = URL(fileURLWithPath: p).standardizedFileURL
            if fm.fileExists(atPath: u.path) { out.append(u) }
        }
        // Hardcoded project path (requested): scan the repo and its claude logs
        let hardProject = URL(fileURLWithPath: "/Users/christopherdavid/code/openagents", isDirectory: true)
        let hardProjectClaude = hardProject.appendingPathComponent("docs/logs/claude", isDirectory: true)
        out.append(hardProject)
        out.append(hardProjectClaude)
        if let extra = env["CLAUDE_EXTRA_DIRS"], !extra.isEmpty {
            for seg in extra.split(separator: ":") {
                out.append(URL(fileURLWithPath: String(seg)).standardizedFileURL)
            }
        }
        #if os(macOS)
        let home = fm.homeDirectoryForCurrentUser
        let home2 = URL(fileURLWithPath: NSHomeDirectory())
        let realHome = URL(fileURLWithPath: "/Users/\(NSUserName())", isDirectory: true)
        let candidates: [URL] = [
            home.appendingPathComponent(".claude/projects", isDirectory: true),
            home.appendingPathComponent(".claude/local/claude/projects", isDirectory: true),
            home.appendingPathComponent(".claude/local/projects", isDirectory: true),
            home.appendingPathComponent(".claude/local/claude", isDirectory: true),
            home.appendingPathComponent(".claude/local", isDirectory: true),
            home.appendingPathComponent(".claude", isDirectory: true),
            home2.appendingPathComponent(".claude/projects", isDirectory: true),
            home2.appendingPathComponent(".claude/local/claude/projects", isDirectory: true),
            home2.appendingPathComponent(".claude/local/projects", isDirectory: true),
            home2.appendingPathComponent(".claude/local/claude", isDirectory: true),
            home2.appendingPathComponent(".claude/local", isDirectory: true),
            home2.appendingPathComponent(".claude", isDirectory: true),
            realHome.appendingPathComponent(".claude/projects", isDirectory: true),
            realHome.appendingPathComponent(".claude/local/claude/projects", isDirectory: true),
            realHome.appendingPathComponent(".claude/local/projects", isDirectory: true),
            realHome.appendingPathComponent(".claude/local/claude", isDirectory: true),
            realHome.appendingPathComponent(".claude/local", isDirectory: true),
            realHome.appendingPathComponent(".claude", isDirectory: true),
        ]
        for c in candidates { out.append(c) }
        for root in [home.appendingPathComponent(".claude", isDirectory: true), home2.appendingPathComponent(".claude", isDirectory: true), realHome.appendingPathComponent(".claude", isDirectory: true)] {
          if let en = fm.enumerator(at: root, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]) {
            for case let p as URL in en { if p.lastPathComponent == "projects" { out.append(p) } }
          }
        }
        let exactProject = realHome.appendingPathComponent(".claude/projects/-Users-christopherdavid-code-openagents", isDirectory: true)
        out.append(exactProject)
        #endif
        // dedupe
        var uniq: [URL] = []
        var seen: Set<String> = []
        for u in out { if !seen.contains(u.path) { uniq.append(u); seen.insert(u.path) } }
        return uniq
    }

    static func listRecentTopN(at base: URL, topK: Int) -> [URL] {
        var out: [URL] = []
        let fm = FileManager.default
        // If base itself contains transcript files, pick directly
        if let direct = try? fm.contentsOfDirectory(at: base, includingPropertiesForKeys: [.contentModificationDateKey], options: [.skipsHiddenFiles]) {
            var directFiles = direct.filter { let e = $0.pathExtension.lowercased(); return e == "jsonl" || e == "json" }
            if !directFiles.isEmpty {
                directFiles.sort { fileMTime($0) > fileMTime($1) }
                if directFiles.count > topK { directFiles = Array(directFiles.prefix(topK)) }
                return directFiles
            }
        }
        // Heuristic: projects/<project>/**/*.jsonl
        let projects = (try? fm.contentsOfDirectory(at: base, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]))?.filter { (try? $0.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true } ?? []
        let pSorted = projects.sorted { $0.lastPathComponent > $1.lastPathComponent }
        for p in pSorted {
            // shallow scan for jsonl; if many, pick newest
            var files: [URL] = []
            if let en = fm.enumerator(at: p, includingPropertiesForKeys: [.isDirectoryKey, .contentModificationDateKey], options: [.skipsHiddenFiles]) {
                var visited = 0
                for case let url as URL in en {
                    if (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true { continue }
                    let ext = url.pathExtension.lowercased()
                    if ext == "jsonl" || ext == "json" { files.append(url) }
                    visited += 1
                    if visited > topK * 200 { break }
                }
            }
            files.sort { fileMTime($0) > fileMTime($1) }
            for f in files { out.append(f); if out.count >= topK { return out } }
        }
        // fallback: deep enumerator capped
        var all: [URL] = []
        if let en = fm.enumerator(at: base, includingPropertiesForKeys: [.isDirectoryKey, .contentModificationDateKey], options: [.skipsHiddenFiles]) {
            var visited = 0
            for case let url as URL in en {
                if (try? url.resourceValues(forKeys: [.isDirectoryKey]).isDirectory) == true { continue }
                let ext = url.pathExtension.lowercased()
                if ext == "jsonl" || ext == "json" { all.append(url) }
                visited += 1
                if visited > topK * 500 { break }
            }
        }
        if all.isEmpty {
            // Fallback using enumerator(atPath:) to avoid URL resource keys
            if let en = fm.enumerator(atPath: base.path) {
                var visited = 0
                for case let rel as String in en {
                    let ext = (rel as NSString).pathExtension.lowercased()
                    if ext == "jsonl" || ext == "json" {
                        all.append(base.appendingPathComponent(rel))
                    }
                    visited += 1
                    if visited > topK * 2000 { break }
                }
            }
        }
        all.sort { fileMTime($0) > fileMTime($1) }
        if all.count > topK { all = Array(all.prefix(topK)) }
        return all
    }

    static func fileMTime(_ url: URL) -> Int64 {
        if let m = (try? FileManager.default.attributesOfItem(atPath: url.path))?[.modificationDate] as? Date { return Int64(m.timeIntervalSince1970 * 1000) }
        return Int64(Date().timeIntervalSince1970 * 1000)
    }

    static func makeSummary(for url: URL, base: URL?) -> LocalThreadSummary {
        let updated = fileMTime(url)
        let id: String = {
            if let b = base { return relativeId(for: url, base: b) }
            return url.deletingPathExtension().lastPathComponent
        }()
        return LocalThreadSummary(id: id, title: nil, source: "claude_code", created_at: nil, updated_at: updated, last_message_ts: nil, message_count: nil)
    }

    static func scanExactProjectTopK(topK: Int = 10) -> [LocalThreadSummary] {
        let base = URL(fileURLWithPath: "/Users/christopherdavid/.claude/projects/-Users-christopherdavid-code-openagents", isDirectory: true)
        // POSIX direct listing first
        var urls = posixListFiles(in: base)
        if urls.isEmpty {
            urls = listRecentTopN(at: base, topK: topK)
        }
        if urls.isEmpty {
            // deep fallback
            urls = []
            if let en = FileManager.default.enumerator(atPath: base.path) {
                var all: [URL] = []
                for case let rel as String in en {
                    let ext = (rel as NSString).pathExtension.lowercased()
                    if ext == "jsonl" || ext == "json" { all.append(base.appendingPathComponent(rel)) }
                    if all.count > topK * 50 { break }
                }
                all.sort { fileMTime($0) > fileMTime($1) }
                if all.count > topK { urls = Array(all.prefix(topK)) } else { urls = all }
            }
        }
        return urls.map { makeSummary(for: $0, base: base) }
    }

    static func posixListFiles(in base: URL) -> [URL] {
        var out: [URL] = []
        let cPath = base.path.cString(using: .utf8)!
        guard let dir = opendir(cPath) else { return out }
        defer { closedir(dir) }
        while let ent = readdir(dir) {
            let name: String = withUnsafePointer(to: ent.pointee.d_name) { rawPtr in
                rawPtr.withMemoryRebound(to: CChar.self, capacity: 256) {
                    String(cString: $0)
                }
            }
            if name == "." || name == ".." { continue }
            let full = base.appendingPathComponent(name)
            var st = stat()
            if lstat(full.path, &st) == 0 {
                #if os(macOS)
                if (st.st_mode & S_IFMT) == S_IFREG {
                    let ext = full.pathExtension.lowercased()
                    if ext == "jsonl" || ext == "json" { out.append(full) }
                }
                #endif
            }
        }
        out.sort { fileMTime($0) > fileMTime($1) }
        if out.count > 10 { out = Array(out.prefix(10)) }
        return out
    }

    static func relativeId(for url: URL, base: URL) -> String {
        let u = url.deletingPathExtension().resolvingSymlinksInPath().standardizedFileURL.path
        let b = base.resolvingSymlinksInPath().standardizedFileURL.path
        if u.hasPrefix(b + "/") { return String(u.dropFirst(b.count + 1)) }
        return url.deletingPathExtension().lastPathComponent
    }
}
