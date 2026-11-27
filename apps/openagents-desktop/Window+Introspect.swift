// Firm reason: 6 (macOS process/tmux polling, Tailscale peer discovery, and bounded read-only file tailing over page/introspect.coax; GitHub #14066).
import AppKit
import Foundation

/// Background platform-effect scanner for Universal Introspection (#14066).
/// Reconciled with #14071: external local harnesses are discovered strictly read-only
/// (bounded directory/tail reads and `ps`), never spawning external harness CLI binaries.
enum IntrospectScanner {
    static let queue = DispatchQueue(label: "openagents.coder.introspect", qos: .utility)

    static let tmuxBinary: String? = [
        "/opt/homebrew/bin/tmux",
        "/usr/local/bin/tmux",
        "/usr/bin/tmux"
    ].first { FileManager.default.isExecutableFile(atPath: $0) }

    struct LocalScanResult {
        let tmuxRaw: String
        let psRaw: String
        let harnessTsv: String
        let pathIndex: [String: String]
    }

    struct RemoteScanResult {
        let machinesTsv: String
        let remoteSessionsTsv: String
    }

    struct EnvHostSnapshot {
        let name: String
        let host: String
        let kind: Int32
        let connected: Bool
    }

    // ------------------------------------------------------------------ safe subprocess

    private static func runQuick(_ path: String, _ args: [String], timeout: TimeInterval = 2.0, limit: Int = 512 * 1024) -> String {
        let p = Process()
        let out = Pipe()
        p.executableURL = URL(fileURLWithPath: path)
        p.arguments = args
        p.standardOutput = out
        p.standardError = FileHandle.nullDevice
        p.standardInput = FileHandle.nullDevice
        var env = ProcessInfo.processInfo.environment
        env["BROWSER"] = "/usr/bin/true"
        env["SSH_CONNECTION"] = "1"
        env["TAILSCALE_BE_CLI"] = "1"
        if env["TERM"]?.isEmpty != false { env["TERM"] = "dumb" }
        p.environment = env
        do { try p.run() } catch { return "" }
        let killer = DispatchWorkItem { if p.isRunning { p.terminate() } }
        DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + timeout, execute: killer)
        let data = out.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        killer.cancel()
        guard p.terminationReason == .exit, p.terminationStatus == 0, data.count <= limit else { return "" }
        return String(decoding: data, as: UTF8.self)
    }

    private static func cleanField(_ raw: String) -> String {
        raw.replacingOccurrences(of: "\t", with: " ")
           .replacingOccurrences(of: "\r", with: " ")
           .replacingOccurrences(of: "\n", with: " ")
           .trimmingCharacters(in: .whitespaces)
    }

    // ------------------------------------------------------------------ local scan

    static func scanLocal() -> LocalScanResult {
        let tmuxRaw: String
        if let tmux = tmuxBinary {
            tmuxRaw = runQuick(tmux, [
                "list-panes", "-a", "-F",
                "#{session_name}\t#{window_index}\t#{pane_index}\t#{pane_pid}\t#{pane_current_command}\t#{pane_current_path}\t#{pane_title}\t#{pane_active}"
            ], timeout: 2.0)
        } else {
            tmuxRaw = ""
        }

        let psRaw = scanProcessTable()
        let (harnessTsv, pathIndex) = scanExternalHarnessCatalog()
        return LocalScanResult(tmuxRaw: tmuxRaw, psRaw: psRaw, harnessTsv: harnessTsv, pathIndex: pathIndex)
    }

    /// Formats `/bin/ps` into TSV rows `pid\ttty\tstat\tcomm\targs\tcwd\n` parsed by `is_parse_process_rows`.
    private static func scanProcessTable() -> String {
        let raw = runQuick("/bin/ps", ["-eo", "pid=,tty=,stat=,comm=,args="], timeout: 2.0)
        guard !raw.isEmpty else { return "" }
        var lines: [String] = []
        lines.reserveCapacity(64)
        for line in raw.split(whereSeparator: \.isNewline) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard !trimmed.isEmpty else { continue }
            // Split first 4 whitespace-separated columns (pid, tty, stat, comm) and keep rest as args.
            var cols: [Substring] = []
            var idx = trimmed.startIndex
            while cols.count < 4 && idx < trimmed.endIndex {
                while idx < trimmed.endIndex && trimmed[idx].isWhitespace { idx = trimmed.index(after: idx) }
                guard idx < trimmed.endIndex else { break }
                let start = idx
                while idx < trimmed.endIndex && !trimmed[idx].isWhitespace { idx = trimmed.index(after: idx) }
                cols.append(trimmed[start..<idx])
            }
            guard cols.count >= 4 else { continue }
            while idx < trimmed.endIndex && trimmed[idx].isWhitespace { idx = trimmed.index(after: idx) }
            let args = idx < trimmed.endIndex ? String(trimmed[idx...]) : String(cols[3])
            // Fast pre-filter in Swift to keep TSV compact (Coax parser still does full classification).
            let lc = (String(cols[3]) + " " + args).lowercased()
            let interesting = lc.contains("claude") || lc.contains("codex") || lc.contains("agy") ||
                              lc.contains("antigravity") || lc.contains("cursor") || lc.contains("opencode") ||
                              lc.contains("inky") || lc.contains("coder") || lc.contains("openagents")
            guard interesting else { continue }
            lines.append("\(cols[0])\t\(cols[1])\t\(cols[2])\t\(cleanField(String(cols[3])))\t\(cleanField(args))\t")
        }
        return lines.joined(separator: "\n") + (lines.isEmpty ? "" : "\n")
    }

    /// Strictly read-only, bounded inspection of known external harness session folders.
    /// Never walks outside these explicit paths and never spawns external harness binaries (#14071).
    private static func scanExternalHarnessCatalog() -> (String, [String: String]) {
        let fm = FileManager.default
        let home = fm.homeDirectoryForCurrentUser
        var rows: [(updatedMs: Int64, line: String, uri: String, path: String)] = []

        // 1. Claude Code transcripts under ~/.claude/projects/<encoded-cwd>/<session>.jsonl (bounded)
        let claudeRoot = home.appendingPathComponent(".claude/projects", isDirectory: true)
        if let projDirs = boundedRecentChildren(of: claudeRoot, maxCount: 10, wantDirectories: true) {
            for pdir in projDirs {
                if let files = boundedRecentChildren(of: pdir.url, maxCount: 3, wantDirectories: false, suffix: ".jsonl") {
                    for f in files {
                        let sid = f.url.deletingPathExtension().lastPathComponent
                        guard !sid.isEmpty, !sid.hasPrefix("agent-") else { continue }
                        let meta = extractJsonlMeta(url: f.url, fallbackCwd: decodeClaudeProjectDir(pdir.url.lastPathComponent))
                        let uri = "local:harness:claude:\(sid)"
                        let status = (Date().timeIntervalSince(f.mtime) < 90.0) ? "running" : "idle"
                        let title = meta.title.isEmpty ? "Claude \(String(sid.prefix(8)))" : meta.title
                        let ms = Int64(f.mtime.timeIntervalSince1970 * 1000)
                        let line = "claude\t\(cleanField(sid))\t\(cleanField(title))\t\(cleanField(meta.cwd))\t\(cleanField(meta.branch))\t\(status)\t\(ms)\t0\t\(cleanField(meta.preview))"
                        rows.append((ms, line, uri, f.url.path))
                    }
                }
            }
        }

        // 2. Codex CLI transcripts under ~/.codex/sessions (bounded depth <= 3)
        let codexRoot = home.appendingPathComponent(".codex/sessions", isDirectory: true)
        for f in boundedRecentJsonlTree(root: codexRoot, maxDepth: 3, maxFiles: 8) {
            let sid = f.url.deletingPathExtension().lastPathComponent
            guard !sid.isEmpty else { continue }
            let meta = extractJsonlMeta(url: f.url, fallbackCwd: "")
            let uri = "local:harness:codex:\(sid)"
            let status = (Date().timeIntervalSince(f.mtime) < 90.0) ? "running" : "idle"
            let title = meta.title.isEmpty ? "Codex \(String(sid.prefix(12)))" : meta.title
            let ms = Int64(f.mtime.timeIntervalSince1970 * 1000)
            let line = "codex\t\(cleanField(sid))\t\(cleanField(title))\t\(cleanField(meta.cwd))\t\(cleanField(meta.branch))\t\(status)\t\(ms)\t0\t\(cleanField(meta.preview))"
            rows.append((ms, line, uri, f.url.path))
        }

        // 3. Antigravity conversations under ~/.gemini/antigravity/conversations (bounded)
        let agyRoot = home.appendingPathComponent(".gemini/antigravity/conversations", isDirectory: true)
        if let convs = boundedRecentChildren(of: agyRoot, maxCount: 8, wantDirectories: nil) {
            for c in convs {
                let sid = c.url.deletingPathExtension().lastPathComponent
                guard !sid.isEmpty, !sid.hasPrefix(".") else { continue }
                let uri = "local:harness:agy:\(sid)"
                let status = (Date().timeIntervalSince(c.mtime) < 90.0) ? "running" : "idle"
                let ms = Int64(c.mtime.timeIntervalSince1970 * 1000)
                let line = "agy\t\(cleanField(sid))\tAntigravity \(cleanField(String(sid.prefix(12))))\t\t\t\(status)\t\(ms)\t0\tConversation artifact"
                rows.append((ms, line, uri, c.url.path))
            }
        }

        rows.sort { $0.updatedMs > $1.updatedMs }
        let capped = Array(rows.prefix(18))
        var idx: [String: String] = [:]
        for r in capped { idx[r.uri] = r.path }
        let tsv = capped.map(\.line).joined(separator: "\n") + (capped.isEmpty ? "" : "\n")
        return (tsv, idx)
    }

    private struct DirEntry {
        let url: URL
        let mtime: Date
    }

    private static func boundedRecentChildren(of dir: URL, maxCount: Int, wantDirectories: Bool?, suffix: String? = nil) -> [DirEntry]? {
        let fm = FileManager.default
        guard let urls = try? fm.contentsOfDirectory(
            at: dir,
            includingPropertiesForKeys: [.contentModificationDateKey, .isDirectoryKey],
            options: [.skipsHiddenFiles]
        ) else { return nil }
        var out: [DirEntry] = []
        for u in urls.prefix(200) {
            if let suffix, !u.lastPathComponent.hasSuffix(suffix) { continue }
            guard let vals = try? u.resourceValues(forKeys: [.contentModificationDateKey, .isDirectoryKey]) else { continue }
            if let wantDir = wantDirectories, (vals.isDirectory ?? false) != wantDir { continue }
            out.append(DirEntry(url: u, mtime: vals.contentModificationDate ?? .distantPast))
        }
        out.sort { $0.mtime > $1.mtime }
        return Array(out.prefix(maxCount))
    }

    private static func boundedRecentJsonlTree(root: URL, maxDepth: Int, maxFiles: Int) -> [DirEntry] {
        var collected: [DirEntry] = []
        func walk(_ dir: URL, depth: Int) {
            if depth > maxDepth || collected.count >= maxFiles * 3 { return }
            guard let entries = boundedRecentChildren(of: dir, maxCount: 6, wantDirectories: nil) else { return }
            for e in entries {
                var isDir: ObjCBool = false
                if FileManager.default.fileExists(atPath: e.url.path, isDirectory: &isDir), isDir.boolValue {
                    if depth < maxDepth { walk(e.url, depth: depth + 1) }
                } else if e.url.lastPathComponent.hasSuffix(".jsonl") {
                    collected.append(e)
                }
            }
        }
        walk(root, depth: 0)
        collected.sort { $0.mtime > $1.mtime }
        return Array(collected.prefix(maxFiles))
    }

    private static func decodeClaudeProjectDir(_ slug: String) -> String {
        guard slug.hasPrefix("-") else { return slug }
        return "/" + slug.dropFirst().replacingOccurrences(of: "-", with: "/")
    }

    private static func extractJsonlMeta(url: URL, fallbackCwd: String) -> (title: String, cwd: String, branch: String, preview: String) {
        let head = readBoundedSlice(url: url, fromEnd: false, maxBytes: 8192)
        let tail = readBoundedSlice(url: url, fromEnd: true, maxBytes: 8192)
        var title = ""
        var cwd = fallbackCwd
        var branch = ""
        var preview = ""
        for chunk in [head, tail] {
            for line in chunk.split(whereSeparator: \.isNewline) {
                guard line.first == "{",
                      let data = String(line).data(using: .utf8),
                      let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { continue }
                if let c = obj["cwd"] as? String, !c.isEmpty { cwd = c }
                if let b = obj["gitBranch"] as? String, !b.isEmpty { branch = b }
                if let text = extractMessageText(from: obj), !text.isEmpty {
                    if title.isEmpty { title = String(text.prefix(80)) }
                    preview = String(text.prefix(120))
                }
            }
        }
        return (title, cwd, branch, preview)
    }

    private static func extractMessageText(from obj: [String: Any]) -> String? {
        if let msg = obj["message"] as? [String: Any] {
            if let s = msg["content"] as? String, !s.isEmpty { return cleanField(s) }
            if let arr = msg["content"] as? [[String: Any]] {
                for item in arr {
                    if let t = item["text"] as? String, !t.isEmpty { return cleanField(t) }
                }
            }
        }
        if let payload = obj["payload"] as? [String: Any] {
            if let s = payload["message"] as? String, !s.isEmpty { return cleanField(s) }
            if let content = payload["content"] as? [[String: Any]] {
                for item in content {
                    if let t = item["text"] as? String, !t.isEmpty { return cleanField(t) }
                }
            }
        }
        if let s = obj["summary"] as? String, !s.isEmpty { return cleanField(s) }
        return nil
    }

    private static func readBoundedSlice(url: URL, fromEnd: Bool, maxBytes: Int) -> String {
        guard let fh = try? FileHandle(forReadingFrom: url) else { return "" }
        defer { try? fh.close() }
        let fileSize = (try? fh.seekToEnd()) ?? 0
        if fileSize == 0 { return "" }
        let count = min(UInt64(maxBytes), fileSize)
        let offset: UInt64 = fromEnd ? (fileSize - count) : 0
        try? fh.seek(toOffset: offset)
        guard let data = try? fh.read(upToCount: Int(count)) else { return "" }
        var text = String(decoding: data, as: UTF8.self)
        if fromEnd && offset > 0, let nl = text.firstIndex(of: "\n") {
            text = String(text[text.index(after: nl)...])
        }
        return text
    }

    // ------------------------------------------------------------------ remote machines scan

    static func scanRemoteMachines(envHosts: [EnvHostSnapshot]) -> RemoteScanResult {
        var rows: [String] = []
        var seen: Set<String> = []

        // 1. Tailscale peers from `tailscale status --json` (3s hard timeout)
        if let tsCli = CoderTailnet.cli,
           let data = CoderTailnet.output(tsCli, ["status", "--json"], timeout: 3.0),
           let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let peers = root["Peer"] as? [String: Any] {
            for (_, val) in peers {
                guard let peer = val as? [String: Any] else { continue }
                let hostName = cleanField((peer["HostName"] as? String) ?? "")
                let dnsRaw = cleanField((peer["DNSName"] as? String) ?? "")
                let dns = dnsRaw.hasSuffix(".") ? String(dnsRaw.dropLast()) : dnsRaw
                let ips = (peer["TailscaleIPs"] as? [String]) ?? []
                let ip = ips.first ?? dns
                let id = !hostName.isEmpty ? hostName : (!dns.isEmpty ? dns : ip)
                guard !id.isEmpty, !seen.contains(id.lowercased()) else { continue }
                seen.insert(id.lowercased())
                let online = (peer["Online"] as? Bool) ?? false
                let os = cleanField((peer["OS"] as? String) ?? "")
                let status = online ? "online" : "offline"
                let addr = !ip.isEmpty ? ip : dns
                rows.append("\(id)\t\(hostName.isEmpty ? id : hostName)\t\(addr)\t\t22\ttailscale\t\(status)\t0\t0\t\(os)")
            }
        }

        // 2. Configured SSH / Boat / WSL environments from page/environments.coax
        for h in envHosts {
            let key = h.name.lowercased()
            guard !h.name.isEmpty, !seen.contains(key) else { continue }
            seen.insert(key)
            let transport: String
            switch h.kind {
            case 2: transport = "boat"
            case 3: transport = "wsl"
            default: transport = "ssh"
            }
            let status = h.connected ? "online" : "offline"
            rows.append("\(cleanField(h.name))\t\(cleanField(h.name))\t\(cleanField(h.host))\t\t22\t\(transport)\t\(status)\t0\t0\t")
        }

        // 3. Optional remote sessions cache written by connected remote coder-host daemon
        let remoteCacheURL = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/desktop/remote-sessions.tsv")
        let remoteSessionsTsv = (try? String(contentsOf: remoteCacheURL, encoding: .utf8)) ?? ""

        let machinesTsv = rows.prefix(16).joined(separator: "\n") + (rows.isEmpty ? "" : "\n")
        return RemoteScanResult(machinesTsv: machinesTsv, remoteSessionsTsv: remoteSessionsTsv)
    }

    // ------------------------------------------------------------------ live output capture

    static func captureOutput(uri: String, pathIndex: [String: String]) -> (output: String, status: String) {
        if uri.hasPrefix("local:tmux:") {
            let target = String(uri.dropFirst("local:tmux:".count))
            guard let tmux = tmuxBinary else {
                return ("tmux binary not found on PATH.", "tmux unavailable")
            }
            let out = runQuick(tmux, ["capture-pane", "-p", "-S", "-200", "-t", target], timeout: 2.0)
            let trimmed = trimTrailingEmptyLines(out)
            let lineCount = trimmed.isEmpty ? 0 : trimmed.split(separator: "\n", omittingEmptySubsequences: false).count
            return (trimmed.isEmpty ? "(pane is empty)" : trimmed, "Live · tmux capture-pane (\(lineCount) lines)")
        }

        if uri.hasPrefix("local:tui:") {
            let pid = String(uri.dropFirst("local:tui:".count))
            let psInfo = runQuick("/bin/ps", ["-p", pid, "-o", "pid=,ppid=,tty=,stat=,%cpu=,%mem=,etime=,args="], timeout: 2.0)
            let hbURL = FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent(".openagents/tui/sessions/\(pid).json")
            var sections: [String] = []
            if !psInfo.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                sections.append("PID   PPID TTY      STAT %CPU %MEM ELAPSED COMMAND\n" + psInfo.trimmingCharacters(in: .whitespacesAndNewlines))
            }
            if let hb = try? String(contentsOf: hbURL, encoding: .utf8), !hb.isEmpty {
                sections.append("Heartbeat (~/.openagents/tui/sessions/\(pid).json):\n" + String(hb.prefix(8192)))
            }
            if sections.isEmpty {
                return ("Process \(pid) has exited.", "Process exited")
            }
            return (sections.joined(separator: "\n\n"), "Live · local TUI process (PID \(pid))")
        }

        if uri.hasPrefix("local:harness:") {
            if let filePath = pathIndex[uri] {
                let url = URL(fileURLWithPath: filePath)
                var isDir: ObjCBool = false
                if FileManager.default.fileExists(atPath: filePath, isDirectory: &isDir), !isDir.boolValue {
                    let tail = readBoundedSlice(url: url, fromEnd: true, maxBytes: 16384)
                    let formatted = formatTranscriptTail(tail, fileName: url.lastPathComponent)
                    return (formatted, "Read-only tail · \(url.lastPathComponent)")
                }
                return ("Session artifact path: \(filePath)", "Read-only · artifact directory")
            }
            // Fallback for PID-based harness process: local:harness:<h>:pid-<N>
            if let r = uri.range(of: ":pid-") {
                let pid = String(uri[r.upperBound...])
                let psInfo = runQuick("/bin/ps", ["-p", pid, "-o", "pid=,ppid=,tty=,stat=,%cpu=,%mem=,etime=,args="], timeout: 2.0)
                if !psInfo.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    return ("PID   PPID TTY      STAT %CPU %MEM ELAPSED COMMAND\n" + psInfo.trimmingCharacters(in: .whitespacesAndNewlines),
                            "Read-only · live process (PID \(pid))")
                }
            }
            return ("No transcript file available for \(uri).", "Read-only")
        }

        if uri.hasPrefix("remote:") {
            return ("Remote session: \(uri)\nConnect to this machine in Settings -> Connections to stream live remote output.", "Remote session")
        }

        return ("", "Ready")
    }

    private static func trimTrailingEmptyLines(_ raw: String) -> String {
        var lines = raw.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        while let last = lines.last, last.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            lines.removeLast()
        }
        return lines.joined(separator: "\n")
    }

    private static func formatTranscriptTail(_ raw: String, fileName: String) -> String {
        guard fileName.hasSuffix(".jsonl") else {
            return raw.isEmpty ? "(empty file)" : raw
        }
        var out: [String] = []
        for line in raw.split(whereSeparator: \.isNewline) {
            guard line.first == "{",
                  let data = String(line).data(using: .utf8),
                  let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { continue }
            let role = (obj["type"] as? String) ?? ((obj["message"] as? [String: Any])?["role"] as? String) ?? "event"
            if let text = extractMessageText(from: obj), !text.isEmpty {
                out.append("[\(role)] \(text)")
            }
        }
        if out.isEmpty {
            return raw.isEmpty ? "(no transcript events yet)" : String(raw.suffix(4096))
        }
        return out.suffix(120).joined(separator: "\n")
    }
}

extension CoderWindow {
    /// Starts periodic local (3s) and remote (10s) introspection discovery timers (#14066).
    func setupIntrospection() {
        guard !TestLaunch.active else { return }
        refreshIntrospectLocal()
        refreshIntrospectRemote()

        introspectLocalTimer?.invalidate()
        introspectLocalTimer = Timer.scheduledTimer(withTimeInterval: 3.0, repeats: true) { [weak self] _ in
            guard let self else { return }
            self.refreshIntrospectLocal()
            if cd_introspect_shown(self.page) != 0 && cd_introspect_active_is_machine(self.page) == 0 {
                self.refreshInspectedOutput()
            }
        }

        introspectRemoteTimer?.invalidate()
        introspectRemoteTimer = Timer.scheduledTimer(withTimeInterval: 10.0, repeats: true) { [weak self] _ in
            self?.refreshIntrospectRemote()
        }
    }

    func retireIntrospection() {
        introspectLocalTimer?.invalidate()
        introspectLocalTimer = nil
        introspectRemoteTimer?.invalidate()
        introspectRemoteTimer = nil
    }

    func refreshIntrospectLocal() {
        guard !introspectLocalBusy else { return }
        introspectLocalBusy = true
        let allowExt: Int32 = AgentSettings.allowLocalHarnesses ? 1 : 0
        IntrospectScanner.queue.async { [weak self] in
            let res = IntrospectScanner.scanLocal()
            DispatchQueue.main.async {
                guard let self, !self.retired else { return }
                self.introspectLocalBusy = false
                self.introspectPathIndex = res.pathIndex
                _ = cd_introspect_set_tui(self.page, res.tmuxRaw, res.psRaw, allowExt)
                _ = cd_introspect_set_harnesses(self.page, res.harnessTsv, res.psRaw, allowExt)
                self.wake()
            }
        }
    }

    func refreshIntrospectRemote() {
        guard !introspectRemoteBusy else { return }
        introspectRemoteBusy = true

        // Snapshot configured SSH / Boat / WSL environments on the main thread before background scan.
        var envHosts: [IntrospectScanner.EnvHostSnapshot] = []
        let count = cd_env_count(page)
        let connectedAlias = RemoteEnvironmentTunnel.shared.activeAlias
        for i in 1..<count {
            var nbuf = [CChar](repeating: 0, count: 128)
            var hbuf = [CChar](repeating: 0, count: 256)
            let nn = cd_env_name(page, i, &nbuf, Int32(nbuf.count))
            let hn = cd_env_host(page, i, &hbuf, Int32(hbuf.count))
            let name = nn > 0 ? String(cString: nbuf) : ""
            let host = hn > 0 ? String(cString: hbuf) : name
            let kind = cd_env_kind(page, i)
            if !name.isEmpty {
                envHosts.append(.init(name: name, host: host, kind: kind, connected: connectedAlias == name))
            }
        }

        IntrospectScanner.queue.async { [weak self] in
            let res = IntrospectScanner.scanRemoteMachines(envHosts: envHosts)
            DispatchQueue.main.async {
                guard let self, !self.retired else { return }
                self.introspectRemoteBusy = false
                _ = cd_introspect_set_machines(self.page, res.machinesTsv, res.remoteSessionsTsv)
                self.wake()
            }
        }
    }

    func refreshInspectedOutput() {
        guard !introspectOutputBusy else { return }
        var idBuf = [CChar](repeating: 0, count: 256)
        guard cd_introspect_active_id(page, &idBuf, Int32(idBuf.count)) > 0 else { return }
        let activeID = String(cString: idBuf)
        if cd_introspect_active_is_machine(page) != 0 {
            refreshIntrospectRemote()
            return
        }
        introspectOutputBusy = true
        let paths = introspectPathIndex
        IntrospectScanner.queue.async { [weak self] in
            let res = IntrospectScanner.captureOutput(uri: activeID, pathIndex: paths)
            DispatchQueue.main.async {
                guard let self, !self.retired else { return }
                self.introspectOutputBusy = false
                _ = cd_introspect_set_output(self.page, activeID, res.output, res.status, 0)
                self.wake()
            }
        }
    }

    /// Handles CP_ASK_INTROSPECT_* (250..254) from page/introspect.coax.
    func introspectAsked(_ what: Int32) {
        switch what {
        case 250: // CP_ASK_INTROSPECT_REFRESH
            refreshIntrospectLocal()
            refreshIntrospectRemote()
        case 251: // CP_ASK_INTROSPECT_INSPECT
            refreshInspectedOutput()
        case 252: // CP_ASK_INTROSPECT_SEND
            break
        case 253: // CP_ASK_INTROSPECT_CONNECT
            syncEnvironments()
            cd_settings_open(page, 10, "") // CP_PG_CONNECTIONS
            wake()
        case 254: // CP_ASK_INTROSPECT_COPY
            var outBuf = [CChar](repeating: 0, count: 16385)
            let n = cd_introspect_output(page, &outBuf, Int32(outBuf.count))
            if n > 0 {
                let text = String(cString: outBuf)
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(text, forType: .string)
                cd_toast(page, "Copied live output", 0)
                wake()
            }
        default:
            break
        }
    }
}
