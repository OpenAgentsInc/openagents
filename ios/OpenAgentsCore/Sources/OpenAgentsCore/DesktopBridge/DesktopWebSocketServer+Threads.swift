#if os(macOS)
import Foundation

extension DesktopWebSocketServer {
    // MARK: - Thread/Index Handlers

    func registerThreadHandlers() {
        // threads/list
        router.register(method: "threads/list") { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleThreadsList(id: id, params: params, rawDict: rawDict, client: client)
        }

        // thread/load_latest_typed
        router.register(method: "thread/load_latest_typed") { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleThreadLoadLatestTyped(id: id, params: params, rawDict: rawDict, client: client)
        }

        // index.status
        router.register(method: "index.status") { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleIndexStatus(id: id, params: params, rawDict: rawDict, client: client)
        }
    }

    func handleThreadsList(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        struct Params: Codable { let topK: Int? }
        let topK: Int = {
            if let p = rawDict["params"], let d = try? JSONSerialization.data(withJSONObject: p), let pr = try? JSONDecoder().decode(Params.self, from: d) { return pr.topK ?? 10 }
            return 10
        }()
        let limit = min(10, max(1, topK))
        let base = CodexScanner.defaultBaseDir()
        let urls = CodexScanner.listRecentTopN(at: base, topK: limit)
        var items = urls.map { CodexScanner.makeSummary(for: $0, base: base) }
        items.sort { ($0.last_message_ts ?? $0.updated_at) > ($1.last_message_ts ?? $1.updated_at) }
        struct Result: Codable { let items: [ThreadSummary] }
        let result = Result(items: items)
        JsonRpcRouter.sendResponse(id: id, result: result) { text in
            client.send(text: text)
        }
    }

    func handleThreadLoadLatestTyped(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        struct LatestTypedResult: Codable {
            let id: String
            let updates: [ACP.Client.SessionUpdate]
        }

        // Load Claude Code sessions ONLY (fall back to Codex if none found)
        let claudeBase = ClaudeCodeScanner.defaultBaseDir()
        let claudeFiles = ClaudeCodeScanner.listRecentTopN(at: claudeBase, topK: 1)

        var file: URL?
        var provider: String = "claude-code"
        var base: URL = claudeBase

        if let clf = claudeFiles.first {
            // Use Claude Code session
            file = clf
            provider = "claude-code"
            base = claudeBase
        } else {
            // Fallback to Codex only if no Claude Code sessions exist
            let codexBase = CodexScanner.defaultBaseDir()
            let codexFiles = CodexScanner.listRecentTopN(at: codexBase, topK: 1)
            if let cf = codexFiles.first {
                file = cf
                provider = "codex"
                base = codexBase
            }
        }

        if let file = file {
            let tid: String
            if provider == "codex" {
                tid = CodexScanner.scanForThreadID(file) ?? CodexScanner.relativeId(for: file, base: base)
            } else {
                tid = ClaudeCodeScanner.scanForSessionID(file) ?? ClaudeCodeScanner.relativeId(for: file, base: base)
            }

            // Read and translate to ACP
            let lines = DesktopWebSocketServer.tailJSONLLines(at: file, maxBytes: 1_000_000, maxLines: 16000)
            let updates = DesktopWebSocketServer.makeTypedUpdates(from: lines, provider: provider)

            let result = LatestTypedResult(id: tid, updates: updates)
            JsonRpcRouter.sendResponse(id: id, result: result) { text in
                OpenAgentsLog.server.debug("send rpc result method=thread/load_latest_typed id=\(id.value) provider=\(provider) bytes=\(text.utf8.count)")
                client.send(text: text)
            }
        } else {
            JsonRpcRouter.sendError(id: id, code: -32002, message: "No threads found") { text in
                client.send(text: text)
            }
        }
    }

    func handleIndexStatus(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        // Minimal Phase-1: respond and emit a tiny synthetic stream for visibility
        // Params may contain session_id to stream under
        let sidStr = (params?["session_id"] as? String) ?? UUID().uuidString
        let sid = ACPSessionId(sidStr)

        struct StatusResult: Codable {
            let workspace_path: String
            let exists: Bool
            let files_indexed: Int?
            let chunks: Int?
        }

        // Default workspace path (env override TEST_WORKSPACE_ROOT)
        let ws = ProcessInfo.processInfo.environment["TEST_WORKSPACE_ROOT"] ?? "\(NSHomeDirectory())/code/openagents"
        let exists = FileManager.default.fileExists(atPath: ws)

        let result = StatusResult(workspace_path: ws, exists: exists, files_indexed: nil, chunks: nil)
        JsonRpcRouter.sendResponse(id: id, result: result) { text in
            OpenAgentsLog.server.debug("send rpc result method=index.status id=\(id.value) text=\(text)")
            client.send(text: text)
        }

        // Emit synthetic tool stream: index.rebuild started→completed
        let call = ACPToolCallWire(call_id: UUID().uuidString, name: "index.rebuild", arguments: [
            "workspace": AnyEncodable(ws)
        ])
        await sendSessionUpdate(sessionId: sid, update: .toolCall(call))

        // started
        let started = ACPToolCallUpdateWire(call_id: call.call_id, status: .started, output: nil, error: nil, _meta: ["progress": AnyEncodable(0.0)])
        await sendSessionUpdate(sessionId: sid, update: .toolCallUpdate(started))

        // completed with tiny payload
        let payload: [String: AnyEncodable] = [
            "workspace": AnyEncodable(ws),
            "indexed": AnyEncodable(false)
        ]
        let completed = ACPToolCallUpdateWire(call_id: call.call_id, status: .completed, output: AnyEncodable(payload), error: nil, _meta: nil)
        await sendSessionUpdate(sessionId: sid, update: .toolCallUpdate(completed))
    }
}
#endif
