import XCTest
@testable import Khala

@MainActor
final class ConversationStoreTests: XCTestCase {
    func testCreatesConversationAndPersistsMessages() throws {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString).appendingPathExtension("json")
        defer { try? FileManager.default.removeItem(at: url) }
        let store = ConversationStore(storeURL: url)
        let conversation = try XCTUnwrap(store.selectedConversation)
        store.appendMessage(.user, content: "Hello Khala", to: conversation.id)
        store.appendMessage(.assistant, content: "Hello.", to: conversation.id)
        let reloaded = ConversationStore(storeURL: url)
        let persisted = try XCTUnwrap(reloaded.selectedConversation)
        XCTAssertEqual(persisted.title, "Hello Khala")
        XCTAssertEqual(persisted.messages.map(\.role), [.user, .assistant])
    }

    func testDerivedTitleTrimsLongFirstLine() {
        let title = Conversation.derivedTitle(from: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ")
        XCTAssertEqual(title, "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUV...")
    }
}
