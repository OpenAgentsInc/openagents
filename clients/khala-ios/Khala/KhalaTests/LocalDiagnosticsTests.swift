import Foundation
@testable import Khala
import XCTest

final class LocalDiagnosticsTests: XCTestCase {
    func testDefaultSnapshotDoesNotRenderSecretsOrLocalPaths() {
        let snapshot = LocalDiagnosticsSnapshot.make(
            generatedAt: Date(timeIntervalSince1970: 0),
            hasAPIKey: true,
            channelName: "Artanis",
            isStreaming: false,
            activeConversation: nil,
            conversationCount: 2,
            isUsingEphemeralFallback: false
        )

        let publicText = snapshot.publicRows.map(\.value).joined(separator: "\n")
        XCTAssertTrue(publicText.contains("Public-safe summary only"))
        XCTAssertFalse(publicText.contains("oa_agent_"))
        XCTAssertFalse(publicText.contains("/Users/"))
        XCTAssertFalse(publicText.localizedCaseInsensitiveContains("prompt"))
        XCTAssertFalse(publicText.localizedCaseInsensitiveContains("private key"))
    }

    func testRawDiagnosticsAreRedactedBeforeRendering() {
        let raw = """
        Authorization: Bearer oa_agent_secret_token
        api_key=oa_agent_second_secret
        path=/Users/operator/work/private-repo
        wallet=spark1abc123
        ~/.codex/auth.json
        """

        let redacted = LocalDiagnosticsSnapshot.redactSensitiveText(raw)

        XCTAssertFalse(redacted.contains("oa_agent_secret_token"))
        XCTAssertFalse(redacted.contains("oa_agent_second_secret"))
        XCTAssertFalse(redacted.contains("/Users/operator"))
        XCTAssertFalse(redacted.contains("spark1abc123"))
        XCTAssertFalse(redacted.contains("~/.codex"))
        XCTAssertTrue(redacted.contains("<redacted>"))
        XCTAssertTrue(redacted.contains("<local-path>"))
    }
}
