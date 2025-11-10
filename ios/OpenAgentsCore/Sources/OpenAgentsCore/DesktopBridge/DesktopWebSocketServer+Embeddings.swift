#if os(macOS)
import Foundation

extension DesktopWebSocketServer {
    // MARK: - Registration
    func registerEmbeddingHandlers() {
        router.register(method: ACPRPC.embeddingGenerate) { [weak self] id, _, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            Task { [weak self] in
                await self?.handleEmbeddingGenerate(id: id, rawDict: rawDict, client: client)
            }
        }
        router.register(method: ACPRPC.embeddingStore) { [weak self] id, _, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            Task { [weak self] in
                await self?.handleEmbeddingStore(id: id, rawDict: rawDict, client: client)
            }
        }
        router.register(method: ACPRPC.embeddingSearch) { [weak self] id, _, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            Task { [weak self] in
                await self?.handleEmbeddingSearch(id: id, rawDict: rawDict, client: client)
            }
        }
        router.register(method: ACPRPC.embeddingStoreBatch) { [weak self] id, _, rawDict in
            guard let self = self, let client = self.currentClient else { return }
            Task { [weak self] in
                await self?.handleEmbeddingStoreBatch(id: id, rawDict: rawDict, client: client)
            }
        }
    }

    // MARK: - Handlers
    private func handleEmbeddingGenerate(id: JSONRPC.ID, rawDict: [String: Any], client: Client) async {
        guard let service = await ensureEmbeddingService() else {
            JsonRpcRouter.sendError(id: id, code: -32603, message: "Embedding service unavailable") { text in
                client.send(text: text)
            }
            return
        }
        guard let params = rawDict["params"],
              let data = try? JSONSerialization.data(withJSONObject: params),
              let request = try? JSONDecoder().decode(EmbedRequest.self, from: data) else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Invalid params") { text in
                client.send(text: text)
            }
            return
        }
        do {
            let response = try await service.generateEmbeddings(request)
            JsonRpcRouter.sendResponse(id: id, result: response) { text in
                client.send(text: text)
            }
        } catch {
            JsonRpcRouter.sendError(id: id, code: -32603, message: error.localizedDescription) { text in
                client.send(text: text)
            }
        }
    }

    private func handleEmbeddingStore(id: JSONRPC.ID, rawDict: [String: Any], client: Client) async {
        guard let service = await ensureEmbeddingService() else {
            JsonRpcRouter.sendError(id: id, code: -32603, message: "Embedding service unavailable") { text in
                client.send(text: text)
            }
            return
        }
        guard let params = rawDict["params"] as? [String: Any],
              let itemId = params["id"] as? String,
              let collection = params["collection"] as? String,
              let text = params["text"] as? String else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Missing required params") { text in
                client.send(text: text)
            }
            return
        }
        let metadata = params["metadata"] as? [String: String]
        do {
            try await service.storeEmbedding(id: itemId, collection: collection, text: text, metadata: metadata)
            JsonRpcRouter.sendResponse(id: id, result: ["success": true]) { text in
                client.send(text: text)
            }
        } catch {
            JsonRpcRouter.sendError(id: id, code: -32603, message: error.localizedDescription) { text in
                client.send(text: text)
            }
        }
    }

    private func handleEmbeddingSearch(id: JSONRPC.ID, rawDict: [String: Any], client: Client) async {
        guard let service = await ensureEmbeddingService() else {
            JsonRpcRouter.sendError(id: id, code: -32603, message: "Embedding service unavailable") { text in
                client.send(text: text)
            }
            return
        }
        guard let params = rawDict["params"],
              let data = try? JSONSerialization.data(withJSONObject: params),
              let request = try? JSONDecoder().decode(SemanticSearchRequest.self, from: data) else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Invalid params") { text in
                client.send(text: text)
            }
            return
        }
        do {
            let response = try await service.semanticSearch(request)
            JsonRpcRouter.sendResponse(id: id, result: response) { text in
                client.send(text: text)
            }
        } catch {
            JsonRpcRouter.sendError(id: id, code: -32603, message: error.localizedDescription) { text in
                client.send(text: text)
            }
        }
    }

    private func handleEmbeddingStoreBatch(id: JSONRPC.ID, rawDict: [String: Any], client: Client) async {
        guard let service = await ensureEmbeddingService() else {
            JsonRpcRouter.sendError(id: id, code: -32603, message: "Embedding service unavailable") { text in
                client.send(text: text)
            }
            return
        }
        guard let params = rawDict["params"] as? [String: Any],
              let collection = params["collection"] as? String,
              let items = params["items"] as? [[String: Any]] else {
            JsonRpcRouter.sendError(id: id, code: -32602, message: "Invalid params") { text in
                client.send(text: text)
            }
            return
        }
        do {
            let mapped: [(id: String, text: String, metadata: [String: String]?)] = try items.map { dict in
                guard let id = dict["id"] as? String, let text = dict["text"] as? String else {
                    throw NSError(domain: "EmbeddingStoreBatch", code: -1, userInfo: [NSLocalizedDescriptionKey: "Missing id/text in batch item"])
                }
                let metadata = dict["metadata"] as? [String: String]
                return (id: id, text: text, metadata: metadata)
            }
            try await service.storeBatch(items: mapped, collection: collection)
            JsonRpcRouter.sendResponse(id: id, result: ["success": true, "count": mapped.count]) { text in
                client.send(text: text)
            }
        } catch {
            JsonRpcRouter.sendError(id: id, code: -32603, message: error.localizedDescription) { text in
                client.send(text: text)
            }
        }
    }

    // MARK: - Helpers
    private func ensureEmbeddingService() async -> EmbeddingService? {
        if let svc = self.embeddingService { return svc }
        guard let db = self.tinyvexDb else { return nil }
        do {
            let svc = try await EmbeddingService(db: db)
            self.embeddingService = svc
            return svc
        } catch {
            OpenAgentsLog.bridgeServer.error("Failed to init EmbeddingService: \(error.localizedDescription)")
            return nil
        }
    }
}
#endif

