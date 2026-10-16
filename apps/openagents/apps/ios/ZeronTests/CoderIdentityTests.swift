import XCTest
import UIKit
@testable import Zeron

final class CoderIdentityTests: XCTestCase {
    func testAppIdentityAndServer() {
        XCTAssertEqual(Bundle.main.bundleIdentifier, "com.openagents.app")
        XCTAssertEqual(Bundle.main.object(forInfoDictionaryKey: "CFBundleDisplayName") as? String, "OpenAgents")
        XCTAssertEqual(productionEdgeUrl(), "https://sync.openagents.com")
        let types = Bundle.main.object(forInfoDictionaryKey: "CFBundleURLTypes") as? [[String: Any]]
        XCTAssertEqual(types?.first?["CFBundleURLSchemes"] as? [String], ["openagents"])
    }

    @MainActor
    func testSavedLegacySceneGetsANewUIKitConfiguration() async throws {
        for _ in 0..<100 where UIApplication.shared.connectedScenes.isEmpty {
            try await Task.sleep(nanoseconds: 50_000_000)
        }
        let session = try XCTUnwrap(UIApplication.shared.openSessions.first { $0.role == .windowApplication })
        let legacy = UISceneConfiguration(name: "Default", sessionRole: session.role)
        legacy.delegateClass = LegacyWindowSceneDelegate.self
        let replacement = LegacySceneArchive(configuration: legacy)
        let archiver = NSKeyedArchiver(requiringSecureCoding: true)
        archiver.delegate = replacement
        archiver.encode(session, forKey: NSKeyedArchiveRootObjectKey)
        archiver.finishEncoding()
        let restored = try XCTUnwrap(NSKeyedUnarchiver.unarchivedObject(ofClass: UISceneSession.self, from: archiver.encodedData))
        XCTAssertTrue(restored.configuration.delegateClass === LegacyWindowSceneDelegate.self)

        let configuration = AppDelegate.windowConfiguration(for: restored)
        XCTAssertTrue(configuration.delegateClass === SceneDelegate.self)
        XCTAssertTrue(configuration.sceneClass === UIWindowScene.self)
        XCTAssertNil(configuration.storyboard)
        XCTAssertTrue(NSClassFromString("OpenAgentsCoderSceneDelegate") === SceneDelegate.self)
        XCTAssertTrue(UIApplication.shared.connectedScenes.first?.delegate is SceneDelegate)
    }

    func testAttributionIsBundled() throws {
        let url = try XCTUnwrap(Bundle.main.url(forResource: "Zeron-MIT", withExtension: "txt"))
        let notice = try String(contentsOf: url, encoding: .utf8)
        XCTAssertTrue(notice.contains("MIT License"))
        XCTAssertTrue(notice.contains("Wing"))
    }
}

private final class LegacyWindowSceneDelegate: UIResponder, UIWindowSceneDelegate {}

private final class LegacySceneArchive: NSObject, NSKeyedArchiverDelegate {
    private let configuration: UISceneConfiguration

    init(configuration: UISceneConfiguration) {
        self.configuration = configuration
    }

    func archiver(_ archiver: NSKeyedArchiver, willEncode object: Any) -> Any? {
        object is UISceneConfiguration ? configuration : object
    }
}
