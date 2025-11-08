#if os(macOS)
import Foundation

extension DesktopWebSocketServer {
    // MARK: - File System Handlers

    func registerFileSystemHandlers() {
        // fs/readTextFile
        router.register(method: ACPRPC.fsReadTextFile) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleFsReadTextFile(id: id, params: params, rawDict: rawDict, client: client)
        }

        // fs/writeTextFile
        router.register(method: ACPRPC.fsWriteTextFile) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleFsWriteTextFile(id: id, params: params, rawDict: rawDict, client: client)
        }
    }

    func handleFsReadTextFile(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        struct ReqURI: Codable { let session_id: ACPSessionId; let uri: String }
        struct ReqPath: Codable { let session_id: ACPSessionId; let path: String; let line: UInt32?; let limit: UInt32? }
        struct Resp: Codable { let content: String }

        if let p = rawDict["params"], let d = try? JSONSerialization.data(withJSONObject: p) {
            var text: String? = nil
            var attempted: String? = nil
            if let req = try? JSONDecoder().decode(ReqPath.self, from: d) {
                attempted = req.path
                text = DesktopWebSocketServer.readText(fromURI: req.path)
            } else if let req = try? JSONDecoder().decode(ReqURI.self, from: d) {
                attempted = req.uri
                text = DesktopWebSocketServer.readText(fromURI: req.uri)
            }
            if let text = text {
                JsonRpcRouter.sendResponse(id: id, result: Resp(content: text)) { responseText in
                    OpenAgentsLog.server.debug("send rpc result method=\(ACPRPC.fsReadTextFile) id=\(id.value) bytes=\(responseText.utf8.count)")
                    client.send(text: responseText)
                }
            } else {
                let msg = "Resource not found"
                JsonRpcRouter.sendError(id: id, code: -32002, message: attempted.map { "\(msg): \($0)" } ?? msg) { text in
                    client.send(text: text)
                }
            }
        } else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Invalid params") { text in
                client.send(text: text)
            }
        }
    }

    func handleFsWriteTextFile(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        struct ReqPath: Codable { let session_id: ACPSessionId; let path: String; let content: String }
        struct ReqURI: Codable { let session_id: ACPSessionId; let uri: String; let text: String }
        struct Resp: Codable { let _meta: [String:String]? }

        if let p = rawDict["params"], let d = try? JSONSerialization.data(withJSONObject: p) {
            var ok = false
            if let req = try? JSONDecoder().decode(ReqPath.self, from: d) {
                ok = DesktopWebSocketServer.writeText(toURI: req.path, text: req.content)
            } else if let req = try? JSONDecoder().decode(ReqURI.self, from: d) {
                ok = DesktopWebSocketServer.writeText(toURI: req.uri, text: req.text)
            }
            if ok {
                JsonRpcRouter.sendResponse(id: id, result: Resp(_meta: nil)) { responseText in
                    OpenAgentsLog.server.debug("send rpc result method=\(ACPRPC.fsWriteTextFile) id=\(id.value) bytes=\(responseText.utf8.count)")
                    client.send(text: responseText)
                }
            } else {
                JsonRpcRouter.sendError(id: id, code: -32603, message: "Write failed") { text in
                    client.send(text: text)
                }
            }
        } else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Invalid params") { text in
                client.send(text: text)
            }
        }
    }
}
#endif
