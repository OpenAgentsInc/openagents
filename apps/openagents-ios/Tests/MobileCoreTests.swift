// Firm reason: 6 (XCTest unit suite verifying the shared Coax C bridge, relay-free direct grants, push frames, review modes, multimodal image attachments, thought summary parsing, Workspace Git/PR wire, NIP-44 sealing, and Antigravity-only policy inside OpenAgents iOS).
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers
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

    func testDefaultRequestsPassStrictV1LegacyWireCompatibility() throws {
        let work = try XCTUnwrap(MobileCoreWork())
        let models = ["gemini-4-neon", "gemini-4-carbon", "gemini-4-hydrogen", "gemini-4-argon", "gemini-3.8-flash"]
        let efforts = ["low", "medium", "high", "xhigh"]

        for model in models {
            for effort in efforts {
                let open = try XCTUnwrap(MobileCore.encodeEnvironmentRequest(
                    work: work,
                    method: "OpenSession",
                    session: "0123456789abcdef0123456789abcdef",
                    command: "abcdef0123456789abcdef0123456789",
                    generation: 1,
                    revision: 0,
                    text: "Ship Antigravity build 1420",
                    issuedMs: 1_700_000_000_000,
                    expiresMs: 1_700_000_300_000,
                    model: model,
                    effort: effort,
                    mode: "auto"
                ))
                // Default ("auto") MUST omit "mode" and pass strict 16-key deployed-host validation.
                XCTAssertTrue(MobileCore.isV1WireCompatible(open), "Default OpenSession (\(model)/\(effort)) broke v1 wire compatibility!")
                let json = try XCTUnwrap(JSONSerialization.jsonObject(with: open) as? [String: Any])
                XCTAssertNil(json["mode"])

                let send = try XCTUnwrap(MobileCore.encodeEnvironmentRequest(
                    work: work,
                    method: "SendMessage",
                    session: "0123456789abcdef0123456789abcdef",
                    command: "abcdef0123456789abcdef0123456789",
                    generation: 1,
                    revision: 2,
                    text: "Continue",
                    issuedMs: 1_700_000_000_000,
                    expiresMs: 1_700_000_300_000,
                    model: model,
                    effort: effort,
                    mode: ""
                ))
                XCTAssertTrue(MobileCore.isV1WireCompatible(send), "Default SendMessage (\(model)/\(effort)) broke v1 wire compatibility!")
            }
        }

        // Non-default explicit "plan" emits "mode":"plan" (triggers transparent downgrade on v1 hosts).
        let plan = try XCTUnwrap(MobileCore.encodeEnvironmentRequest(
            work: work,
            method: "OpenSession",
            session: "0123456789abcdef0123456789abcdef",
            command: "abcdef0123456789abcdef0123456789",
            generation: 1,
            revision: 0,
            text: "Plan carefully",
            issuedMs: 1_700_000_000_000,
            expiresMs: 1_700_000_300_000,
            model: "gemini-4-neon",
            effort: "high",
            mode: "plan"
        ))
        XCTAssertFalse(MobileCore.isV1WireCompatible(plan))
    }

    func testHostErrorFormatterHumanizesEveryWireRefusalCode() {
        let rawCodes = [
            "unsupported", "forbidden", "expired", "revoked", "conflict",
            "no_agent", "model_unavailable", "capacity", "not_found",
            "unsupported_profile", "folder_refused", "workspace_refused",
            "storage_unavailable", "source_unavailable", "interaction_expired",
            "malformed", "rate_limited", "too_large", "bounds", "output_limit",
            "idempotency_conflict", "unavailable",
        ]
        for code in rawCodes {
            let msg = HostErrorFormatter.describe(code, method: "OpenSession")
            XCTAssertFalse(msg.isEmpty)
            XCTAssertNotEqual(msg, code, "Wire refusal code '\(code)' must never leak raw to users!")
            XCTAssertGreaterThan(msg.count, 20, "Error copy for '\(code)' should be plain, actionable English.")
        }
        let wsForbidden = HostErrorFormatter.describe("forbidden", method: "ReadWorkspace")
        XCTAssertTrue(wsForbidden.contains("workspace-enable"))
    }

    func testWorkspaceChangedFilesSplitsUnifiedDiffPerFile() {
        let snap = WorkspaceSnapshot(
            host: String(repeating: "a", count: 64),
            workspace: "default",
            revision: String(repeating: "1", count: 64),
            path: "/Users/dev/openagents",
            repository: "OpenAgentsInc/openagents",
            branch: "main",
            status: " M apps/openagents-ios/Sources/ChatView.swift\n?? README-new.md",
            diff: """
            diff --git a/apps/openagents-ios/Sources/ChatView.swift b/apps/openagents-ios/Sources/ChatView.swift
            @@ -1,2 +1,3 @@
            -old
            +new
            +extra
            """,
            diffCut: false,
            error: "",
            githubError: "",
            worktrees: [],
            branches: ["main", "feat/antigravity"],
            pullRequests: []
        )
        let files = snap.changedFiles
        XCTAssertEqual(files.count, 2)
        XCTAssertEqual(files[0].path, "apps/openagents-ios/Sources/ChatView.swift")
        XCTAssertEqual(files[0].code, "M")
        XCTAssertTrue(files[0].diff.contains("+extra"))
        XCTAssertEqual(files[1].path, "README-new.md")
        XCTAssertEqual(files[1].code, "??")
        XCTAssertTrue(files[1].diff.isEmpty)
    }

    func testFuzz120RandomizedPromptsAndFoldersThroughCoaxAndNIP44() throws {
        var sk = Data(repeating: 0, count: 32)
        sk[31] = 3
        let host = "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"
        let grant = String(repeating: "9", count: 64)
        let work = try XCTUnwrap(MobileCoreWork())

        // Deterministic xorshift64* PRNG so fuzz failures reproduce identically.
        var rng: UInt64 = 0xD1B54A32D192ED03
        func nextU64() -> UInt64 {
            rng ^= rng >> 12
            rng ^= rng << 25
            rng ^= rng >> 27
            return rng &* 2685821657736338717
        }

        let alphabet = Array("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 !@#%^&*()_+-=[]{}|;':\",.<>?/`~\\\n\t😀🚀")
        let models = ["gemini-4-neon", "gemini-4-carbon", "gemini-3.8-flash"]
        let efforts = ["low", "medium", "high", "xhigh"]

        for i in 0..<120 {
            let len = Int(nextU64() % 240) + 1
            var prompt = ""
            prompt.reserveCapacity(len)
            for _ in 0..<len {
                prompt.append(alphabet[Int(nextU64() % UInt64(alphabet.count))])
            }
            let model = models[i % models.count]
            let effort = MobileCore.normalizeAntigravityEffort(model: model, want: efforts[i % efforts.count])
            let req = try XCTUnwrap(MobileCore.encodeEnvironmentRequest(
                work: work,
                method: i % 2 == 0 ? "OpenSession" : "SendMessage",
                session: "0123456789abcdef0123456789abcdef",
                command: String(format: "%032x", i + 1),
                generation: 1,
                revision: Int64(i),
                text: prompt,
                issuedMs: 1_700_000_000_000,
                expiresMs: 1_700_000_300_000,
                model: model,
                effort: effort,
                mode: "auto"
            ), "Fuzz prompt #\(i) failed Coax encoding")
            XCTAssertTrue(MobileCore.isV1WireCompatible(req), "Fuzz prompt #\(i) violated v1 wire compatibility")
            let sealed = try XCTUnwrap(MobileCore.sealRPC(
                work: work,
                secret: sk,
                host: host,
                relay: MobileCore.directRelayLabel,
                grant: grant,
                epoch: 1,
                params: req,
                nowSeconds: 1_790_000_000
            ), "Fuzz prompt #\(i) failed NIP-44 seal")
            XCTAssertEqual(sealed.eventID.count, 64)
        }
    }

    private func makeTestPNG(width: Int = 256, height: Int = 256) throws -> Data {
        let cs = CGColorSpaceCreateDeviceRGB()
        let ctx = try XCTUnwrap(CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: cs,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ))
        ctx.setFillColor(CGColor(red: 0.9, green: 0.1, blue: 0.1, alpha: 1.0))
        ctx.fill(CGRect(x: 0, y: 0, width: width, height: height))
        let cg = try XCTUnwrap(ctx.makeImage())
        let out = NSMutableData()
        let dest = try XCTUnwrap(CGImageDestinationCreateWithData(out, UTType.png.identifier as CFString, 1, nil))
        CGImageDestinationAddImage(dest, cg, nil)
        XCTAssertTrue(CGImageDestinationFinalize(dest))
        return out as Data
    }

    func testMultimodalImageAttachmentCompressionRoundTripAndV1WireCompat() throws {
        let png = try makeTestPNG(width: 320, height: 240)
        let att = FileAttachment.prepareImage(data: png, name: "photo 3641.jpg")
        XCTAssertTrue(att.isImage)
        let b64 = try XCTUnwrap(att.imageBase64)
        let wireBytes = try XCTUnwrap(Data(base64Encoded: b64))
        XCTAssertLessThanOrEqual(wireBytes.count, 3_400, "Compressed wire image must fit cleanly inside one 6000-byte es_project_at item chunk")
        XCTAssertNotNil(att.imageData, "High-res local preview thumbnail should also be populated")
        let block = att.renderedPromptBlock()
        XCTAssertTrue(block.contains("[oa-image "))
        XCTAssertTrue(block.contains("mime=image/jpeg"))
        XCTAssertTrue(block.contains("base64="))

        let fullPrompt = "Tell me what you see here." + block
        let parsed = ParsedUserMessage.parse(fullPrompt)
        XCTAssertEqual(parsed.text, "Tell me what you see here.")
        XCTAssertEqual(parsed.images.count, 1)
        XCTAssertEqual(parsed.images[0].name, "photo_3641.jpg")
        XCTAssertEqual(parsed.images[0].data, wireBytes)

        // Must ALSO pass strict 16-key v1 wire compatibility!
        let work = try XCTUnwrap(MobileCoreWork())
        let req = try XCTUnwrap(MobileCore.encodeEnvironmentRequest(
            work: work,
            method: "OpenSession",
            session: "0123456789abcdef0123456789abcdef",
            command: "abcdef0123456789abcdef0123456789",
            generation: 1,
            revision: 0,
            text: fullPrompt,
            issuedMs: 1_700_000_000_000,
            expiresMs: 1_700_000_300_000,
            model: "gemini-4-neon",
            effort: "medium",
            mode: "auto"
        ))
        XCTAssertTrue(MobileCore.isV1WireCompatible(req))
    }

    func testThoughtSummaryParserExtractsDesktopHeadingWithoutRawAsterisks() {
        let raw = "**Confirming System Functionality**\n\nI'm verifying the wire."
        let heading = ThoughtSummaryParser.extractHeading(raw)
        XCTAssertEqual(heading, "Confirming System Functionality")
        XCTAssertFalse(heading.contains("*"))

        let plain = "Checking the repository layout."
        XCTAssertEqual(ThoughtSummaryParser.extractHeading(plain), "Checking the repository layout.")
        XCTAssertEqual(ThoughtSummaryParser.extractHeading("   \n  "), "")
    }

    @MainActor
    func testIncomingOpenAgentsURLDeepLinkPopulatesComposer() throws {
        let store = AntigravityStore()
        store.draftText = ""
        let url = try XCTUnwrap(URL(string: "openagents://ask?prompt=Check%20Tailscale%20status"))
        store.handleIncomingURL(url)
        XCTAssertEqual(store.draftText, "Check Tailscale status")
    }
}
