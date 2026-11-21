// Firm reason: 6 (AppKit dialogs, 0600 token file I/O, loopback OAuth PKCE callback, and worker env wiring over page/mcpskills.coax).
// MCP server management UI, MCP OAuth PKCE loopback flow, and Skills catalog/picker (GitHub #13879).
import AppKit
import CryptoKit
import Darwin
import Foundation

struct McpServerConfig: Codable {
    var name: String
    var transport: String   // "http" or "stdio"
    var endpoint: String
    var enabled: Bool
    var status: Int
    var tool_count: Int
    var has_token: Bool?
}

struct McpConfigFile: Codable {
    var v: Int
    var servers: [McpServerConfig]
}

enum McpServerStore {
    static var configURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/desktop/mcp-servers.json")
    }

    static var tokensDir: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/Application Support/Coder/mcp-tokens")
    }

    private static func sanitizedKey(_ name: String) -> String {
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "-_"))
        let mapped = name.unicodeScalars.map { allowed.contains($0) ? Character($0) : "_" }
        let s = String(mapped)
        return s.isEmpty ? "default" : s
    }

    static func tokenURL(for name: String) -> URL {
        tokensDir.appendingPathComponent(sanitizedKey(name) + ".token")
    }

    static func loadJSON() -> String {
        guard let data = try? Data(contentsOf: configURL),
              let text = String(data: data, encoding: .utf8) else {
            return "{\"v\":1,\"servers\":[]}"
        }
        return text
    }

    static func load() -> [McpServerConfig] {
        guard let data = try? Data(contentsOf: configURL),
              let file = try? JSONDecoder().decode(McpConfigFile.self, from: data) else {
            return []
        }
        return file.servers
    }

    static func saveJSON(_ json: String) {
        let dir = configURL.deletingLastPathComponent()
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        try? json.data(using: .utf8)?.write(to: configURL, options: .atomic)
    }

    static func save(_ servers: [McpServerConfig]) {
        let file = McpConfigFile(v: 1, servers: servers)
        if let data = try? JSONEncoder().encode(file), let text = String(data: data, encoding: .utf8) {
            saveJSON(text)
        }
    }

    /// Saves a bearer/OAuth token in a 0600 file under ~/Library/Application Support/Coder/mcp-tokens/.
    static func saveToken(_ token: String, for name: String) {
        let trimmed = token.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        try? FileManager.default.createDirectory(at: tokensDir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let url = tokenURL(for: name)
        FileManager.default.createFile(atPath: url.path, contents: trimmed.data(using: .utf8), attributes: [.posixPermissions: 0o600])
        try? FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: url.path)
    }

    static func readToken(for name: String) -> String? {
        let url = tokenURL(for: name)
        guard let text = try? String(contentsOf: url, encoding: .utf8) else { return nil }
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    static func deleteToken(for name: String) {
        try? FileManager.default.removeItem(at: tokenURL(for: name))
    }

    /// Builds worker environment variables for enabled MCP servers (compatible with agyworker.coax #13833).
    /// Tokens are read from 0600 files and passed only in process env — never logged or serialized to config JSON.
    static func workerEnv() -> [String: String] {
        let servers = load().filter { $0.enabled }
        var env: [String: String] = [:]
        if let http = servers.first(where: { $0.transport == "http" }) {
            env["CODER_MCP_URL"] = http.endpoint
            if let tok = readToken(for: http.name) {
                env["CODER_MCP_TOKEN"] = tok
            }
        }
        if let stdio = servers.first(where: { $0.transport == "stdio" }) {
            env["CODER_MCP_STDIO"] = stdio.endpoint
        }
        return env
    }
}

/// PKCE S256 + localhost loopback callback flow for MCP OAuth sign-in.
final class McpOAuthFlow {
    private static var active: McpOAuthFlow?
    private var listenFD: Int32 = -1
    private var cancelled = false

    static func randomBase64URL(bytes count: Int) -> String {
        var bytes = [UInt8](repeating: 0, count: count)
        _ = SecRandomCopyBytes(kSecRandomDefault, count, &bytes)
        return Data(bytes).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    static func pkceChallenge(for verifier: String) -> String {
        let digest = SHA256.hash(data: Data(verifier.utf8))
        return Data(digest).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    static func start(serverName: String, authBaseURL: String, onComplete: @escaping (Bool, String, String?) -> Void) {
        active?.cancel()
        let flow = McpOAuthFlow()
        active = flow
        flow.run(serverName: serverName, authBaseURL: authBaseURL, onComplete: onComplete)
    }

    func cancel() {
        cancelled = true
        if listenFD >= 0 {
            Darwin.close(listenFD)
            listenFD = -1
        }
    }

    private func run(serverName: String, authBaseURL: String, onComplete: @escaping (Bool, String, String?) -> Void) {
        let verifier = Self.randomBase64URL(bytes: 32)
        let challenge = Self.pkceChallenge(for: verifier)
        let nonce = Self.randomBase64URL(bytes: 16)

        let fd = Darwin.socket(AF_INET, SOCK_STREAM, 0)
        guard fd >= 0 else {
            DispatchQueue.main.async { onComplete(false, nonce, "Could not open loopback socket.") }
            return
        }
        var yes: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &yes, socklen_t(MemoryLayout<Int32>.size))
        var addr = sockaddr_in()
        addr.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_port = 0
        addr.sin_addr = in_addr(s_addr: inet_addr("127.0.0.1"))
        let bindRes = withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.bind(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        guard bindRes == 0, Darwin.listen(fd, 4) == 0 else {
            Darwin.close(fd)
            DispatchQueue.main.async { onComplete(false, nonce, "Could not bind loopback callback listener.") }
            return
        }
        var bound = sockaddr_in()
        var len = socklen_t(MemoryLayout<sockaddr_in>.size)
        _ = withUnsafeMutablePointer(to: &bound) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.getsockname(fd, $0, &len)
            }
        }
        let port = UInt16(bigEndian: bound.sin_port)
        self.listenFD = fd

        let redirectURI = "http://127.0.0.1:\(port)/callback"
        var comps = URLComponents(string: authBaseURL)
        var items = comps?.queryItems ?? []
        items.append(URLQueryItem(name: "response_type", value: "code"))
        items.append(URLQueryItem(name: "redirect_uri", value: redirectURI))
        items.append(URLQueryItem(name: "state", value: nonce))
        items.append(URLQueryItem(name: "code_challenge", value: challenge))
        items.append(URLQueryItem(name: "code_challenge_method", value: "S256"))
        comps?.queryItems = items
        if let url = comps?.url {
            NSWorkspace.shared.open(url)
        }

        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            guard let self else { return }
            defer {
                if self.listenFD >= 0 {
                    Darwin.close(self.listenFD)
                    self.listenFD = -1
                }
            }
            var clientAddr = sockaddr_in()
            var clientLen = socklen_t(MemoryLayout<sockaddr_in>.size)
            let clientFD = withUnsafeMutablePointer(to: &clientAddr) {
                $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                    Darwin.accept(fd, $0, &clientLen)
                }
            }
            guard clientFD >= 0, !self.cancelled else { return }
            defer { Darwin.close(clientFD) }
            var buf = [UInt8](repeating: 0, count: 4096)
            let n = Darwin.read(clientFD, &buf, buf.count)
            let req = n > 0 ? String(decoding: buf[0..<n], as: UTF8.self) : ""
            var ok = false
            var gotToken: String? = nil
            if let firstLine = req.split(separator: "\r\n").first {
                let parts = firstLine.split(separator: " ")
                if parts.count >= 2, let cb = URLComponents(string: "http://127.0.0.1" + String(parts[1])) {
                    let qState = cb.queryItems?.first(where: { $0.name == "state" })?.value
                    let qCode = cb.queryItems?.first(where: { $0.name == "code" })?.value
                    let qTok = cb.queryItems?.first(where: { $0.name == "access_token" })?.value
                    if qState == nonce, let credential = qTok ?? qCode, !credential.isEmpty {
                        gotToken = credential
                        ok = true
                    }
                }
            }
            let body = ok
                ? "<html><body style='font-family:-apple-system;padding:32px'><h2>Signed in to \(serverName)</h2><p>You can close this tab and return to OpenAgents Desktop.</p></body></html>"
                : "<html><body style='font-family:-apple-system;padding:32px'><h2>Sign-in failed</h2><p>State mismatch or missing code. Please retry from OpenAgents Desktop.</p></body></html>"
            let resp = "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: \(body.utf8.count)\r\nConnection: close\r\n\r\n\(body)"
            _ = resp.withCString { Darwin.write(clientFD, $0, strlen($0)) }
            if ok, let tok = gotToken {
                McpServerStore.saveToken(tok, for: serverName)
            }
            DispatchQueue.main.async {
                onComplete(ok, nonce, ok ? nil : "OAuth callback validation failed.")
            }
        }
    }
}

struct DiscoveredSkill {
    var name: String
    var source: String   // "user", "project", or "json"
    var enabled: Bool
    var desc: String
    var path: String
}

enum SkillsCatalog {
    static var disabledURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/desktop/disabled-skills.json")
    }

    static func loadDisabled() -> Set<String> {
        guard let data = try? Data(contentsOf: disabledURL),
              let arr = try? JSONDecoder().decode([String].self, from: data) else {
            return []
        }
        return Set(arr)
    }

    static func saveDisabled(_ names: Set<String>) {
        let dir = disabledURL.deletingLastPathComponent()
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        if let data = try? JSONEncoder().encode(Array(names).sorted()) {
            try? data.write(to: disabledURL, options: .atomic)
        }
    }

    @discardableResult
    static func toggleDisabled(_ name: String) -> Bool {
        var set = loadDisabled()
        let nowEnabled: Bool
        if set.contains(name) {
            set.remove(name)
            nowEnabled = true
        } else {
            set.insert(name)
            nowEnabled = false
        }
        saveDisabled(set)
        return nowEnabled
    }

    private static func scanDirectory(_ root: URL, source: String, disabled: Set<String>, into out: inout [DiscoveredSkill], seen: inout Set<String>) {
        let names = ((try? FileManager.default.contentsOfDirectory(atPath: root.path)) ?? []).sorted()
        for name in names where out.count < 64 {
            let file = root.appendingPathComponent(name).appendingPathComponent("SKILL.md")
            guard let text = try? String(contentsOf: file, encoding: .utf8) else { continue }
            var description = ""
            if text.hasPrefix("---") {
                for line in text.split(separator: "\n").dropFirst() {
                    if line.hasPrefix("---") { break }
                    if line.hasPrefix("description:") {
                        description = String(line.dropFirst(12)).trimmingCharacters(in: .whitespaces)
                    }
                }
            }
            let key = source + ":" + name
            if seen.contains(key) { continue }
            seen.insert(key)
            out.append(DiscoveredSkill(name: name, source: source, enabled: !disabled.contains(name), desc: String(description.prefix(180)), path: file.path))
        }
    }

    private static func scanJSON(_ url: URL, disabled: Set<String>, into out: inout [DiscoveredSkill], seen: inout Set<String>) {
        struct JsonSkill: Codable { var name: String; var description: String?; var path: String? }
        guard let data = try? Data(contentsOf: url),
              let list = try? JSONDecoder().decode([JsonSkill].self, from: data) else { return }
        for item in list where out.count < 64 && !item.name.isEmpty {
            let key = "json:" + item.name
            if seen.contains(key) { continue }
            seen.insert(key)
            out.append(DiscoveredSkill(name: item.name, source: "json", enabled: !disabled.contains(item.name), desc: String((item.description ?? "").prefix(180)), path: item.path ?? url.path))
        }
    }

    static func discover(repoFolder: URL?) -> [DiscoveredSkill] {
        let disabled = loadDisabled()
        var out: [DiscoveredSkill] = []
        var seen: Set<String> = []
        if let repo = repoFolder {
            scanDirectory(repo.appendingPathComponent(".claude/skills"), source: "project", disabled: disabled, into: &out, seen: &seen)
            scanJSON(repo.appendingPathComponent(".claude/skills.json"), disabled: disabled, into: &out, seen: &seen)
        }
        let userRoot = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".claude/skills")
        scanDirectory(userRoot, source: "user", disabled: disabled, into: &out, seen: &seen)
        let desktopJSON = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/desktop/skills.json")
        scanJSON(desktopJSON, disabled: disabled, into: &out, seen: &seen)
        return out
    }

    static func pagePayload(repoFolder: URL?) -> String {
        let skills = discover(repoFolder: repoFolder)
        var lines: [String] = []
        lines.reserveCapacity(skills.count)
        for sk in skills {
            let cleanDesc = sk.desc.replacingOccurrences(of: "\t", with: " ").replacingOccurrences(of: "\n", with: " ")
            let cleanPath = sk.path.replacingOccurrences(of: "\t", with: " ").replacingOccurrences(of: "\n", with: " ")
            lines.append("\(sk.name)\t\(sk.source)\t\(sk.enabled ? "1" : "0")\t\(cleanDesc)\t\(cleanPath)")
        }
        return lines.joined(separator: "\n") + (lines.isEmpty ? "" : "\n")
    }

    static func enabledSkillsNote(repoFolder: URL?) -> String {
        let tools = coderToolsNote()
        let enabled = discover(repoFolder: repoFolder).filter { $0.enabled }
        var rows: [String] = []
        for sk in enabled.prefix(24) {
            rows.append("- \(sk.name) (\(sk.path)): \(sk.desc.prefix(300))")
        }
        if rows.isEmpty { return tools.isEmpty ? "" : tools + "The message:\n" }
        return tools + "Skills installed on this computer. When a message is about one, read its file with cat first, then do what it says:\n" + rows.joined(separator: "\n") + "\n\nThe message:\n"
    }
}

extension CoderWindow {
    func syncMcpSkills() {
        let mcpJSON = McpServerStore.loadJSON()
        let skillsPayload = SkillsCatalog.pagePayload(repoFolder: folder)
        _ = cd_mcp_load_json(page, mcpJSON)
        _ = cd_skills_load(page, skillsPayload)
        for r in runs.values where r.page != page {
            _ = cd_mcp_load_json(r.page, mcpJSON)
            _ = cd_skills_load(r.page, skillsPayload)
        }
        syncSkillsNote(for: page, isForeman: isForemanShown)
        configureWorkerEnv(run, isForeman: isForemanShown)
        for (id, r) in runs {
            syncSkillsNote(for: r.page, isForeman: id == Foreman.chatID)
            configureWorkerEnv(r, isForeman: id == Foreman.chatID)
        }
        wake()
    }

    private func persistMcpFromPage() {
        var buf = [CChar](repeating: 0, count: 32768)
        let n = cd_mcp_save_json(page, &buf, Int32(buf.count))
        if n > 0 {
            McpServerStore.saveJSON(String(cString: buf))
        }
        syncMcpSkills()
    }

    func mcpAddDialog() {
        var d = CoderDialog(size: .md, title: "Add MCP Server", body: "Configure an HTTP or stdio Model Context Protocol server. Enabled servers are injected into Microcoder, Claude, and Antigravity. Optional bearer tokens are saved in a 0600 file and never logged.")
        d.fields = [
            .init(label: "Name", value: "", hint: "e.g. github or local-fs"),
            .init(label: "Transport (http or stdio)", value: "http", hint: "http or stdio"),
            .init(label: "URL or Command", value: "", hint: "https://... or npx -y @modelcontextprotocol/..."),
            .init(label: "Bearer Token (optional)", value: "", hint: "Stored in 0600 file, never logged", secure: true)
        ]
        d.buttons = [.init(title: "Cancel"), .init(title: "Add Server", kind: .primary)]
        d.cancel = 0; d.confirm = 1
        present(d) { [weak self] answer, values in
            guard let self else { return }
            if answer == 1 {
                let name = (values.count > 0 ? values[0] : "").trimmingCharacters(in: .whitespacesAndNewlines)
                let trRaw = (values.count > 1 ? values[1] : "http").trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
                let endpoint = (values.count > 2 ? values[2] : "").trimmingCharacters(in: .whitespacesAndNewlines)
                let token = (values.count > 3 ? values[3] : "").trimmingCharacters(in: .whitespacesAndNewlines)
                if name.isEmpty || endpoint.isEmpty {
                    self.present(.notice("Missing fields", "Enter both a server name and a URL or stdio command."))
                    return
                }
                let tr: Int32 = trRaw == "stdio" ? 1 : 0
                if !token.isEmpty {
                    McpServerStore.saveToken(token, for: name)
                }
                let idx = cd_mcp_add(self.page, name, tr, endpoint, 1)
                if idx >= 0 {
                    self.persistMcpFromPage()
                    cd_toast(self.page, "Added MCP server \(name)", 0)
                }
            }
            self.refocus()
        }
    }

    func mcpToggleSelected() {
        persistMcpFromPage()
        wake()
    }

    func mcpRemoveSelected() {
        let idx = Int(cd_mcp_target(page))
        let servers = McpServerStore.load()
        guard idx >= 0 && idx < servers.count else { return }
        let name = servers[idx].name
        var d = CoderDialog(size: .sm, title: "Remove MCP Server?", body: "Remove \"\(name)\" and delete its stored token file?")
        d.buttons = [.init(title: "Cancel"), .init(title: "Remove", kind: .danger)]
        d.cancel = 0; d.confirm = 0
        present(d) { [weak self] answer, _ in
            guard let self else { return }
            if answer == 1 {
                McpServerStore.deleteToken(for: name)
                _ = cd_mcp_remove(self.page, Int32(idx))
                self.persistMcpFromPage()
                cd_toast(self.page, "Removed MCP server \(name)", 0)
            }
            self.refocus()
        }
    }

    func mcpStartOAuth() {
        var srvBuf = [CChar](repeating: 0, count: 128)
        let n = cd_mcp_oauth_server(page, &srvBuf, Int32(srvBuf.count))
        var serverName = n > 0 ? String(cString: srvBuf) : ""
        let servers = McpServerStore.load()
        let idx = Int(cd_mcp_target(page))
        var authURL = ""
        if serverName.isEmpty, idx >= 0 && idx < servers.count {
            serverName = servers[idx].name
            authURL = servers[idx].endpoint
        } else if let found = servers.first(where: { $0.name == serverName }) {
            authURL = found.endpoint
        }
        guard !serverName.isEmpty, !authURL.isEmpty else {
            present(.notice("MCP OAuth", "Select an HTTP MCP server first."))
            return
        }
        cd_toast(page, "Opening browser to sign in to \(serverName)…", 0)
        McpOAuthFlow.start(serverName: serverName, authBaseURL: authURL) { [weak self] ok, nonce, err in
            guard let self else { return }
            _ = cd_mcp_oauth_complete(self.page, serverName, nonce, ok ? 1 : 0)
            if ok {
                self.persistMcpFromPage()
                cd_toast(self.page, "Signed in to MCP server \(serverName)", 0)
            } else {
                cd_toast(self.page, err ?? "MCP OAuth sign-in failed", 1)
            }
            self.wake()
        }
    }

    func skillToggleSelected() {
        let idx = cd_skills_target(page)
        var nameBuf = [CChar](repeating: 0, count: 128)
        let n = cd_skill_name(page, idx, &nameBuf, Int32(nameBuf.count))
        guard n > 0 else { return }
        let name = String(cString: nameBuf)
        _ = SkillsCatalog.toggleDisabled(name)
        syncMcpSkills()
    }
}
