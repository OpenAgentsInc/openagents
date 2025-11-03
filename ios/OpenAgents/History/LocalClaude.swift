import Foundation

struct LocalClaudeDiscovery {
    static func defaultBases() -> [URL] {
        let fm = FileManager.default
        var out: [URL] = []
        let env = ProcessInfo.processInfo.environment
        if let p = env["CLAUDE_PROJECTS_DIR"], !p.isEmpty {
            let u = URL(fileURLWithPath: p).standardizedFileURL
            if fm.fileExists(atPath: u.path) { out.append(u) }
        }
        let home = fm.homeDirectoryForCurrentUser
        let home2 = URL(fileURLWithPath: NSHomeDirectory())
        let candidates: [URL] = [
            home.appendingPathComponent(".claude/projects", isDirectory: true),
            home.appendingPathComponent(".claude/local/claude/projects", isDirectory: true),
            home.appendingPathComponent(".claude/local/projects", isDirectory: true),
            home2.appendingPathComponent(".claude/projects", isDirectory: true),
            home2.appendingPathComponent(".claude/local/claude/projects", isDirectory: true),
            home2.appendingPathComponent(".claude/local/projects", isDirectory: true),
        ]
        // Always include candidates; sandbox checks can lie on fileExists
        for c in candidates { out.append(c) }
        // Fallback: any 'projects' dir under ~/.claude
        for root in [home.appendingPathComponent(".claude", isDirectory: true), home2.appendingPathComponent(".claude", isDirectory: true)] {
          if let en = fm.enumerator(at: root, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]) {
            for case let p as URL in en {
                if p.lastPathComponent == "projects" { out.append(p) }
            }
          }
        }
        // dedupe
        var uniq: [URL] = []
        var seen: Set<String> = []
        for u in out { if !seen.contains(u.path) { uniq.append(u); seen.insert(u.path) } }
        return uniq
    }

    static func listRecentTopN(at base: URL, topK: Int) -> [URL] {
        var out: [URL] = []
        let fm = FileManager.default
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
                    if url.pathExtension.lowercased() == "jsonl" { files.append(url) }
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
                if url.pathExtension.lowercased() == "jsonl" { all.append(url) }
                visited += 1
                if visited > topK * 500 { break }
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

    static func relativeId(for url: URL, base: URL) -> String {
        let u = url.deletingPathExtension().resolvingSymlinksInPath().standardizedFileURL.path
        let b = base.resolvingSymlinksInPath().standardizedFileURL.path
        if u.hasPrefix(b + "/") { return String(u.dropFirst(b.count + 1)) }
        return url.deletingPathExtension().lastPathComponent
    }
}
