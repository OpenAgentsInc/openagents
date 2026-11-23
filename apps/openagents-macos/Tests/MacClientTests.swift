// Firm reason: 6 (macOS XCTest unit suite verifying the shared Coax C core, Tailscale admission/direct-grant/push codecs, Workspace Git wire, and Antigravity-only policy inside OpenAgents macOS).
import XCTest
@testable import OpenAgentsMac

final class MacClientTests: XCTestCase {
    func testSharedCoaxAntigravityPolicyAndReviewModes() throws {
        XCTAssertTrue(MobileCore.isAntigravityModel("gemini-4-neon"))
        XCTAssertTrue(MobileCore.isAntigravityModel("gemini-3.8-flash"))
        XCTAssertFalse(MobileCore.isAntigravityModel("claude-opus-5-5"))
        XCTAssertFalse(MobileCore.isAntigravityModel("inkling-2-mini"))
        XCTAssertEqual(MobileCore.normalizeAntigravityModel("claude-opus-5-5"), "gemini-4-neon")
        XCTAssertTrue(MobileCore.isValidReviewMode("auto"))
        XCTAssertTrue(MobileCore.isValidReviewMode("default"))
        XCTAssertTrue(MobileCore.isValidReviewMode("plan"))
        XCTAssertFalse(MobileCore.isValidReviewMode("yolo"))
    }

    func testSharedCoaxEnvironmentAndWorkspaceWireOnMacOS() throws {
        let work = try XCTUnwrap(MobileCoreWork())
        let req = try XCTUnwrap(MobileCore.encodeEnvironmentRequest(
            work: work,
            method: "OpenSession",
            session: "0123456789abcdef0123456789abcdef",
            command: "abcdef0123456789abcdef0123456789",
            generation: 1,
            revision: 0,
            text: "Audit diff on Mac",
            issuedMs: 1_700_000_000_000,
            expiresMs: 1_700_000_300_000,
            model: "gemini-4-neon",
            effort: "xhigh",
            mode: "plan"
        ))
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: req) as? [String: Any])
        XCTAssertEqual(json["mode"] as? String, "plan")
        XCTAssertEqual(json["model"] as? String, "gemini-4-neon")

        // Native ~/.gemini Antigravity list sentinel preserved
        let agyList = try XCTUnwrap(MobileCore.encodeEnvironmentRequest(work: work, method: "ListSessions", model: "antigravity"))
        let agyJSON = try XCTUnwrap(JSONSerialization.jsonObject(with: agyList) as? [String: Any])
        XCTAssertEqual(agyJSON["model"] as? String, "antigravity")

        // Workspace Git RPC
        let wsRead = try XCTUnwrap(MobileCore.encodeWorkspaceRequest(work: work, method: "ReadWorkspace"))
        let wsJSON = try XCTUnwrap(JSONSerialization.jsonObject(with: wsRead) as? [String: Any])
        XCTAssertEqual(wsJSON["method"] as? String, "ReadWorkspace")
    }

    func testTailscaleAdmissionDirectGrantAndPushCodecsOnMacOS() throws {
        // Owner admission rules
        let owner = AppTailnet.Owner(user: 42, login: "user@example.com", ip: "100.64.0.10", dns: "mac.tail.ts.net", name: "mac")
        let goodWhois = """
        {"Node":{"Addresses":["100.64.0.22/32"],"User":42},"UserProfile":{"ID":42,"LoginName":"user@example.com"}}
        """.data(using: .utf8)
        XCTAssertNil(AppTailnet.decide(remote: "100.64.0.22", whois: goodWhois, owner: owner))
        XCTAssertEqual(AppTailnet.decide(remote: "192.168.1.5", whois: goodWhois, owner: owner), .notTailnet)

        let taggedWhois = """
        {"Node":{"Addresses":["100.64.0.22/32"],"User":42,"Tags":["tag:ci"]},"UserProfile":{"ID":42,"LoginName":"user@example.com"}}
        """.data(using: .utf8)
        XCTAssertEqual(AppTailnet.decide(remote: "100.64.0.22", whois: taggedWhois, owner: owner), .tagged)

        // Direct grant codec
        let dev = String(repeating: "1", count: 64)
        let host = String(repeating: "2", count: 64)
        let grant = String(repeating: "3", count: 64)
        let reqLine = AppTailnet.directGrantRequestLine(device: dev, label: "OpenAgents macOS")
        let parsedReq = try XCTUnwrap(AppTailnet.parseDirectGrantRequest(Data(reqLine.dropLast())))
        XCTAssertEqual(parsedReq.device, dev)

        let ans = AppTailnet.DirectGrantAnswer(host: host, label: "Mac Studio", grant: grant, epoch: 1, expires: 1_800_000_000, refused: nil)
        let parsedAns = try XCTUnwrap(AppTailnet.directGrantAnswer(Data(AppTailnet.directGrantAnswerLine(ans).dropLast())))
        XCTAssertEqual(parsedAns, ans)

        // Push frame codec
        let push = AppTailnet.PushEvent(seq: 4, kind: "sessions_changed", session: "", revision: 0, busy: false, hasInteraction: false)
        let parsedPush = try XCTUnwrap(AppTailnet.parsePush(Data(AppTailnet.pushLine(push).dropLast())))
        XCTAssertEqual(parsedPush, push)
    }

    func testV1LegacyWireCompatAndErrorHumanizationOnMacOS() throws {
        let work = try XCTUnwrap(MobileCoreWork())
        let autoReq = try XCTUnwrap(MobileCore.encodeEnvironmentRequest(
            work: work,
            method: "OpenSession",
            session: "0123456789abcdef0123456789abcdef",
            command: "abcdef0123456789abcdef0123456789",
            generation: 1,
            revision: 0,
            text: "Test macOS legacy wire compat",
            issuedMs: 1_700_000_000_000,
            expiresMs: 1_700_000_300_000,
            model: "gemini-4-neon",
            effort: "medium",
            mode: "auto"
        ))
        XCTAssertTrue(MobileCore.isV1WireCompatible(autoReq))
        XCTAssertNotEqual(HostErrorFormatter.describe("unsupported"), "unsupported")
        XCTAssertNotEqual(HostErrorFormatter.describe("no_agent"), "no_agent")
    }
}
