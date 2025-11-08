#if os(macOS)
import Foundation

extension DesktopWebSocketServer {
    // MARK: - Terminal Handler

    func registerTerminalHandler() {
        router.register(method: ACPRPC.terminalRun) { [weak self] id, params, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            await self.handleTerminalRun(id: id, params: params, rawDict: rawDict, client: client)
        }
    }

    func handleTerminalRun(id: JSONRPC.ID, params: [String: Any]?, rawDict: [String: Any], client: Client) async {
        struct Req: Codable { let session_id: ACPSessionId; let command: [String]; let cwd: String?; let env: [String:String]?; let output_byte_limit: Int? }
        struct Resp: Codable { let output: String; let truncated: Bool; let exit_status: Int32? }

        guard let p = rawDict["params"], let d = try? JSONSerialization.data(withJSONObject: p), let req = try? JSONDecoder().decode(Req.self, from: d) else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Invalid params") { text in
                client.send(text: text)
            }
            return
        }

        let result = DesktopWebSocketServer.runCommand(req.command, cwd: req.cwd, env: req.env, limit: req.output_byte_limit)
        JsonRpcRouter.sendResponse(id: id, result: Resp(output: result.output, truncated: result.truncated, exit_status: result.exitStatus)) { responseText in
            OpenAgentsLog.bridgeServer.debug("send rpc result method=\(ACPRPC.terminalRun) id=\(id.value) bytes=\(responseText.utf8.count)")
            client.send(text: responseText)
        }
    }
}
#endif
