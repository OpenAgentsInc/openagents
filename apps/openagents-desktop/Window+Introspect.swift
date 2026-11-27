// Firm reason: 6 (macOS local OpenAgents TUI/tmux inspection over page/introspect.coax; GitHub #14066).
import AppKit
import Foundation

/// On-demand platform-effect scanner for local OpenAgents TUI/tmux sessions (#14066).
/// Never polls external third-party harnesses or mesh peer networks in the background.
enum IntrospectScanner {
    static let queue = DispatchQueue(label: "openagents.coder.introspect", qos: .utility)

    static let tmuxBinary: String? = [
        "/opt/homebrew/bin/tmux",
        "/usr/local/bin/tmux",
        "/usr/bin/tmux"
    ].first { FileManager.default.isExecutableFile(atPath: $0) }

    struct SessionRecord {
        let id: String
        let kind: String
        let harness: String
        let machineID: String
        let machineLabel: String
        let title: String
        let cwd: String
        let branch: String
        let status: String
        let updatedMs: Int64
        let pid: Int
        let preview: String
        let isExternalHarness: Bool
        let localCommandable: Bool

        func asDictionary(allowExternal: Bool) -> [String: Any] {
            [
                "id": id,
                "kind": kind,
                "harness": harness,
                "machine_id": machineID,
                "machine_label": machineLabel,
                "title": title,
                "cwd": cwd,
                "branch": branch,
                "status": status,
                "updated_ms": updatedMs,
                "pid": pid,
                "preview": preview,
                "commandable": isExternalHarness ? allowExternal : localCommandable
            ]
        }
    }

    struct MachineRecord {
        let id: String
        let label: String
        let host: String
        let user: String
        let port: Int
        let transport: String
        let status: String
        let os: String

        func asDictionary() -> [String: Any] {
            [
                "id": id,
                "label": label,
                "host": host,
                "user": user,
                "port": port,
                "transport": transport,
                "status": status,
                "os": os
            ]
        }

        var sshDestination: String {
            user.isEmpty ? host : "\(user)@\(host)"
        }
    }

    struct LocalScanResult {
        let tmuxRaw: String
        let psRaw: String
        let harnessTsv: String
        let pathIndex: [String: String]
        let tuiSessions: [SessionRecord]
        let harnessSessions: [SessionRecord]
    }

    struct RemoteScanResult {
        let machinesTsv: String
        let remoteSessionsTsv: String
        let machines: [MachineRecord]
        let remoteSessions: [SessionRecord]
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

    private static func runQuickWithStatus(_ path: String, _ args: [String], timeout: TimeInterval = 5.0, limit: Int = 512 * 1024) -> (ok: Bool, code: Int32, stdout: String, stderr: String) {
        let p = Process()
        let out = Pipe()
        let err = Pipe()
        p.executableURL = URL(fileURLWithPath: path)
        p.arguments = args
        p.standardOutput = out
        p.standardError = err
        p.standardInput = FileHandle.nullDevice
        var env = ProcessInfo.processInfo.environment
        env["BROWSER"] = "/usr/bin/true"
        env["SSH_CONNECTION"] = "1"
        if env["TERM"]?.isEmpty != false { env["TERM"] = "dumb" }
        p.environment = env
        do { try p.run() } catch { return (false, -1, "", error.localizedDescription) }
        let killer = DispatchWorkItem { if p.isRunning { p.terminate() } }
        DispatchQueue.global(qos: .utility).asyncAfter(deadline: .now() + timeout, execute: killer)
        let outData = out.fileHandleForReading.readDataToEndOfFile()
        let errData = err.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        killer.cancel()
        let outStr = String(decoding: outData.prefix(limit), as: UTF8.self)
        let errStr = String(decoding: errData.prefix(limit), as: UTF8.self)
        return (p.terminationReason == .exit && p.terminationStatus == 0, p.terminationStatus, outStr, errStr)
    }

    private static func resolveExecutable(_ name: String) -> String? {
        let candidates = [
            "/opt/homebrew/bin/\(name)",
            "/usr/local/bin/\(name)",
            "/usr/bin/\(name)",
            FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".local/bin/\(name)").path,
            FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/bin/\(name)").path
        ]
        if let found = candidates.first(where: { FileManager.default.isExecutableFile(atPath: $0) }) { return found }
        if let pathEnv = ProcessInfo.processInfo.environment["PATH"] {
            for dir in pathEnv.split(separator: ":") {
                let p = String(dir) + "/" + name
                if FileManager.default.isExecutableFile(atPath: p) { return p }
            }
        }
        return nil
    }

    private static func shellQuote(_ s: String) -> String {
        "'" + s.replacingOccurrences(of: "'", with: "'\\''") + "'"
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
        let tuiRecords = buildTuiRecords(tmuxRaw: tmuxRaw, psRaw: psRaw)
        return LocalScanResult(
            tmuxRaw: tmuxRaw,
            psRaw: psRaw,
            harnessTsv: "",
            pathIndex: [:],
            tuiSessions: tuiRecords,
            harnessSessions: []
        )
    }

    private static func buildTuiRecords(tmuxRaw: String, psRaw: String) -> [SessionRecord] {
        var out: [SessionRecord] = []
        var seenPids: Set<Int> = []
        for line in tmuxRaw.split(whereSeparator: \.isNewline) {
            let cols = line.split(separator: "\t", omittingEmptySubsequences: false).map(String.init)
            guard cols.count >= 6 else { continue }
            let target = "\(cols[0]):\(cols[1]).\(cols[2])"
            let pid = Int(cols[3]) ?? 0
            if pid > 0 { seenPids.insert(pid) }
            let cmd = cols[4]
            let cwd = cols[5]
            let ptitle = cols.count > 6 ? cols[6] : ""
            let plainShells: Set<String> = ["zsh", "bash", "sh", "fish", "dash", "login", "tmux"]
            let base = URL(fileURLWithPath: cmd).lastPathComponent.lowercased()
            let st = (!base.isEmpty && !plainShells.contains(base)) ? "running" : "idle"
            out.append(SessionRecord(
                id: "local:tmux:\(target)",
                kind: "tui",
                harness: "tmux",
                machineID: "local",
                machineLabel: "This Mac",
                title: cmd.isEmpty ? target : "\(target) · \(URL(fileURLWithPath: cmd).lastPathComponent)",
                cwd: cwd,
                branch: "",
                status: st,
                updatedMs: Int64(Date().timeIntervalSince1970 * 1000),
                pid: pid,
                preview: ptitle.isEmpty ? cwd : ptitle,
                isExternalHarness: false,
                localCommandable: true
            ))
        }
        for line in psRaw.split(whereSeparator: \.isNewline) {
            let cols = line.split(separator: "\t", omittingEmptySubsequences: false).map(String.init)
            guard cols.count >= 5, let pid = Int(cols[0]), pid > 1, !seenPids.contains(pid) else { continue }
            let tty = cols[1]
            let comm = URL(fileURLWithPath: cols[3]).lastPathComponent.lowercased()
            let args = cols[4]
            guard tty != "??" && tty != "?" && !tty.isEmpty else { continue }
            if ["coder-host", "openagents-environment", "psionic-inkling-serve", "gitguard"].contains(comm) { continue }
            if args.contains("--worker") || args.contains("serve --standby") { continue }
            if comm == "coder" || comm == "inky" || comm == "oa-worker" || comm == "openagents" {
                out.append(SessionRecord(
                    id: "local:tui:\(pid)",
                    kind: "tui",
                    harness: comm == "inky" ? "inkling" : "tui",
                    machineID: "local",
                    machineLabel: "This Mac",
                    title: "\(comm) · \(tty)",
                    cwd: cols.count > 5 ? cols[5] : "",
                    branch: "",
                    status: "running",
                    updatedMs: Int64(Date().timeIntervalSince1970 * 1000),
                    pid: pid,
                    preview: args,
                    isExternalHarness: false,
                    localCommandable: true
                ))
            }
        }
        return out
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
            // Only inspect OpenAgents local TUI processes (never third-party external harnesses).
            let lc = (String(cols[3]) + " " + args).lowercased()
            let interesting = lc.contains("inky") || lc.contains("coder") || lc.contains("openagents")
            guard interesting else { continue }
            lines.append("\(cols[0])\t\(cols[1])\t\(cols[2])\t\(cleanField(String(cols[3])))\t\(cleanField(args))\t")
        }
        return lines.joined(separator: "\n") + (lines.isEmpty ? "" : "\n")
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
        var machines: [MachineRecord] = []
        var seen: Set<String> = []

        // 1. Configured SSH / Boat / WSL environments from page/environments.coax
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
            machines.append(MachineRecord(id: cleanField(h.name), label: cleanField(h.name), host: cleanField(h.host), user: "", port: 22, transport: transport, status: status, os: ""))
        }

        // 3. Optional remote sessions cache written by connected remote coder-host daemon
        let remoteCacheURL = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/desktop/remote-sessions.tsv")
        let remoteSessionsTsv = (try? String(contentsOf: remoteCacheURL, encoding: .utf8)) ?? ""
        var remoteSessions: [SessionRecord] = []
        for line in remoteSessionsTsv.split(whereSeparator: \.isNewline) {
            let cols = line.split(separator: "\t", omittingEmptySubsequences: false).map(String.init)
            guard cols.count >= 5 else { continue }
            let mid = cleanField(cols[0])
            let mlabel = cleanField(cols[1])
            let inner = cleanField(cols[2])
            guard !mid.isEmpty, !inner.isEmpty else { continue }
            let harness = cleanField(cols[3])
            let title = cleanField(cols[4])
            let cwd = cols.count > 5 ? cleanField(cols[5]) : ""
            let branch = cols.count > 6 ? cleanField(cols[6]) : ""
            let status = cols.count > 7 ? cleanField(cols[7]) : "idle"
            let upd = cols.count > 8 ? (Int64(cols[8]) ?? 0) : 0
            let preview = cols.count > 9 ? cleanField(cols[9]) : ""
            remoteSessions.append(SessionRecord(
                id: "remote:\(mid):\(inner)",
                kind: "remote",
                harness: harness.isEmpty ? "openagents" : harness,
                machineID: mid,
                machineLabel: mlabel.isEmpty ? mid : mlabel,
                title: title.isEmpty ? inner : title,
                cwd: cwd,
                branch: branch,
                status: status.isEmpty ? "idle" : status,
                updatedMs: upd,
                pid: 0,
                preview: preview,
                isExternalHarness: false,
                localCommandable: true
            ))
        }

        let cappedMachines = Array(machines.prefix(16))
        let machinesTsv = rows.prefix(16).joined(separator: "\n") + (rows.isEmpty ? "" : "\n")
        return RemoteScanResult(machinesTsv: machinesTsv, remoteSessionsTsv: remoteSessionsTsv, machines: cappedMachines, remoteSessions: remoteSessions)
    }

    // ------------------------------------------------------------------ live output capture & direct control (#14067)

    private static func resolveRemoteTarget(machineID: String, machines: [MachineRecord]) -> (dest: String, port: Int)? {
        if let m = machines.first(where: { $0.id.caseInsensitiveCompare(machineID) == .orderedSame || $0.label.caseInsensitiveCompare(machineID) == .orderedSame }) {
            let dest = m.sshDestination.isEmpty ? m.id : m.sshDestination
            return (dest, m.port > 0 ? m.port : 22)
        }
        guard !machineID.isEmpty else { return nil }
        return (machineID, 22)
    }

    static func captureOutput(uri: String, pathIndex: [String: String], tailLines: Int = 200, machines: [MachineRecord] = []) -> (output: String, status: String) {
        if uri.hasPrefix("local:tmux:") {
            let target = String(uri.dropFirst("local:tmux:".count))
            guard let tmux = tmuxBinary else {
                return ("tmux binary not found on PATH.", "tmux unavailable")
            }
            let clamped = max(10, min(500, tailLines))
            let out = runQuick(tmux, ["capture-pane", "-p", "-S", "-\(clamped)", "-t", target], timeout: 2.0)
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
                    return (formatted, "Live tail · \(url.lastPathComponent)")
                }
                return ("Session artifact path: \(filePath)", "Artifact directory")
            }
            // Fallback for PID-based harness process: local:harness:<h>:pid-<N>
            if let r = uri.range(of: ":pid-") {
                let pid = String(uri[r.upperBound...])
                let psInfo = runQuick("/bin/ps", ["-p", pid, "-o", "pid=,ppid=,tty=,stat=,%cpu=,%mem=,etime=,args="], timeout: 2.0)
                if !psInfo.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    return ("PID   PPID TTY      STAT %CPU %MEM ELAPSED COMMAND\n" + psInfo.trimmingCharacters(in: .whitespacesAndNewlines),
                            "Live process (PID \(pid))")
                }
            }
            return ("No transcript file available for \(uri).", "No transcript")
        }

        if uri.hasPrefix("remote:") {
            let rest = String(uri.dropFirst("remote:".count))
            if let colon = rest.firstIndex(of: ":") {
                let machineID = String(rest[..<colon])
                let inner = String(rest[rest.index(after: colon)...])
                if let target = resolveRemoteTarget(machineID: machineID, machines: machines), inner.hasPrefix("tmux:") {
                    let tmuxPane = String(inner.dropFirst("tmux:".count))
                    let clamped = max(10, min(500, tailLines))
                    let cmd = "tmux capture-pane -p -S -\(clamped) -t \(shellQuote(tmuxPane))"
                    var sshArgs = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=5"]
                    if target.port != 22 { sshArgs.append(contentsOf: ["-p", "\(target.port)"]) }
                    sshArgs.append(contentsOf: [target.dest, cmd])
                    let res = runQuickWithStatus("/usr/bin/ssh", sshArgs, timeout: 8.0)
                    if res.ok {
                        let trimmed = trimTrailingEmptyLines(res.stdout)
                        return (trimmed.isEmpty ? "(remote pane is empty)" : trimmed, "Live · remote tmux on \(machineID)")
                    }
                }
            }
            return ("Remote session: \(uri)\nConnect to this machine in Settings -> Connections or verify SSH access to stream live output.", "Remote session")
        }

        return ("", "Ready")
    }

    static func sendCommand(uri: String, prompt: String, mode: String, submit: Bool, allowLocalHarnesses: Bool, machines: [MachineRecord]) -> (status: Int, body: [String: Any]) {
        if uri.hasPrefix("local:tmux:") {
            let target = String(uri.dropFirst("local:tmux:".count))
            guard let tmux = tmuxBinary else {
                return (503, ["ok": false, "session_id": uri, "error": "tmux binary not found on PATH"])
            }
            if mode == "keys" {
                let keys = prompt.split(separator: " ").map(String.init)
                let res = runQuickWithStatus(tmux, ["send-keys", "-t", target] + (keys.isEmpty ? [prompt] : keys), timeout: 5.0)
                if !res.ok {
                    return (400, ["ok": false, "session_id": uri, "error": res.stderr.isEmpty ? "tmux send-keys failed" : cleanField(res.stderr)])
                }
                return (200, ["ok": true, "session_id": uri, "kind": "tui", "target": target, "mode": "keys", "submitted": true])
            }
            let r1 = runQuickWithStatus(tmux, ["send-keys", "-t", target, "-l", "--", prompt], timeout: 5.0)
            if !r1.ok {
                return (400, ["ok": false, "session_id": uri, "error": r1.stderr.isEmpty ? "tmux send-keys failed (pane may not exist)" : cleanField(r1.stderr)])
            }
            if submit {
                let r2 = runQuickWithStatus(tmux, ["send-keys", "-t", target, "Enter"], timeout: 5.0)
                if !r2.ok {
                    return (500, ["ok": false, "session_id": uri, "error": cleanField(r2.stderr)])
                }
            }
            return (200, ["ok": true, "session_id": uri, "kind": "tui", "target": target, "mode": mode, "submitted": submit])
        }

        if uri.hasPrefix("local:tui:") {
            let pidStr = String(uri.dropFirst("local:tui:".count))
            guard let pid = Int(pidStr), pid > 1 else {
                return (400, ["ok": false, "session_id": uri, "error": "invalid TUI pid"])
            }
            let dir = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/tui/sessions", isDirectory: true)
            try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
            let inbox = dir.appendingPathComponent("\(pid).inbox")
            let line = prompt + "\n"
            if FileManager.default.fileExists(atPath: inbox.path), let fh = try? FileHandle(forWritingTo: inbox) {
                defer { try? fh.close() }
                _ = try? fh.seekToEnd()
                fh.write(Data(line.utf8))
            } else {
                _ = FileManager.default.createFile(atPath: inbox.path, contents: Data(line.utf8), attributes: [.posixPermissions: 0o600])
            }
            return (200, ["ok": true, "session_id": uri, "kind": "tui", "pid": pid, "inbox": inbox.path])
        }

        if uri.hasPrefix("local:harness:") {
            // Reconciled with #14071: external local harnesses are strictly opt-in via Settings -> Agent.
            guard allowLocalHarnesses else {
                return (409, [
                    "ok": false,
                    "session_id": uri,
                    "requires_opt_in": true,
                    "allow_local_harnesses": false,
                    "error": "External local harness control is disabled (opt-in off). Enable 'Allow delegating to other local harnesses' in Settings -> Agent (#14071)."
                ])
            }
            let rest = String(uri.dropFirst("local:harness:".count))
            guard let colon = rest.firstIndex(of: ":") else {
                return (400, ["ok": false, "session_id": uri, "error": "malformed harness session URI"])
            }
            let harness = String(rest[..<colon]).lowercased()
            let sid = String(rest[rest.index(after: colon)...])
            switch harness {
            case "claude":
                guard let bin = resolveExecutable("claude") else {
                    return (503, ["ok": false, "session_id": uri, "error": "claude CLI not found on PATH"])
                }
                let res = runQuickWithStatus(bin, ["--resume", sid, "-p", prompt, "--dangerously-skip-permissions"], timeout: 60.0)
                return (res.ok ? 200 : 500, [
                    "ok": res.ok,
                    "session_id": uri,
                    "harness": "claude",
                    "exit_code": Int(res.code),
                    "output": String(res.stdout.prefix(16384)),
                    "error": res.ok ? "" : cleanField(res.stderr)
                ])
            case "codex":
                guard let bin = resolveExecutable("codex") else {
                    return (503, ["ok": false, "session_id": uri, "error": "codex CLI not found on PATH"])
                }
                let res = runQuickWithStatus(bin, ["exec", "resume", sid, prompt], timeout: 60.0)
                return (res.ok ? 200 : 500, [
                    "ok": res.ok,
                    "session_id": uri,
                    "harness": "codex",
                    "exit_code": Int(res.code),
                    "output": String(res.stdout.prefix(16384)),
                    "error": res.ok ? "" : cleanField(res.stderr)
                ])
            default:
                return (400, ["ok": false, "session_id": uri, "error": "direct CLI resume is not available for harness '\(harness)'"])
            }
        }

        if uri.hasPrefix("remote:") {
            let rest = String(uri.dropFirst("remote:".count))
            guard let colon = rest.firstIndex(of: ":") else {
                return (400, ["ok": false, "session_id": uri, "error": "malformed remote session URI"])
            }
            let machineID = String(rest[..<colon])
            let inner = String(rest[rest.index(after: colon)...])
            guard let target = resolveRemoteTarget(machineID: machineID, machines: machines) else {
                return (404, ["ok": false, "session_id": uri, "error": "unknown remote machine: \(machineID)"])
            }
            let remoteCmd: String
            if inner.hasPrefix("tmux:") {
                let pane = String(inner.dropFirst("tmux:".count))
                if mode == "keys" {
                    remoteCmd = "tmux send-keys -t \(shellQuote(pane)) \(shellQuote(prompt))"
                } else {
                    remoteCmd = submit
                        ? "tmux send-keys -t \(shellQuote(pane)) -l -- \(shellQuote(prompt)) && tmux send-keys -t \(shellQuote(pane)) Enter"
                        : "tmux send-keys -t \(shellQuote(pane)) -l -- \(shellQuote(prompt))"
                }
            } else {
                let payloadObj: [String: Any] = ["session_id": inner, "prompt": prompt, "mode": mode, "submit": submit]
                let payloadStr = (try? JSONSerialization.data(withJSONObject: payloadObj)).map { String(decoding: $0, as: UTF8.self) } ?? "{}"
                remoteCmd = """
                python3 -c 'import json,os,urllib.request,sys; d=json.load(open(os.path.expanduser("~/.openagents/session-bus.json"))); req=urllib.request.Request(d["url"]+"/v1/sessions/send", data=sys.argv[1].encode(), headers={"Content-Type":"application/json","X-Session-Bus-Token":d["token"]}, method="POST"); print(urllib.request.urlopen(req, timeout=15).read().decode())' \(shellQuote(payloadStr))
                """
            }
            var sshArgs = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=6"]
            if target.port != 22 { sshArgs.append(contentsOf: ["-p", "\(target.port)"]) }
            sshArgs.append(contentsOf: [target.dest, remoteCmd])
            let res = runQuickWithStatus("/usr/bin/ssh", sshArgs, timeout: 20.0)
            return (res.ok ? 200 : 502, [
                "ok": res.ok,
                "session_id": uri,
                "machine": machineID,
                "output": String(res.stdout.prefix(8192)),
                "error": res.ok ? "" : cleanField(res.stderr)
            ])
        }

        return (404, ["ok": false, "session_id": uri, "error": "unsupported session URI: \(uri)"])
    }

    static func stopSession(uri: String, allowLocalHarnesses: Bool, machines: [MachineRecord]) -> (status: Int, body: [String: Any]) {
        if uri.hasPrefix("local:tmux:") {
            let target = String(uri.dropFirst("local:tmux:".count))
            guard let tmux = tmuxBinary else {
                return (503, ["ok": false, "session_id": uri, "error": "tmux binary not found on PATH"])
            }
            let res = runQuickWithStatus(tmux, ["send-keys", "-t", target, "C-c"], timeout: 3.0)
            return (res.ok ? 200 : 400, [
                "ok": res.ok,
                "session_id": uri,
                "status": res.ok ? "interrupted" : "failed",
                "error": res.ok ? "" : cleanField(res.stderr)
            ])
        }
        if uri.hasPrefix("local:tui:") {
            let pidStr = String(uri.dropFirst("local:tui:".count))
            guard let pid = Int32(pidStr), pid > 1, pid != getpid() else {
                return (400, ["ok": false, "session_id": uri, "error": "invalid TUI pid"])
            }
            let rc = Darwin.kill(pid, SIGINT)
            return (rc == 0 ? 200 : 400, ["ok": rc == 0, "session_id": uri, "pid": Int(pid), "status": rc == 0 ? "interrupted" : "failed"])
        }
        if uri.hasPrefix("local:harness:") {
            guard allowLocalHarnesses else {
                return (409, [
                    "ok": false,
                    "session_id": uri,
                    "requires_opt_in": true,
                    "allow_local_harnesses": false,
                    "error": "External local harness control is disabled (opt-in off). Enable 'Allow delegating to other local harnesses' in Settings -> Agent (#14071)."
                ])
            }
            if let r = uri.range(of: ":pid-"), let pid = Int32(String(uri[r.upperBound...])), pid > 1, pid != getpid() {
                let rc = Darwin.kill(pid, SIGINT)
                return (rc == 0 ? 200 : 400, ["ok": rc == 0, "session_id": uri, "pid": Int(pid), "status": rc == 0 ? "interrupted" : "failed"])
            }
            return (200, ["ok": true, "session_id": uri, "status": "idle"])
        }
        if uri.hasPrefix("remote:") {
            let rest = String(uri.dropFirst("remote:".count))
            guard let colon = rest.firstIndex(of: ":") else {
                return (400, ["ok": false, "session_id": uri, "error": "malformed remote session URI"])
            }
            let machineID = String(rest[..<colon])
            let inner = String(rest[rest.index(after: colon)...])
            guard let target = resolveRemoteTarget(machineID: machineID, machines: machines) else {
                return (404, ["ok": false, "session_id": uri, "error": "unknown remote machine: \(machineID)"])
            }
            let cmd: String
            if inner.hasPrefix("tmux:") {
                cmd = "tmux send-keys -t \(shellQuote(String(inner.dropFirst("tmux:".count)))) C-c"
            } else {
                let payloadStr = "{\"session_id\":\(shellQuote(inner))}"
                cmd = "python3 -c 'import json,os,urllib.request,sys; d=json.load(open(os.path.expanduser(\"~/.openagents/session-bus.json\"))); req=urllib.request.Request(d[\"url\"]+\"/v1/sessions/stop\", data=json.dumps({\"session_id\":sys.argv[1]}).encode(), headers={\"Content-Type\":\"application/json\",\"X-Session-Bus-Token\":d[\"token\"]}, method=\"POST\"); print(urllib.request.urlopen(req, timeout=10).read().decode())' \(shellQuote(inner))"
                _ = payloadStr
            }
            var sshArgs = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=6"]
            if target.port != 22 { sshArgs.append(contentsOf: ["-p", "\(target.port)"]) }
            sshArgs.append(contentsOf: [target.dest, cmd])
            let res = runQuickWithStatus("/usr/bin/ssh", sshArgs, timeout: 12.0)
            return (res.ok ? 200 : 502, ["ok": res.ok, "session_id": uri, "machine": machineID, "status": res.ok ? "stopped" : "failed", "error": res.ok ? "" : cleanField(res.stderr)])
        }
        return (404, ["ok": false, "session_id": uri, "error": "unsupported session URI: \(uri)"])
    }

    static func spawnExternal(machine: String, harness: String, cwd: String, prompt: String, title: String, allowLocalHarnesses: Bool, machines: [MachineRecord]) -> (status: Int, body: [String: Any]) {
        let externalHarnesses: Set<String> = ["claude", "codex", "agy", "antigravity", "cursor", "opencode"]
        if machine.isEmpty && externalHarnesses.contains(harness) {
            guard allowLocalHarnesses else {
                return (409, [
                    "ok": false,
                    "harness": harness,
                    "requires_opt_in": true,
                    "allow_local_harnesses": false,
                    "error": "Spawning external local harness '\(harness)' is disabled (opt-in off). Enable 'Allow delegating to other local harnesses' in Settings -> Agent (#14071)."
                ])
            }
        }
        if !machine.isEmpty {
            guard let target = resolveRemoteTarget(machineID: machine, machines: machines) else {
                return (404, ["ok": false, "error": "unknown remote machine: \(machine)"])
            }
            let sessName = "oa-\(Int(Date().timeIntervalSince1970) % 100000)"
            var remoteCmd = "tmux new-session -d -P -F '#{session_name}:#{window_index}.#{pane_index}' -s \(shellQuote(sessName))"
            if !cwd.isEmpty { remoteCmd += " -c \(shellQuote(cwd))" }
            if !prompt.isEmpty { remoteCmd += " && tmux send-keys -t \(shellQuote(sessName)) -l -- \(shellQuote(prompt)) && tmux send-keys -t \(shellQuote(sessName)) Enter" }
            var sshArgs = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=6"]
            if target.port != 22 { sshArgs.append(contentsOf: ["-p", "\(target.port)"]) }
            sshArgs.append(contentsOf: [target.dest, remoteCmd])
            let res = runQuickWithStatus("/usr/bin/ssh", sshArgs, timeout: 15.0)
            let pane = res.stdout.split(whereSeparator: \.isNewline).first.map(String.init) ?? "\(sessName):0.0"
            return (res.ok ? 200 : 502, [
                "ok": res.ok,
                "session_id": "remote:\(machine):tmux:\(pane)",
                "kind": "remote",
                "machine": machine,
                "harness": harness.isEmpty ? "tmux" : harness,
                "status": res.ok ? "running" : "failed",
                "error": res.ok ? "" : cleanField(res.stderr)
            ])
        }

        // Local tmux/tui or opted-in external harness inside a detached tmux pane
        guard let tmux = tmuxBinary else {
            return (503, ["ok": false, "error": "tmux binary not found on PATH"])
        }
        let cleanTitle = title.map { ($0.isLetter || $0.isNumber || $0 == "-" || $0 == "_") ? $0 : "-" }
        let sessName = cleanTitle.isEmpty ? "oa-\(Int(Date().timeIntervalSince1970) % 100000)" : String(cleanTitle.prefix(24))
        var args = ["new-session", "-d", "-P", "-F", "#{session_name}:#{window_index}.#{pane_index}", "-s", sessName]
        if !cwd.isEmpty { args.append(contentsOf: ["-c", cwd]) }
        let created = runQuickWithStatus(tmux, args, timeout: 5.0)
        guard created.ok else {
            return (500, ["ok": false, "error": created.stderr.isEmpty ? "tmux new-session failed" : cleanField(created.stderr)])
        }
        let target = created.stdout.trimmingCharacters(in: .whitespacesAndNewlines)
        let sendLine: String
        if harness == "claude" {
            sendLine = prompt.isEmpty ? "claude --dangerously-skip-permissions" : "claude -p \(shellQuote(prompt)) --dangerously-skip-permissions"
        } else if harness == "codex" {
            sendLine = prompt.isEmpty ? "codex" : "codex exec \(shellQuote(prompt))"
        } else {
            sendLine = prompt
        }
        if !sendLine.isEmpty {
            _ = runQuickWithStatus(tmux, ["send-keys", "-t", target, "-l", "--", sendLine], timeout: 5.0)
            _ = runQuickWithStatus(tmux, ["send-keys", "-t", target, "Enter"], timeout: 5.0)
        }
        return (200, [
            "ok": true,
            "session_id": "local:tmux:\(target)",
            "kind": "tui",
            "harness": harness.isEmpty ? "tmux" : harness,
            "cwd": cwd,
            "title": title.isEmpty ? sessName : title,
            "status": "running"
        ])
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
    /// Background polling of external harnesses and remote machines is intentionally disabled
    /// so OpenAgents Desktop stays lightweight and keeps the sidebar focused on OpenAgents chats.
    func setupIntrospection() {
        retireIntrospection()
        _ = cd_introspect_set_harnesses(page, "", "", 0)
        _ = cd_introspect_set_machines(page, "", "")
    }

    func retireIntrospection() {
        introspectLocalTimer?.invalidate()
        introspectLocalTimer = nil
        introspectRemoteTimer?.invalidate()
        introspectRemoteTimer = nil
    }

    func snapshotEnvHostsForIntrospect() -> [IntrospectScanner.EnvHostSnapshot] {
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
        return envHosts
    }

    func refreshIntrospectLocal() {
        guard !introspectLocalBusy else { return }
        introspectLocalBusy = true
        IntrospectScanner.queue.async { [weak self] in
            let res = IntrospectScanner.scanLocal()
            DispatchQueue.main.async {
                guard let self, !self.retired else { return }
                self.introspectLocalBusy = false
                self.introspectLocalSnap = res
                self.introspectPathIndex = res.pathIndex
                _ = cd_introspect_set_tui(self.page, res.tmuxRaw, res.psRaw, 0)
                _ = cd_introspect_set_harnesses(self.page, "", "", 0)
                self.wake()
            }
        }
    }

    func refreshIntrospectRemote() {
        guard !introspectRemoteBusy else { return }
        introspectRemoteBusy = true

        let envHosts = snapshotEnvHostsForIntrospect()
        IntrospectScanner.queue.async { [weak self] in
            let res = IntrospectScanner.scanRemoteMachines(envHosts: envHosts)
            DispatchQueue.main.async {
                guard let self, !self.retired else { return }
                self.introspectRemoteBusy = false
                self.introspectRemoteSnap = res
                _ = cd_introspect_set_machines(self.page, "", "")
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
        let machines = introspectRemoteSnap?.machines ?? []
        IntrospectScanner.queue.async { [weak self] in
            let res = IntrospectScanner.captureOutput(uri: activeID, pathIndex: paths, machines: machines)
            DispatchQueue.main.async {
                guard let self, !self.retired else { return }
                self.introspectOutputBusy = false
                _ = cd_introspect_set_output(self.page, activeID, res.output, res.status, 0)
                self.wake()
            }
        }
    }

    /// Handles CP_ASK_INTROSPECT_* (250..255) from page/introspect.coax (#14066, #14067).
    func introspectAsked(_ what: Int32) {
        switch what {
        case 250: // CP_ASK_INTROSPECT_REFRESH
            refreshIntrospectLocal()
            refreshIntrospectRemote()
        case 251: // CP_ASK_INTROSPECT_INSPECT
            refreshInspectedOutput()
            if cd_introspect_can_command(page) != 0 {
                focusComposer()
            }
        case 252: // CP_ASK_INTROSPECT_SEND (direct command from inspector composer; #14067)
            var idBuf = [CChar](repeating: 0, count: 256)
            var cmdBuf = [CChar](repeating: 0, count: 4097)
            guard cd_introspect_active_id(page, &idBuf, Int32(idBuf.count)) > 0,
                  cd_introspect_cmd(page, &cmdBuf, Int32(cmdBuf.count)) > 0 else { return }
            let activeID = String(cString: idBuf)
            let command = String(cString: cmdBuf)
            let allowExt = AgentSettings.allowLocalHarnesses
            let machines = introspectRemoteSnap?.machines ?? []
            IntrospectScanner.queue.async { [weak self] in
                let res = IntrospectScanner.sendCommand(
                    uri: activeID,
                    prompt: command,
                    mode: "send",
                    submit: true,
                    allowLocalHarnesses: allowExt,
                    machines: machines
                )
                DispatchQueue.main.async {
                    guard let self, !self.retired else { return }
                    let ok = res.status >= 200 && res.status < 300
                    let errMsg = (res.body["error"] as? String) ?? ""
                    let note = ok ? "Sent to \(activeID)" : (errMsg.isEmpty ? "Command failed (HTTP \(res.status))" : errMsg)
                    _ = cd_introspect_cmd_done(self.page, activeID, note, ok ? 0 : 1)
                    self.refreshInspectedOutput()
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { [weak self] in
                        self?.refreshInspectedOutput()
                    }
                    self.wake()
                }
            }
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
        case 255: // CP_ASK_INTROSPECT_STOP (interrupt / Ctrl-C; #14067)
            var idBuf = [CChar](repeating: 0, count: 256)
            guard cd_introspect_active_id(page, &idBuf, Int32(idBuf.count)) > 0 else { return }
            let activeID = String(cString: idBuf)
            let allowExt = AgentSettings.allowLocalHarnesses
            let machines = introspectRemoteSnap?.machines ?? []
            IntrospectScanner.queue.async { [weak self] in
                let res = IntrospectScanner.stopSession(uri: activeID, allowLocalHarnesses: allowExt, machines: machines)
                DispatchQueue.main.async {
                    guard let self, !self.retired else { return }
                    let ok = res.status >= 200 && res.status < 300
                    let errMsg = (res.body["error"] as? String) ?? ""
                    let note = ok ? "Interrupted \(activeID)" : (errMsg.isEmpty ? "Stop failed" : errMsg)
                    _ = cd_introspect_cmd_done(self.page, activeID, note, ok ? 0 : 1)
                    self.refreshInspectedOutput()
                    self.wake()
                }
            }
        default:
            break
        }
    }
}
