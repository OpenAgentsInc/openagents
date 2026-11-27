// Firm reason: 6 (standalone Swift verification check for CloudChatSync.swift + #authority account-chat-cloud-wire.coax; #14093).
import CryptoKit
import Foundation

func check(_ cond: @autoclosure () -> Bool, _ msg: String) {
    if !cond() {
        fputs("FAIL: \(msg)\n", stderr)
        exit(1)
    }
}

final class MockServer: @unchecked Sendable {
    private let lock = NSLock()
    var offline = false
    var serverError = false
    var requests: [(method: String, path: String, body: String)] = []
    var seenOps: [String: String] = [:]
    var committedOps: [String] = []
    var remoteDetailJSON: String = ""

    func handle(_ req: URLRequest) async throws -> (Data, Int) {
        let method = req.httpMethod ?? "GET"
        let path = req.url?.path ?? ""
        let body = req.httpBody.flatMap { String(data: $0, encoding: .utf8) } ?? ""
        let (isOff, isErr, detail) = lock.withLock { (offline, serverError, remoteDetailJSON) }

        if isOff {
            throw URLError(.notConnectedToInternet)
        }
        if isErr {
            return (Data("{\"error\":\"temporary\"}".utf8), 503)
        }
        let replayResult: (Data, Int)? = lock.withLock {
            requests.append((method, path, body))
            if method == "POST",
               let d = body.data(using: .utf8),
               let obj = try? JSONSerialization.jsonObject(with: d) as? [String: Any],
               let opID = obj["operation_id"] as? String {
                if let prev = seenOps[opID] {
                    if prev == body {
                        return (Data("{\"replayed\":true}".utf8), 200)
                    }
                    return (Data("{\"error\":\"conflict\"}".utf8), 409)
                }
                seenOps[opID] = body
                committedOps.append(opID)
            }
            return nil
        }
        if let replayResult { return replayResult }

        if method == "GET" && path.hasPrefix("/api/chat/v2/conversations/") {
            return (Data(detail.utf8), 200)
        }
        return (Data("{\"replayed\":false}".utf8), 200)
    }

    var uniqueCommittedCount: Int {
        lock.lock(); defer { lock.unlock() }
        return committedOps.count
    }

    var allRequests: [(method: String, path: String, body: String)] {
        lock.lock(); defer { lock.unlock() }
        return requests
    }
}

@main
struct CloudChatSyncCheck {
    static func main() async {
        let fm = FileManager.default
        let tmp = fm.temporaryDirectory.appendingPathComponent("oa-cloud-chat-sync-\(UUID().uuidString)", isDirectory: true)
        try! fm.createDirectory(at: tmp, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: tmp) }

        // 1. Deterministic conversation & turn ID mapping + canonical cnv_ preservation.
        let cidA = CloudChatSync.conversationID(for: "550e8400-e29b-41d4-a716-446655440000")
        let cidB = CloudChatSync.conversationID(for: "550e8400-e29b-41d4-a716-446655440000")
        check(cidA == cidB, "conversationID is deterministic")
        check(CloudChatSync.isCanonicalConvID(cidA), "conversationID has canonical cnv_<32hex> format")
        let already = "cnv_0123456789abcdef0123456789abcdef"
        check(CloudChatSync.conversationID(for: already) == already, "canonical cnv_ preserved verbatim")
        let tid1 = CloudChatSync.turnID(for: cidA, ordinal: 1)
        let tid2 = CloudChatSync.turnID(for: cidA, ordinal: 2)
        check(tid1.hasPrefix("trn_") && tid1.count == 36 && tid1 != tid2, "turnID deterministic and distinct per ordinal")

        // 2. Compute & cost provenance strictly enforces $0 for local Inkling 2 Mini.
        let localProv = CloudChatSync.classifyModel("inkling-2-mini", isLocalHint: false, observedCostMsat: 999)
        check(localProv.compute == "local" && localProv.costMsat == 0, "inkling-2-mini strictly local and 0 msat")
        let cloudProv = CloudChatSync.classifyModel("google/gemini-4-neon", isLocalHint: false, observedCostMsat: 1250)
        check(cloudProv.compute == "cloud" && cloudProv.costMsat == 1250, "gemini-4-neon cloud with cost preserved")
        check(CloudChatSync.validateCompute(compute: "local", costMsat: 0), "C authority accepts local 0 msat")
        check(!CloudChatSync.validateCompute(compute: "local", costMsat: 1), "C authority rejects local non-zero msat")
        check(CloudChatSync.validateCompute(compute: "cloud", costMsat: 4200), "C authority accepts cloud cost")

        // 3. Network drop & offline outbox durability + idempotent reconciliation on reconnect.
        let server = MockServer()
        server.offline = true
        let sync1 = CloudChatSync(
            stateDir: tmp,
            originProvider: { "http://127.0.0.1:9999" },
            isSignedIn: { true },
            authorizer: { req in
                var r = req
                r.setValue("Bearer test_token", forHTTPHeaderField: "Authorization")
                return r
            },
            transport: { req in try await server.handle(req) }
        )

        // Stream rich oa-worker lines (thinking, tool call + diff + result, approval, steer, ended with usage) while OFFLINE!
        sync1.observeWorkerLine(chatID: "desktop-local-1", title: "Local Inkling Run", folder: "/work/oa", model: "inkling-2-mini", isLocal: true, line: "{\"kind\":\"turn\",\"ask\":\"Fix bug\"}")
        sync1.observeWorkerLine(chatID: "desktop-local-1", title: "Local Inkling Run", folder: "/work/oa", model: "inkling-2-mini", isLocal: true, line: "{\"kind\":\"thought\",\"text\":\"checking parser\"}")
        sync1.observeWorkerLine(chatID: "desktop-local-1", title: "Local Inkling Run", folder: "/work/oa", model: "inkling-2-mini", isLocal: true, line: "{\"kind\":\"command\",\"id\":\"c1\",\"tool\":\"apply_patch\",\"args\":\"{\\\"file\\\":\\\"a.coax\\\"}\",\"diff\":\"@@ -1 +1 @@\\n-a\\n+b\\n\",\"done\":true,\"failed\":false,\"text\":\"patched\"}")
        sync1.observeWorkerLine(chatID: "desktop-local-1", title: "Local Inkling Run", folder: "/work/oa", model: "inkling-2-mini", isLocal: true, line: "{\"kind\":\"ask\",\"text\":\"Run tests?\"}")
        sync1.observeWorkerLine(chatID: "desktop-local-1", title: "Local Inkling Run", folder: "/work/oa", model: "inkling-2-mini", isLocal: true, line: "{\"kind\":\"steered\",\"text\":\"Yes run all\"}")
        sync1.observeWorkerLine(chatID: "desktop-local-1", title: "Local Inkling Run", folder: "/work/oa", model: "inkling-2-mini", isLocal: true, line: "{\"kind\":\"delta\",\"text\":\"All tests pass!\"}")
        sync1.observeWorkerLine(chatID: "desktop-local-1", title: "Local Inkling Run", folder: "/work/oa", model: "inkling-2-mini", isLocal: true, line: "{\"kind\":\"ended\",\"ending\":\"done\",\"cost_msat\":500,\"usage\":{\"input_tokens\":120,\"output_tokens\":45,\"reasoning_tokens\":18}}")
        sync1.syncMeta(chatID: "desktop-local-1", title: "Renamed Local Run", pinned: true, archived: false)

        let deliveredOffline = await sync1.flushOutbox(ignoreBackoff: true)
        check(deliveredOffline == 0, "offline flush delivers 0")
        check(sync1.pendingOutboxCount == 8, "all 8 operations buffered in durable offline outbox (got \(sync1.pendingOutboxCount))")

        // Simulate app restart / reconnect by creating a brand new CloudChatSync from the same stateDir!
        server.offline = false
        let sync2 = CloudChatSync(
            stateDir: tmp,
            originProvider: { "http://127.0.0.1:9999" },
            isSignedIn: { true },
            authorizer: { req in
                var r = req
                r.setValue("Bearer test_token", forHTTPHeaderField: "Authorization")
                return r
            },
            transport: { req in try await server.handle(req) }
        )
        check(sync2.pendingOutboxCount == 8, "restarted CloudChatSync loaded all 8 durable outbox items")
        let deliveredOnline = await sync2.flushOutbox(ignoreBackoff: true)
        check(deliveredOnline == 8, "reconnect flushed all 8 items (got \(deliveredOnline))")
        check(sync2.pendingOutboxCount == 0, "outbox empty after reconnect flush")
        check(server.uniqueCommittedCount == 8, "server committed exactly 8 unique operations")

        // Verify local $0 provenance and rich diff/tool/usage fields reached the server via #authority codec.
        let reqs = server.allRequests
        check(reqs.count == 8, "8 requests received")
        let patchReq = reqs[2].body
        check(patchReq.contains("\"compute\":\"local\"") && patchReq.contains("\"cost_msat\":0"), "local model strictly 0 msat on wire")
        check(patchReq.contains("\"tool_name\":\"apply_patch\"") && patchReq.contains("@@ -1 +1 @@"), "tool call and diff round-tripped")
        let endReq = reqs[6].body
        check(endReq.contains("\"cost_msat\":0") && endReq.contains("\"reasoning_tokens\":18"), "ended event preserves usage and forces $0 local cost")
        check(reqs[7].path.hasSuffix("/meta") && reqs[7].body.contains("\"pinned\":true"), "meta sync updates title/pin")

        // 4. Incoming Web/Mobile SSE event -> fetches conversation & materializes into Desktop callback + persists cursor.
        let webConvID = "cnv_abcdef0123456789abcdef0123456789"
        server.remoteDetailJSON = """
        {
          "conversation": {
            "id": "\(webConvID)",
            "title": "Started on Mobile",
            "pinned": true,
            "archived": false,
            "workspace": "/work/mobile",
            "model": "google/gemini-4-neon"
          },
          "turns": [
            { "user_text": "Deploy docs", "output": "Deployed cleanly!" }
          ],
          "events": []
        }
        """
        final class RemoteBox: @unchecked Sendable { var value: RemoteCloudConversation? }
        let materialized = RemoteBox()
        sync2.onRemoteConversation = { remote in materialized.value = remote }
        await sync2.handleSSEEvent(
            id: 42,
            event: "conversation_created",
            data: "{\"conversation_id\":\"\(webConvID)\",\"source_device\":\"mobile-ios\"}"
        )
        check(sync2.currentCursor == 42, "SSE cursor persisted at 42")
        check(materialized.value?.conversationID == webConvID, "remote Web/Mobile conversation materialized")
        check(materialized.value?.title == "Started on Mobile" && materialized.value?.pinned == true, "remote metadata preserved")
        check(materialized.value?.chatJSON.contains("Deploy docs") == true && materialized.value?.chatJSON.contains("Deployed cleanly!") == true, "remote turn messages preserved in Desktop chat JSON")

        print("ok   cloud-chat-sync-check: all Desktop CloudChatSync + #authority wire codec assertions passed")
    }
}
