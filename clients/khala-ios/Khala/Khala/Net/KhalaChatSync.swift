import Foundation

/// Minimal Khala Sync bridge for the interim SwiftUI app.
///
/// This intentionally speaks only the existing public wire routes:
/// - `POST /api/sync/push` for `chat.createThread` / `chat.appendMessage`
/// - `POST /api/sync/bootstrap` for owner-private `chat_thread` metadata
///
/// Message bodies are sent only as private sync mutations to the authenticated
/// owner's `scope.thread.<threadId>`; the evidence bundle path below never
/// records body text.
extension KhalaClient {
    static let chatSyncPushURL = URL(string: "https://openagents.com/api/sync/push")!
    static let chatSyncBootstrapURL = URL(string: "https://openagents.com/api/sync/bootstrap")!
    static let chatSyncPushRouteRef = "route.khala_sync.push.v0_1"
    static let chatSyncBootstrapRouteRef = "route.khala_sync.bootstrap.v0_1"

    static let chatSyncProtocolVersion = 1
    static let chatSyncSchemaVersion = 1

    static let chatCreateThreadMutatorName = "chat.createThread"
    static let chatAppendMessageMutatorName = "chat.appendMessage"
    static let chatThreadEntityType = "chat_thread"

    struct ChatSyncMutationStatus: Equatable {
        let mutationId: Int
        let status: String
        let errorCode: String?
        let errorMessageSafe: String?
    }

    struct ChatSyncPushResult: Equatable {
        let routeRef: String
        let threadId: String
        let messageId: String
        let mutationIds: [Int]
        let lastMutationId: Int
        let statuses: [ChatSyncMutationStatus]
    }

    struct SyncedChatThread: Equatable {
        let threadId: String
        let ownerUserId: String
        let title: String
        let messageCount: Int
        let lastMessageAt: String?
        let createdAt: String
        let updatedAt: String
    }

    struct ChatSyncBootstrapResult: Equatable {
        let routeRef: String
        let scope: String
        let cursor: Int?
        let threads: [SyncedChatThread]
    }

    static func chatSyncThreadId(for conversationId: UUID) -> String {
        "ios.thread.\(conversationId.uuidString.lowercased())"
    }

    static func chatSyncMessageId(for messageId: UUID) -> String {
        "ios.message.\(messageId.uuidString.lowercased())"
    }

    static func pushChatSyncTurn(
        threadId: String,
        title: String,
        messageId: String,
        body: String,
        apiKey: String,
        stateStore: KhalaChatSyncStateStore = .shared,
        session: URLSession = .shared
    ) async throws -> ChatSyncPushResult {
        let trimmedKey = apiKey.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedKey.isEmpty else { throw KhalaError.missingKey }
        let trimmedBody = body.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedBody.isEmpty else {
            throw KhalaError.syncProtocol("Cannot sync an empty chat message.")
        }

        var state = stateStore.load()
        var nextMutationId = state.lastMutationId + 1
        var createMutationId: Int?
        var mutations: [[String: Any]] = []

        if !state.syncedThreadIds.contains(threadId) {
            createMutationId = nextMutationId
            nextMutationId += 1
            mutations.append([
                "mutationId": createMutationId!,
                "name": chatCreateThreadMutatorName,
                "argsJson": try sortedJsonString([
                    "threadId": threadId,
                    "title": title.trimmingCharacters(in: .whitespacesAndNewlines),
                ]),
            ])
        }

        mutations.append([
            "mutationId": nextMutationId,
            "name": chatAppendMessageMutatorName,
            "argsJson": try sortedJsonString([
                "threadId": threadId,
                "messageId": messageId,
                "body": trimmedBody,
            ]),
        ])

        let response = try await postChatSyncPush(
            apiKey: trimmedKey,
            body: [
                "protocolVersion": chatSyncProtocolVersion,
                "schemaVersion": chatSyncSchemaVersion,
                "clientGroupId": state.clientGroupId,
                "clientId": state.clientId,
                "mutations": mutations,
            ],
            session: session
        )

        let statuses = response.results.map {
            ChatSyncMutationStatus(
                mutationId: $0.mutationId,
                status: $0.status,
                errorCode: $0.errorCode,
                errorMessageSafe: $0.errorMessageSafe
            )
        }

        state.lastMutationId = max(state.lastMutationId, response.lastMutationId)
        if createMutationId == nil {
            state.syncedThreadIds.insert(threadId)
        } else if let createMutationId,
                  statuses.contains(where: { result in
                      result.mutationId == createMutationId
                          && (result.status == "applied"
                              || result.status == "duplicate"
                              || result.errorCode == "thread_exists")
                  }) {
            state.syncedThreadIds.insert(threadId)
        }
        if statuses.contains(where: { $0.errorCode == "thread_not_found" }) {
            state.syncedThreadIds.remove(threadId)
        }
        stateStore.save(state)

        if let rejected = statuses.first(where: { !isBenignChatSyncRejection($0, createMutationId: createMutationId) }) {
            throw KhalaError.syncRejected(
                rejected.errorMessageSafe
                    ?? rejected.errorCode
                    ?? "Khala Sync rejected mutation \(rejected.mutationId)."
            )
        }

        return ChatSyncPushResult(
            routeRef: chatSyncPushRouteRef,
            threadId: threadId,
            messageId: messageId,
            mutationIds: mutations.compactMap { $0["mutationId"] as? Int },
            lastMutationId: response.lastMutationId,
            statuses: statuses
        )
    }

    static func resolveChatSyncOwnerUserId(
        apiKey: String,
        session: URLSession = .shared
    ) async throws -> String {
        let status = try await fetchFleetInspectorStatus(apiKey: apiKey, session: session)
        guard let owner = status.connectedIdentity?.trimmingCharacters(in: .whitespacesAndNewlines),
              !owner.isEmpty
        else {
            throw KhalaError.syncProtocol("Owner user ref was not present in fleet status.")
        }
        return owner
    }

    static func fetchChatSyncThreads(
        ownerUserId: String,
        apiKey: String,
        stateStore: KhalaChatSyncStateStore = .shared,
        session: URLSession = .shared
    ) async throws -> ChatSyncBootstrapResult {
        let trimmedKey = apiKey.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedKey.isEmpty else { throw KhalaError.missingKey }
        let owner = ownerUserId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !owner.isEmpty else {
            throw KhalaError.syncProtocol("Owner user ref is required to refetch chat sync.")
        }

        let state = stateStore.load()
        let scope = "scope.user.\(owner)"
        var pageToken: String?
        var cursor: Int?
        var entities: [ChatSyncBootstrapEntity] = []

        repeat {
            var body: [String: Any] = [
                "protocolVersion": chatSyncProtocolVersion,
                "schemaVersion": chatSyncSchemaVersion,
                "scope": scope,
                "clientGroupId": state.clientGroupId,
                "pageSize": 200,
            ]
            if let pageToken {
                body["pageToken"] = pageToken
            }
            let response = try await postChatSyncBootstrap(
                apiKey: trimmedKey,
                body: body,
                session: session
            )
            entities.append(contentsOf: response.entities)
            cursor = response.cursor
            pageToken = response.nextPageToken
        } while pageToken != nil

        let threads = try entities.compactMap { entity -> SyncedChatThread? in
            guard entity.entityType == chatThreadEntityType else { return nil }
            return try decodeSyncedChatThread(from: entity.postImageJson)
        }
        stateStore.markSyncedThreadIds(threads.map(\.threadId))
        return ChatSyncBootstrapResult(
            routeRef: chatSyncBootstrapRouteRef,
            scope: scope,
            cursor: cursor,
            threads: threads.sorted { left, right in
                if left.updatedAt == right.updatedAt {
                    return left.threadId > right.threadId
                }
                return left.updatedAt > right.updatedAt
            }
        )
    }

    private static func postChatSyncPush(
        apiKey: String,
        body: [String: Any],
        session: URLSession
    ) async throws -> ChatSyncPushResponse {
        var request = URLRequest(url: chatSyncPushURL)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        request.httpBody = try JSONSerialization.data(withJSONObject: body, options: [.sortedKeys])

        do {
            let (data, response) = try await session.data(for: request)
            guard let http = response as? HTTPURLResponse else { throw KhalaError.decoding }
            if http.statusCode == 401 || http.statusCode == 403 { throw KhalaError.unauthorized }
            if http.statusCode == 402 { throw KhalaError.quotaExceeded }
            guard (200..<300).contains(http.statusCode) else {
                throw KhalaError.http(http.statusCode, String(data: data, encoding: .utf8) ?? "")
            }
            return try JSONDecoder().decode(ChatSyncPushResponse.self, from: data)
        } catch let err as KhalaError {
            throw err
        } catch {
            throw KhalaError.transport(error)
        }
    }

    private static func postChatSyncBootstrap(
        apiKey: String,
        body: [String: Any],
        session: URLSession
    ) async throws -> ChatSyncBootstrapResponse {
        var request = URLRequest(url: chatSyncBootstrapURL)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(apiKey)", forHTTPHeaderField: "Authorization")
        request.httpBody = try JSONSerialization.data(withJSONObject: body, options: [.sortedKeys])

        do {
            let (data, response) = try await session.data(for: request)
            guard let http = response as? HTTPURLResponse else { throw KhalaError.decoding }
            if http.statusCode == 401 || http.statusCode == 403 { throw KhalaError.unauthorized }
            if http.statusCode == 402 { throw KhalaError.quotaExceeded }
            guard (200..<300).contains(http.statusCode) else {
                throw KhalaError.http(http.statusCode, String(data: data, encoding: .utf8) ?? "")
            }
            return try JSONDecoder().decode(ChatSyncBootstrapResponse.self, from: data)
        } catch let err as KhalaError {
            throw err
        } catch {
            throw KhalaError.transport(error)
        }
    }

    private static func decodeSyncedChatThread(from postImageJson: String) throws -> SyncedChatThread {
        guard let data = postImageJson.data(using: .utf8),
              let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let threadId = object["threadId"] as? String,
              let ownerUserId = object["ownerUserId"] as? String,
              let title = object["title"] as? String,
              let createdAt = object["createdAt"] as? String,
              let updatedAt = object["updatedAt"] as? String
        else {
            throw KhalaError.decoding
        }
        let messageCount = object["messageCount"] as? Int
            ?? (object["messageCount"] as? NSNumber)?.intValue
            ?? 0
        return SyncedChatThread(
            threadId: threadId,
            ownerUserId: ownerUserId,
            title: title,
            messageCount: messageCount,
            lastMessageAt: object["lastMessageAt"] as? String,
            createdAt: createdAt,
            updatedAt: updatedAt
        )
    }

    private static func sortedJsonString(_ object: [String: Any]) throws -> String {
        let data = try JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
        guard let text = String(data: data, encoding: .utf8) else {
            throw KhalaError.decoding
        }
        return text
    }

    private static func isBenignChatSyncRejection(
        _ result: ChatSyncMutationStatus,
        createMutationId: Int?
    ) -> Bool {
        guard result.status == "rejected" else { return true }
        if result.mutationId == createMutationId, result.errorCode == "thread_exists" {
            return true
        }
        if result.errorCode == "message_exists" {
            return true
        }
        return false
    }
}

struct KhalaChatSyncState: Codable, Equatable {
    var clientGroupId: String
    var clientId: String
    var lastMutationId: Int
    var syncedThreadIds: Set<String>

    static func fresh() -> KhalaChatSyncState {
        KhalaChatSyncState(
            clientGroupId: "khala_ios_cg_\(UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased())",
            clientId: "khala_ios_client_\(UUID().uuidString.replacingOccurrences(of: "-", with: "").lowercased())",
            lastMutationId: 0,
            syncedThreadIds: []
        )
    }
}

final class KhalaChatSyncStateStore {
    static let shared = KhalaChatSyncStateStore()

    private let defaults: UserDefaults
    private let key: String

    init(
        defaults: UserDefaults = .standard,
        key: String = "com.openagents.khala.chat_sync.state.v1"
    ) {
        self.defaults = defaults
        self.key = key
    }

    func load() -> KhalaChatSyncState {
        guard let data = defaults.data(forKey: key),
              let state = try? JSONDecoder().decode(KhalaChatSyncState.self, from: data),
              !state.clientGroupId.isEmpty,
              !state.clientId.isEmpty,
              state.lastMutationId >= 0
        else {
            let fresh = KhalaChatSyncState.fresh()
            save(fresh)
            return fresh
        }
        return state
    }

    func save(_ state: KhalaChatSyncState) {
        guard let data = try? JSONEncoder().encode(state) else { return }
        defaults.set(data, forKey: key)
    }

    func markSyncedThreadIds(_ threadIds: [String]) {
        guard !threadIds.isEmpty else { return }
        var state = load()
        for threadId in threadIds {
            state.syncedThreadIds.insert(threadId)
        }
        save(state)
    }

    func resetForTests() {
        defaults.removeObject(forKey: key)
    }
}

private struct ChatSyncPushResponse: Decodable {
    let protocolVersion: Int
    let results: [ChatSyncPushMutationResult]
    let lastMutationId: Int
}

private struct ChatSyncPushMutationResult: Decodable {
    let mutationId: Int
    let status: String
    let errorCode: String?
    let errorMessageSafe: String?
}

private struct ChatSyncBootstrapResponse: Decodable {
    let protocolVersion: Int
    let scope: String
    let entities: [ChatSyncBootstrapEntity]
    let cursor: Int?
    let nextPageToken: String?
}

private struct ChatSyncBootstrapEntity: Decodable {
    let entityType: String
    let entityId: String
    let postImageJson: String
}
