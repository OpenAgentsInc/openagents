// Firm reason: 6 (AppKit/Darwin PTY host, DispatchSource I/O, and lsof/ps port scanning over the compiled page).
// Integrated multi-tab terminal pane host (page/termpane.coax, GitHub #13869):
// spawns interactive shells on POSIX PTYs (host.c cd_pty_spawn), streams output into the page's
// 8 MiB per-tab scrollback buffers, forwards keyboard input and window resizes, scans listening
// local dev-server ports + process-tree CPU/RAM telemetry, and attaches selected terminal output
// to the composer as a terminal-context chip.
import AppKit
import Darwin

final class TerminalSession {
    let tabId: Int32
    let masterFD: Int32
    let pid: Int32
    let cwd: String
    private var readSource: DispatchSourceRead?
    private var closed = false
    private let lock = NSLock()

    init(tabId: Int32, masterFD: Int32, pid: Int32, cwd: String) {
        self.tabId = tabId
        self.masterFD = masterFD
        self.pid = pid
        self.cwd = cwd
    }

    func startReading(onOutput: @escaping (Data) -> Void, onExit: @escaping (Int32) -> Void) {
        let queue = DispatchQueue(label: "openagents.term.pty.\(tabId)", qos: .userInitiated)
        let source = DispatchSource.makeReadSource(fileDescriptor: masterFD, queue: queue)
        source.setEventHandler { [weak self] in
            guard let self else { return }
            var buf = [UInt8](repeating: 0, count: 8192)
            while true {
                let n = Darwin.read(self.masterFD, &buf, buf.count)
                if n > 0 {
                    onOutput(Data(buf[0..<n]))
                } else if n == 0 {
                    self.finish(onExit: onExit)
                    return
                } else {
                    if errno == EAGAIN || errno == EINTR { return }
                    self.finish(onExit: onExit)
                    return
                }
            }
        }
        source.setCancelHandler { [fd = masterFD] in
            _ = Darwin.close(fd)
        }
        readSource = source
        source.resume()
    }

    func write(_ data: Data) {
        lock.lock()
        let isClosed = closed
        lock.unlock()
        guard !isClosed, !data.isEmpty else { return }
        data.withUnsafeBytes { raw in
            guard let base = raw.baseAddress else { return }
            var sent = 0
            while sent < raw.count {
                let n = Darwin.write(masterFD, base.advanced(by: sent), raw.count - sent)
                if n > 0 { sent += n }
                else if errno == EINTR { continue }
                else { break }
            }
        }
    }

    func resize(cols: Int32, rows: Int32) {
        lock.lock()
        let isClosed = closed
        lock.unlock()
        guard !isClosed else { return }
        _ = cd_pty_resize(masterFD, cols, rows)
    }

    func terminate() {
        lock.lock()
        if closed { lock.unlock(); return }
        closed = true
        let src = readSource
        readSource = nil
        lock.unlock()
        cd_pty_kill(pid)
        src?.cancel()
        var status: Int32 = 0
        _ = waitpid(pid, &status, WNOHANG)
    }

    private func finish(onExit: @escaping (Int32) -> Void) {
        lock.lock()
        if closed { lock.unlock(); return }
        closed = true
        let src = readSource
        readSource = nil
        lock.unlock()
        src?.cancel()
        var status: Int32 = 0
        var code: Int32 = 0
        if waitpid(pid, &status, WNOHANG) > 0 {
            if (status & 0x7f) == 0 { code = (status >> 8) & 0xff }
            else { code = 128 + (status & 0x7f) }
        }
        onExit(code)
    }
}

extension CoderWindow {
    func toggleTerminal() {
        cd_term_toggle(page)
        // handle immediate open/focus op synchronously if triggered
        let what = cd_action(page)
        if what == 52 { termAsked() }
        wake()
    }

    func termAsked() {
        let op = cd_term_op(page)
        let tabId = cd_term_op_tab(page)
        switch op {
        case 1: // CP_TERM_OP_OPEN
            spawnTerminalTab(tabId)
        case 2: // CP_TERM_OP_CLOSE_TAB
            closeTerminalTab(tabId)
        case 4: // CP_TERM_OP_INPUT
            flushTerminalInput()
        case 5: // CP_TERM_OP_SEND_CHAT
            sendTerminalSelectionToChat(tabId: tabId)
        case 6: // CP_TERM_OP_FOCUS
            window.makeFirstResponder(canvas)
            wake()
        case 7: // CP_TERM_OP_CLOSED
            break
        default:
            break
        }
    }

    func syncTerminalFrame() {
        var cols: Int32 = 0
        var rows: Int32 = 0
        let tabId = cd_term_resize_due(page, &cols, &rows)
        if tabId > 0, let session = termSessions[tabId] {
            session.resize(cols: cols, rows: rows)
        }
    }

    func stopAllTerminals() {
        termPortTimer?.invalidate()
        termPortTimer = nil
        for (_, session) in termSessions {
            session.terminate()
        }
        termSessions.removeAll()
    }

    private func spawnTerminalTab(_ tabId: Int32) {
        guard tabId > 0, termSessions[tabId] == nil else { return }
        var cwdBuf = [CChar](repeating: 0, count: 1024)
        let n = cd_term_tab_cwd(page, tabId, &cwdBuf, Int32(cwdBuf.count))
        let rawCwd = n > 0 ? (String(cString: cwdBuf) as NSString).expandingTildeInPath : folder.path
        var isDir: ObjCBool = false
        let cwd = (FileManager.default.fileExists(atPath: rawCwd, isDirectory: &isDir) && isDir.boolValue) ? rawCwd : folder.path
        let shell = ProcessInfo.processInfo.environment["SHELL"] ?? "/bin/zsh"
        var pid: Int32 = -1
        let fd = cd_pty_spawn(shell, cwd, 80, 24, &pid)
        guard fd >= 0, pid > 0 else {
            let msg = "Failed to spawn PTY for \(shell)\n"
            msg.withCString { ptr in _ = cd_term_feed(page, tabId, ptr, Int32(strlen(ptr))) }
            cd_term_exited(page, tabId, 1)
            wake()
            return
        }
        let session = TerminalSession(tabId: tabId, masterFD: fd, pid: pid, cwd: cwd)
        termSessions[tabId] = session
        session.startReading(
            onOutput: { [weak self] data in
                DispatchQueue.main.async {
                    guard let self else { return }
                    data.withUnsafeBytes { raw in
                        guard let base = raw.bindMemory(to: CChar.self).baseAddress else { return }
                        _ = cd_term_feed(self.page, tabId, base, Int32(data.count))
                    }
                    self.wake()
                }
            },
            onExit: { [weak self] code in
                DispatchQueue.main.async {
                    guard let self else { return }
                    cd_term_exited(self.page, tabId, code)
                    self.termSessions.removeValue(forKey: tabId)
                    if self.termSessions.isEmpty {
                        self.termPortTimer?.invalidate()
                        self.termPortTimer = nil
                    }
                    self.wake()
                }
            }
        )
        ensureTerminalPortTimer()
        window.makeFirstResponder(canvas)
        wake()
    }

    private func closeTerminalTab(_ tabId: Int32) {
        if let session = termSessions.removeValue(forKey: tabId) {
            session.terminate()
        }
        if termSessions.isEmpty {
            termPortTimer?.invalidate()
            termPortTimer = nil
        }
    }

    private func flushTerminalInput() {
        var buf = [CChar](repeating: 0, count: 4096)
        let n = cd_term_input_out(page, &buf, Int32(buf.count))
        guard n > 0 else { return }
        let activeId = cd_term_active_id(page)
        guard activeId > 0, let session = termSessions[activeId] else { return }
        let data = buf.withUnsafeBufferPointer { Data(buffer: UnsafeBufferPointer(rebasing: $0[0..<Int(n)])) }
        session.write(data)
    }

    private func sendTerminalSelectionToChat(tabId: Int32) {
        var buf = [CChar](repeating: 0, count: 16385)
        let n = cd_term_selected(page, &buf, Int32(buf.count))
        guard n > 0 else { return }
        let text = String(cString: buf)
        let dir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents", isDirectory: true)
            .appendingPathComponent("terminal-context", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let ts = Int(Date().timeIntervalSince1970)
        let fileURL = dir.appendingPathComponent("terminal-tab-\(tabId)-\(ts).txt")
        do {
            try text.write(to: fileURL, atomically: true, encoding: .utf8)
            _ = cd_attach(page, fileURL.path)
            focusComposer()
            wake()
        } catch {
            // fallback: append snippet into composer draft if disk write failed
            let combined = draft.isEmpty ? text : draft + "\n" + text
            _ = cd_set_text(page, combined)
            focusComposer()
            wake()
        }
    }

    private func ensureTerminalPortTimer() {
        guard termPortTimer == nil else { return }
        let timer = Timer(timeInterval: 2.5, repeats: true) { [weak self] _ in
            self?.scanTerminalPorts()
        }
        RunLoop.main.add(timer, forMode: .common)
        termPortTimer = timer
    }

    private func scanTerminalPorts() {
        guard cd_term_open(page) != 0, !termSessions.isEmpty else { return }
        let snapshot: [(Int32, Int32)] = termSessions.map { ($0.key, $0.value.pid) }
        DispatchQueue.global(qos: .utility).async { [weak self] in
            let reports = CoderWindow.collectPortReports(tabs: snapshot)
            DispatchQueue.main.async {
                guard let self else { return }
                for (tabId, payload) in reports {
                    _ = cd_term_ports(self.page, tabId, payload)
                }
                if !reports.isEmpty { self.wake() }
            }
        }
    }

    private static func runCommandOutput(_ path: String, args: [String]) -> String {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: path)
        p.arguments = args
        let pipe = Pipe()
        p.standardOutput = pipe
        p.standardError = FileHandle.nullDevice
        do { try p.run() } catch { return "" }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        return String(data: data, encoding: .utf8) ?? ""
    }

    private static func collectPortReports(tabs: [(Int32, Int32)]) -> [(Int32, String)] {
        struct ProcInfo { let pid: Int32; let ppid: Int32; let cpuTenths: Int; let rssKB: Int; let comm: String }
        let psOut = runCommandOutput("/bin/ps", args: ["-eo", "pid=,ppid=,pcpu=,rss=,comm="])
        var procs: [Int32: ProcInfo] = [:]
        var children: [Int32: [Int32]] = [:]
        for line in psOut.split(separator: "\n") {
            let parts = line.split(maxSplits: 4, omittingEmptySubsequences: true, whereSeparator: { $0 == " " || $0 == "\t" })
            guard parts.count >= 5,
                  let pid = Int32(parts[0]),
                  let ppid = Int32(parts[1]) else { continue }
            let cpuVal = Double(parts[2]) ?? 0.0
            let rssVal = Int(parts[3]) ?? 0
            let comm = URL(fileURLWithPath: String(parts[4])).lastPathComponent
            procs[pid] = ProcInfo(pid: pid, ppid: ppid, cpuTenths: Int((cpuVal * 10.0).rounded()), rssKB: rssVal, comm: comm)
            children[ppid, default: []].append(pid)
        }

        // Parse lsof listening TCP sockets: -F pcn emits p<pid>, c<cmd>, n<addr:port>
        let lsofOut = runCommandOutput("/usr/sbin/lsof", args: ["-nP", "-iTCP", "-sTCP:LISTEN", "-F", "pcn"])
        var pidPorts: [Int32: [(Int, String)]] = [:]
        var curPid: Int32 = 0
        var curCmd = ""
        for line in lsofOut.split(separator: "\n") {
            guard let first = line.first else { continue }
            let rest = String(line.dropFirst())
            if first == "p", let p = Int32(rest) { curPid = p }
            else if first == "c" { curCmd = rest }
            else if first == "n", curPid > 0 {
                if let colon = rest.lastIndex(of: ":"),
                   let port = Int(rest[rest.index(after: colon)...]),
                   port > 0 && port <= 65535 {
                    pidPorts[curPid, default: []].append((port, curCmd))
                }
            }
        }

        var results: [(Int32, String)] = []
        for (tabId, rootPid) in tabs {
            // BFS over descendant process tree of rootPid
            var tree: Set<Int32> = [rootPid]
            var queue: [Int32] = [rootPid]
            while !queue.isEmpty {
                let p = queue.removeFirst()
                for child in children[p] ?? [] where !tree.contains(child) {
                    tree.insert(child)
                    queue.append(child)
                }
            }
            var totalCPU = 0
            var totalRSS = 0
            var lines: [String] = []
            for p in tree {
                if let info = procs[p] {
                    totalCPU += info.cpuTenths
                    totalRSS += info.rssKB
                }
            }
            lines.append("TREE\t\(totalCPU)\t\(totalRSS)")
            var seenPorts: Set<Int> = []
            for p in tree.sorted() {
                for (port, cmd) in pidPorts[p] ?? [] where !seenPorts.contains(port) {
                    seenPorts.insert(port)
                    let info = procs[p]
                    let name = cmd.isEmpty ? (info?.comm ?? "proc") : cmd
                    lines.append("PORT\t\(port)\t\(p)\t\(name)\t\(info?.cpuTenths ?? 0)\t\(info?.rssKB ?? 0)")
                }
            }
            results.append((tabId, lines.joined(separator: "\n")))
        }
        return results
    }
}
