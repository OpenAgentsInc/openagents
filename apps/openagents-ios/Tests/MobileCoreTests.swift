// Firm reason: 6 (XCTest unit suite verifying the shared Coax C bridge, relay-free direct grants, push frames, review modes, Workspace Git/PR wire, NIP-44 sealing, and Antigravity-only policy inside OpenAgents iOS).
import XCTest
@testable import OpenAgents

final class MobileCoreTests: XCTestCase {
    func testAntigravityOnlyModelEffortAndReviewModePolicy() {
        XCTAssertTrue(MobileCore.isAntigravityModel("gemini-4-neon"))
        XCTAssertTrue(MobileCore.isAntigravityModel("gemini-4-carbon"))
        XCTAssertTrue(MobileCore.isAntigravityModel("gemini-3.8-flash"))
        XCTAssertTrue(MobileCore.isAntigravityModel("antigravity"))

        XCTAssertFalse(MobileCore.isAntigravityModel("inkling-2-mini"))
        XCTAssertFalse(MobileCore.isAntigravityModel("inkling"))
        XCTAssertFalse(MobileCore.isAntigravityModel("claude-opus-5-5"))
        XCTAssertFalse(MobileCore.isAntigravityModel("default"))
        XCTAssertFalse(MobileCore.isAntigravityModel(""))

        XCTAssertEqual(MobileCore.normalizeAntigravityModel("gemini-4-hydrogen"), "gemini-4-hydrogen")
        XCTAssertEqual(MobileCore.normalizeAntigravityModel("antigravity"), "gemini-4-neon")
        XCTAssertEqual(MobileCore.normalizeAntigravityModel("claude-opus-5-5"), "gemini-4-neon")
        XCTAssertEqual(MobileCore.normalizeAntigravityModel("inkling-2-mini"), "gemini-4-neon")

        XCTAssertEqual(MobileCore.normalizeAntigravityEffort(model: "gemini-4-neon", want: "xhigh"), "xhigh")
        XCTAssertEqual(MobileCore.normalizeAntigravityEffort(model: "gemini-4-neon", want: "low"), "low")
        XCTAssertEqual(MobileCore.normalizeAntigravityEffort(model: "gemini-4-neon", want: "bogus"), "medium")

        XCTAssertTrue(MobileCore.isValidReviewMode("auto"))
        XCTAssertTrue(MobileCore.isValidReviewMode("default"))
        XCTAssertTrue(MobileCore.isValidReviewMode("plan"))
        XCTAssertFalse(MobileCore.isValidReviewMode("unsafe"))
        XCTAssertEqual(ReviewMode.ask.wireValue, "default")
    }

    func testCoaxRequestEncoderEnforcesAntigravityModelsReviewModeAndNativeAgySentinel() throws {
        let work = try XCTUnwrap(MobileCoreWork())

        let valid = MobileCore.encodeEnvironmentRequest(
            work: work,
            method: "OpenSession",
            session: "abcdef0123456789abcdef0123456789",
            command: "0123456789abcdef0123456789abcdef",
            generation: 1,
            revision: 0,
            text: "Inspect Tailscale handshake",
            issuedMs: 1_700_000_000_000,
            expiresMs: 1_700_000_300_000,
            model: "gemini-4-neon",
            effort: "high",
            folder: "apps/openagents-ios",
            mode: "plan"
        )
        let validData = try XCTUnwrap(valid)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: validData) as? [String: Any])
        XCTAssertEqual(json["method"] as? String, "OpenSession")
        XCTAssertEqual(json["session_id"] as? String, "abcdef0123456789abcdef0123456789")
        XCTAssertEqual(json["model"] as? String, "gemini-4-neon")
        XCTAssertEqual(json["effort"] as? String, "high")
        XCTAssertEqual(json["folder"] as? String, "apps/openagents-ios")
        XCTAssertEqual(json["mode"] as? String, "plan")

        // ListSessions with "antigravity" preserves the sentinel for native ~/.gemini chats
        let nativeList = try XCTUnwrap(MobileCore.encodeEnvironmentRequest(
            work: work,
            method: "ListSessions",
            model: "antigravity"
        ))
        let nativeJSON = try XCTUnwrap(JSONSerialization.jsonObject(with: nativeList) as? [String: Any])
        XCTAssertEqual(nativeJSON["model"] as? String, "antigravity")

        // Non-Antigravity models and invalid review modes must be rejected by Coax before sealing.
        XCTAssertNil(MobileCore.encodeEnvironmentRequest(
            work: work,
            method: "OpenSession",
            session: "abcdef0123456789abcdef0123456789",
            command: "0123456789abcdef0123456789abcdef",
            generation: 1,
            revision: 0,
            text: "hi",
            issuedMs: 1_700_000_000_000,
            expiresMs: 1_700_000_300_000,
            model: "claude-opus-5-5",
            effort: "high"
        ))
        XCTAssertNil(MobileCore.encodeEnvironmentRequest(
            work: work,
            method: "OpenSession",
            session: "abcdef0123456789abcdef0123456789",
            command: "0123456789abcdef0123456789abcdef",
            generation: 1,
            revision: 0,
            text: "hi",
            issuedMs: 1_700_000_000_000,
            expiresMs: 1_700_000_300_000,
            model: "gemini-4-neon",
            effort: "high",
            mode: "bogus"
        ))
        // Path traversal in folder is rejected by Coax.
        XCTAssertNil(MobileCore.encodeEnvironmentRequest(
            work: work,
            method: "OpenSession",
            session: "abcdef0123456789abcdef0123456789",
            command: "0123456789abcdef0123456789abcdef",
            generation: 1,
            revision: 0,
            text: "hi",
            issuedMs: 1_700_000_000_000,
            expiresMs: 1_700_000_300_000,
            model: "gemini-4-neon",
            folder: "../secrets"
        ))
    }

    func testWorkspaceGitAndPullRequestWireEncoding() throws {
        let work = try XCTUnwrap(MobileCoreWork())
        let readData = try XCTUnwrap(MobileCore.encodeWorkspaceRequest(work: work, method: "ReadWorkspace"))
        let readJSON = try XCTUnwrap(JSONSerialization.jsonObject(with: readData) as? [String: Any])
        XCTAssertEqual(readJSON["method"] as? String, "ReadWorkspace")

        let rev = String(repeating: "a", count: 64)
        let sw = try XCTUnwrap(MobileCore.encodeWorkspaceRequest(
            work: work,
            method: "SwitchBranch",
            branch: "feat/antigravity",
            expectedRevision: rev,
            command: "0123456789abcdef0123456789abcdef",
            issuedMs: 1_700_000_000_000,
            expiresMs: 1_700_000_060_000
        ))
        let swJSON = try XCTUnwrap(JSONSerialization.jsonObject(with: sw) as? [String: Any])
        XCTAssertEqual(swJSON["branch"] as? String, "feat/antigravity")

        // Unsafe git option-like ref name rejected by Coax WsRequest
        XCTAssertNil(MobileCore.encodeWorkspaceRequest(
            work: work,
            method: "SwitchBranch",
            branch: "-rf",
            expectedRevision: rev,
            command: "0123456789abcdef0123456789abcdef",
            issuedMs: 1_700_000_000_000,
            expiresMs: 1_700_000_060_000
        ))
    }

    func testNIP44PublicKeyAndSealRPC() throws {
        var sk = Data(repeating: 0, count: 32)
        sk[31] = 1
        let pub = try XCTUnwrap(MobileCore.publicKeyHex(secret: sk))
        XCTAssertEqual(pub, "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
        XCTAssertTrue(MobileCore.isValidHostKey(pub))
        XCTAssertFalse(MobileCore.isValidHostKey(pub.uppercased()))

        let work = try XCTUnwrap(MobileCoreWork())
        let params = try XCTUnwrap(MobileCore.encodeEnvironmentRequest(work: work, method: "ListSessions"))
        let host = "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"
        let grant = String(repeating: "2", count: 64)

        let sealed = try XCTUnwrap(MobileCore.sealRPC(
            work: work,
            secret: sk,
            host: host,
            relay: MobileCore.directRelayLabel,
            grant: grant,
            epoch: 0,
            params: params,
            nowSeconds: 1_790_000_000
        ))
        XCTAssertEqual(sealed.eventID.count, 64)
        XCTAssertTrue(sealed.event.count > 100)
    }

    func testTailscaleDirectGrantAndPushFrameCodecs() throws {
        // Direct transport handshake
        let hello = AppTailnet.directHelloLine()
        XCTAssertTrue(AppTailnet.isDirectHello(Data(hello.dropLast())))
        XCTAssertTrue(AppTailnet.directAck(Data(AppTailnet.directAckLine(refused: nil).dropLast())).ok)
        XCTAssertEqual(AppTailnet.directAck(Data(AppTailnet.directAckLine(refused: "tagged").dropLast())).refused, "tagged")

        // Relay-free direct grant request & answer
        let dev = String(repeating: "b", count: 64)
        let host = String(repeating: "a", count: 64)
        let grant = String(repeating: "c", count: 64)
        let reqLine = AppTailnet.directGrantRequestLine(device: dev, label: "OpenAgents iOS")
        let parsedReq = try XCTUnwrap(AppTailnet.parseDirectGrantRequest(Data(reqLine.dropLast())))
        XCTAssertEqual(parsedReq.device, dev)
        XCTAssertEqual(parsedReq.label, "OpenAgents iOS")

        let ans = AppTailnet.DirectGrantAnswer(host: host, label: "Mac Studio", grant: grant, epoch: 1, expires: 1_800_000_000, refused: nil)
        let ansLine = AppTailnet.directGrantAnswerLine(ans)
        let parsedAns = try XCTUnwrap(AppTailnet.directGrantAnswer(Data(ansLine.dropLast())))
        XCTAssertEqual(parsedAns, ans)

        // Real-time push subscribe & change notification frames
        let subLine = AppTailnet.subscribeLine(session: "0123456789abcdef0123456789abcdef")
        XCTAssertEqual(AppTailnet.parseSubscribe(Data(subLine.dropLast())), "0123456789abcdef0123456789abcdef")

        let push = AppTailnet.PushEvent(seq: 9, kind: "session_updated", session: "0123456789abcdef0123456789abcdef", revision: 12, busy: true, hasInteraction: true)
        let pushLine = AppTailnet.pushLine(push)
        let parsedPush = try XCTUnwrap(AppTailnet.parsePush(Data(pushLine.dropLast())))
        XCTAssertEqual(parsedPush, push)

        // Unknown push kind must be refused by Coax schema enforcement
        let badPush = AppTailnet.PushEvent(seq: 1, kind: "bogus", session: "", revision: 0, busy: false, hasInteraction: false)
        XCTAssertTrue(AppTailnet.pushLine(badPush).isEmpty)
    }

    func testParsedEnvironmentViewFiltersOutNonAntigravityModels() throws {
        let host = String(repeating: "a", count: 64)
        let rawJSON = """
        {
          "schema": "openagents.environment.runtime.v1alpha1",
          "host_pubkey": "\(host)",
          "display_name": "Studio Mac",
          "method": "DescribeEnvironment",
          "sessions": [],
          "items": [],
          "error": "",
          "complete": true,
          "next_cursor": "",
          "command_id": "",
          "command_status": "",
          "source_epoch": "",
          "workspace_name": "openagents",
          "folders": ["apps/openagents-ios"],
          "efforts": [
            {"effort_id": "low", "display_name": "Low"},
            {"effort_id": "medium", "display_name": "Medium"},
            {"effort_id": "high", "display_name": "High"},
            {"effort_id": "xhigh", "display_name": "X-High"}
          ],
          "default_effort": "medium",
          "models": [
            {"model_id": "gemini-4-neon", "display_name": "Gemini 4 Neon", "efforts": ["low", "medium", "high", "xhigh"]},
            {"model_id": "inkling-2-mini", "display_name": "Inkling 2 Mini", "efforts": ["low", "medium", "high"]},
            {"model_id": "claude-opus-5-5", "display_name": "Claude Opus 5.5", "efforts": ["high"]},
            {"model_id": "gemini-3.8-flash", "display_name": "Gemini 3.8 Flash", "efforts": ["low", "medium", "high"]}
          ]
        }
        """
        let cleanData = Data(rawJSON.utf8)
        let work = try XCTUnwrap(MobileCoreWork())
        XCTAssertTrue(MobileCore.validateView(work: work, host: host, method: "DescribeEnvironment", raw: cleanData))

        let parsed = try XCTUnwrap(ParsedEnvironmentView.parse(cleanData))
        XCTAssertEqual(parsed.antigravityModels.map(\.id), ["gemini-4-neon", "gemini-3.8-flash"])
        XCTAssertEqual(parsed.folders, ["apps/openagents-ios"])
    }
}
