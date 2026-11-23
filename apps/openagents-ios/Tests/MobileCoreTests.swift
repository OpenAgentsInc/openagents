// Firm reason: 6 (XCTest unit suite verifying the compiled Coax C bridge, NIP-44 sealing, Tailscale codecs, and Antigravity-only policy inside the iOS app).
import XCTest
@testable import OpenAgents

final class MobileCoreTests: XCTestCase {
    func testAntigravityOnlyModelAndEffortPolicy() {
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
    }

    func testCoaxRequestEncoderEnforcesAntigravityModelsAtWireBoundary() throws {
        let work = try XCTUnwrap(MobileCoreWork())

        let valid = MobileCore.encodeEnvironmentRequest(
            work: work,
            method: "OpenSession",
            command: "0123456789abcdef0123456789abcdef",
            generation: 1,
            revision: 0,
            text: "Inspect Tailscale handshake",
            issuedMs: 1_700_000_000_000,
            expiresMs: 1_700_000_300_000,
            model: "gemini-4-neon",
            effort: "high",
            folder: "apps/openagents-ios"
        )
        let validData = try XCTUnwrap(valid)
        let json = try XCTUnwrap(JSONSerialization.jsonObject(with: validData) as? [String: Any])
        XCTAssertEqual(json["method"] as? String, "OpenSession")
        XCTAssertEqual(json["model"] as? String, "gemini-4-neon")
        XCTAssertEqual(json["effort"] as? String, "high")
        XCTAssertEqual(json["folder"] as? String, "apps/openagents-ios")

        // Non-Antigravity models must be rejected by Coax before sealing.
        XCTAssertNil(MobileCore.encodeEnvironmentRequest(
            work: work,
            method: "OpenSession",
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
            command: "0123456789abcdef0123456789abcdef",
            generation: 1,
            revision: 0,
            text: "hi",
            issuedMs: 1_700_000_000_000,
            expiresMs: 1_700_000_300_000,
            model: "inkling-2-mini",
            effort: "medium"
        ))

        // Path traversal in folder is rejected by Coax.
        XCTAssertNil(MobileCore.encodeEnvironmentRequest(
            work: work,
            method: "OpenSession",
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

    func testNIP44PublicKeyAndSealRPC() throws {
        // Known secp256k1 private key = 1 -> generator G x-coordinate
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
            relay: "tailscale://direct",
            grant: grant,
            epoch: 0,
            params: params,
            nowSeconds: 1_790_000_000
        ))
        XCTAssertEqual(sealed.eventID.count, 64)
        XCTAssertTrue(sealed.event.count > 100)
    }

    func testTailscaleDirectCodecRoundTrip() {
        let hello = AppTailnet.directHelloLine()
        XCTAssertFalse(hello.isEmpty)
        let helloLine = Data(hello.dropLast()) // strip \n
        XCTAssertTrue(AppTailnet.isDirectHello(helloLine))

        let okAck = AppTailnet.directAckLine(refused: nil)
        let okParsed = AppTailnet.directAck(Data(okAck.dropLast()))
        XCTAssertTrue(okParsed.ok)
        XCTAssertNil(okParsed.refused)

        let refusedAck = AppTailnet.directAckLine(refused: "tagged")
        let refusedParsed = AppTailnet.directAck(Data(refusedAck.dropLast()))
        XCTAssertFalse(refusedParsed.ok)
        XCTAssertEqual(refusedParsed.refused, "tagged")

        XCTAssertTrue(AppTailnet.isTailnet("100.101.102.103"))
        XCTAssertFalse(AppTailnet.isTailnet("192.168.1.5"))
        XCTAssertEqual(AppTailnet.dnsName("MacBook.Tail123.ts.net."), "macbook.tail123.ts.net")
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
          "folders": ["apps/openagents-ios", "../escape"],
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
        let data = Data(rawJSON.utf8)
        let work = try XCTUnwrap(MobileCoreWork())
        // Note: `../escape` causes Coax `ea_view_read` to reject malformed folders; verify strictness too.
        XCTAssertFalse(MobileCore.validateView(work: work, host: host, method: "DescribeEnvironment", raw: data))

        let cleanJSON = rawJSON.replacingOccurrences(of: ", \"../escape\"", with: "")
        let cleanData = Data(cleanJSON.utf8)
        XCTAssertTrue(MobileCore.validateView(work: work, host: host, method: "DescribeEnvironment", raw: cleanData))

        let parsed = try XCTUnwrap(ParsedEnvironmentView.parse(cleanData))
        XCTAssertEqual(parsed.antigravityModels.map(\.id), ["gemini-4-neon", "gemini-3.8-flash"])
        XCTAssertEqual(parsed.folders, ["apps/openagents-ios"])
    }
}
