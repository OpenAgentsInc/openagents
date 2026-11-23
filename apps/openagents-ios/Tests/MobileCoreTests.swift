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
        XCTAssertTrue(MobileCore.isAntigravityModel("gemini-pro-orchestrator"))
        XCTAssertTrue(MobileCore.isAntigravityModel("claude-opus-5-5"))
        XCTAssertTrue(MobileCore.isAntigravityModel("gpt-oss-120b"))
        XCTAssertTrue(MobileCore.isAntigravityModel("antigravity"))

        XCTAssertFalse(MobileCore.isAntigravityModel("inkling-2-mini"))
        XCTAssertFalse(MobileCore.isAntigravityModel("inkling"))
        XCTAssertFalse(MobileCore.isAntigravityModel("default"))
        XCTAssertFalse(MobileCore.isAntigravityModel(""))
        XCTAssertFalse(MobileCore.isAntigravityModel("--danger"))

        XCTAssertEqual(MobileCore.normalizeAntigravityModel("gemini-4-hydrogen"), "gemini-4-hydrogen")
        XCTAssertEqual(MobileCore.normalizeAntigravityModel("claude-opus-5-5"), "claude-opus-5-5")
        XCTAssertEqual(MobileCore.normalizeAntigravityModel("antigravity"), "gemini-4-neon")
        XCTAssertEqual(MobileCore.normalizeAntigravityModel("inkling-2-mini"), "gemini-4-neon")

        XCTAssertEqual(MobileCore.normalizeAntigravityEffort(model: "gemini-4-neon", want: "xhigh"), "xhigh")
        XCTAssertEqual(MobileCore.normalizeAntigravityEffort(model: "gemini-4-neon", want: "low"), "low")
        XCTAssertEqual(MobileCore.normalizeAntigravityEffort(model: "claude-opus-5-5", want: "max"), "max")
        XCTAssertEqual(MobileCore.normalizeAntigravityEffort(model: "gemini-3.8-flash", want: "xhigh"), "high")
        XCTAssertEqual(MobileCore.normalizeAntigravityEffort(model: "gemini-pro-orchestrator", want: "high"), "")
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

        // Local Inkling and invalid review modes must be rejected by Coax before sealing.
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
            model: "inkling-2-mini",
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
            {"effort_id": "xhigh", "display_name": "X-High"},
            {"effort_id": "max", "display_name": "Max"}
          ],
          "default_effort": "medium",
          "models": [
            {"model_id": "gemini-4-neon", "display_name": "Gemini 4 Neon", "efforts": ["low", "medium", "high", "xhigh"]},
            {"model_id": "inkling-2-mini", "display_name": "Inkling 2 Mini", "efforts": ["low", "medium", "high"]},
            {"model_id": "gemini-pro-orchestrator", "display_name": "Gemini Pro Orchestrator", "efforts": []},
            {"model_id": "claude-opus-5-5", "display_name": "Claude Opus 5.5", "efforts": ["high", "max"]},
            {"model_id": "gemini-3.8-flash", "display_name": "Gemini 3.8 Flash", "efforts": ["low", "medium", "high"]}
          ],
          "host_version": "2.0.0",
          "agy_version": "1.5.14",
          "agy_auth_method": "oauth-personal",
          "agy_signed_in": true,
          "capabilities": ["wire.v2", "agy.dynamic_models", "agy.update"]
        }
        """
        let cleanData = Data(rawJSON.utf8)
        let work = try XCTUnwrap(MobileCoreWork())
        XCTAssertTrue(MobileCore.validateView(work: work, host: host, method: "DescribeEnvironment", raw: cleanData))

        let parsed = try XCTUnwrap(ParsedEnvironmentView.parse(cleanData))
        XCTAssertEqual(parsed.antigravityModels.map(\.id), ["gemini-4-neon", "gemini-pro-orchestrator", "claude-opus-5-5", "gemini-3.8-flash"])
        let orch = try XCTUnwrap(parsed.antigravityModels.first(where: { $0.id == "gemini-pro-orchestrator" }))
        XCTAssertTrue(orch.supportedEfforts.isEmpty)
        XCTAssertFalse(orch.supportsThinking)
        let claude = try XCTUnwrap(parsed.antigravityModels.first(where: { $0.id == "claude-opus-5-5" }))
        XCTAssertEqual(claude.supportedEfforts, [.high, .max])
        XCTAssertEqual(parsed.hostMeta?.hostVersion, "2.0.0")
        XCTAssertEqual(parsed.hostMeta?.agyVersion, "1.5.14")
        XCTAssertTrue(parsed.hostMeta?.supportsRemoteUpdate == true)
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

    func testAcceptEditsTurboOptionIDAndAttachmentsWireEncoding() throws {
        let work = try XCTUnwrap(MobileCoreWork())
        XCTAssertTrue(MobileCore.isValidReviewMode("accept-edits"))
        XCTAssertTrue(MobileCore.isValidReviewMode("turbo"))
        XCTAssertEqual(ReviewMode.acceptEdits.wireValue, "accept-edits")
        XCTAssertEqual(ReviewMode.turbo.wireValue, "turbo")
        XCTAssertEqual(ReviewMode.fromWire("accept-edits"), .acceptEdits)
        XCTAssertEqual(ReviewMode.fromWire("default"), .ask)

        let resp = try XCTUnwrap(MobileCore.encodeEnvironmentRequest(
            work: work,
            method: "RespondInteraction",
            session: "0123456789abcdef0123456789abcdef",
            command: "abcdef0123456789abcdef0123456789",
            generation: 1,
            revision: 1,
            interaction: "permit-7",
            allow: false,
            issuedMs: 1_700_000_000_000,
            expiresMs: 1_700_000_300_000,
            optionID: "reject_always"
        ))
        let respJSON = try XCTUnwrap(JSONSerialization.jsonObject(with: resp) as? [String: Any])
        XCTAssertEqual(respJSON["option_id"] as? String, "reject_always")

        // Structured attachment JSON round-trips on OpenSession/SendMessage and omits key when empty
        let png = try makeTestPNG(width: 480, height: 320)
        let att = FileAttachment.prepareImage(data: png, name: "dense ui.png")
        let wireJSON = FileAttachment.encodeWireAttachmentsJSON([att])
        XCTAssertFalse(wireJSON.isEmpty)
        let sendWithAtt = try XCTUnwrap(MobileCore.encodeEnvironmentRequest(
            work: work,
            method: "SendMessage",
            session: "0123456789abcdef0123456789abcdef",
            command: "abcdef0123456789abcdef0123456789",
            generation: 1,
            revision: 1,
            text: "Inspect screenshot",
            issuedMs: 1_700_000_000_000,
            expiresMs: 1_700_000_300_000,
            model: "gemini-4-neon",
            effort: "medium",
            mode: "auto",
            attachmentsJSON: wireJSON
        ))
        let attReqJSON = try XCTUnwrap(JSONSerialization.jsonObject(with: sendWithAtt) as? [String: Any])
        let attArr = try XCTUnwrap(attReqJSON["attachments"] as? [[String: Any]])
        XCTAssertEqual(attArr.count, 1)
        XCTAssertEqual(attArr[0]["mime_type"] as? String, "image/jpeg")
    }

    func testParsedEnvironmentViewDecodesInteractionWarningOptionsAndSessionMode() throws {
        let host = String(repeating: "a", count: 64)
        let rawJSON = """
        {
          "schema": "openagents.environment.runtime.v1alpha1",
          "host_pubkey": "\(host)",
          "display_name": "Studio Mac",
          "method": "ReadSession",
          "sessions": [
            {
              "session_id": "0123456789abcdef0123456789abcdef",
              "workspace_id": "default",
              "title": "Interactive Approval",
              "origin": "managed",
              "control": "available",
              "generation": 1,
              "revision": 2,
              "busy": true,
              "interaction_id": "permit-3",
              "updated_at_ms": 1700000000000,
              "model": "gemini-4-neon",
              "effort": "high",
              "mode": "default"
            }
          ],
          "items": [],
          "error": "",
          "complete": true,
          "next_cursor": "",
          "command_id": "",
          "command_status": "",
          "source_epoch": "",
          "interaction_prompt": "Run rm -rf /tmp/scratch",
          "interaction_warning": "Destructive command detected",
          "interaction_question": false,
          "interaction_options": [
            {"optionId": "allow_once", "name": "Allow Once", "kind": "allow_once"},
            {"optionId": "reject_always", "name": "Always Deny", "kind": "reject_always"}
          ]
        }
        """
        let data = Data(rawJSON.utf8)
        let work = try XCTUnwrap(MobileCoreWork())
        XCTAssertTrue(MobileCore.validateView(work: work, host: host, method: "ReadSession", raw: data))
        let parsed = try XCTUnwrap(ParsedEnvironmentView.parse(data))
        XCTAssertEqual(parsed.sessions.first?.mode, "default")
        XCTAssertEqual(parsed.interactionPrompt, "Run rm -rf /tmp/scratch")
        XCTAssertEqual(parsed.interactionWarning, "Destructive command detected")
        XCTAssertEqual(parsed.interactionOptions.count, 2)
        XCTAssertEqual(parsed.interactionOptions[0].id, "allow_once")
        XCTAssertTrue(parsed.interactionOptions[0].isAllow)
        XCTAssertEqual(parsed.interactionOptions[1].id, "reject_always")
        XCTAssertFalse(parsed.interactionOptions[1].isAllow)
    }

    func testWorkspaceChangedFilesHandlesSpacesQuotesRenamesAndPaginationMerge() {
        let base = WorkspaceSnapshot(
            host: String(repeating: "a", count: 64),
            workspace: "default",
            revision: String(repeating: "1", count: 64),
            path: "/Users/dev/openagents",
            repository: "OpenAgentsInc/openagents",
            branch: "main",
            status: " M \"dir with spaces/file name.swift\"\nR  old/path.swift -> new/renamed path.swift",
            diff: """
            diff --git "a/dir with spaces/file name.swift" "b/dir with spaces/file name.swift"
            @@ -1 +1 @@
            -old
            +new
            """,
            diffCut: false,
            error: "",
            githubError: "",
            worktrees: [],
            branches: [],
            pullRequests: []
        )
        let files = base.changedFiles
        XCTAssertEqual(files.count, 2)
        XCTAssertEqual(files[0].path, "dir with spaces/file name.swift")
        XCTAssertTrue(files[0].diff.contains("+new"))
        XCTAssertEqual(files[1].path, "new/renamed path.swift")

        let tree = WorkspaceWorktree(id: String(repeating: "b", count: 64), path: "/tmp/wt", branch: "feat/x", locked: false)
        let pr = WorkspacePR(number: 42, title: "Fix P0s", url: "", head: "feat/x", base: "main", body: "", state: "open")
        let merged = base.merging(worktrees: [tree], branches: ["main", "feat/x"], pullRequests: [pr], githubError: "")
        XCTAssertEqual(merged.worktrees, [tree])
        XCTAssertEqual(merged.branches, ["main", "feat/x"])
        XCTAssertEqual(merged.pullRequests, [pr])

        // UTF-8 byte-safe truncation never exceeds byte budget on multi-byte emoji
        let emoji = String(repeating: "🚀", count: 10) // 40 UTF-8 bytes
        let cut = emoji.truncatedToUTF8Bytes(10)
        XCTAssertLessThanOrEqual(cut.utf8.count, 10)
        XCTAssertEqual(cut, "🚀🚀")
    }

    func testWave2ErrorCoverageTypedFFIDiagnosticsPoolAndPersistenceRecovery() throws {
        // 1. 100% coverage of Coax EA_ERRORS, HB_CODES, workspace, and tailnet refusal codes (#14006)
        let allCoaxRefusals = [
            "unsupported", "forbidden", "missing_right", "expired", "revoked", "stale",
            "wrong_code", "denied", "refused", "conflict", "stale_cursor", "record_limit",
            "discovery_incomplete", "no_agent", "model_unavailable", "capacity", "not_found",
            "unsupported_profile", "folder_refused", "workspace_refused", "stale_state",
            "dirty_worktree", "workspace_busy", "workspace_locked", "github_unavailable",
            "github_remote_required", "workspace_unavailable", "repository_unavailable",
            "diff_unavailable", "branches_unavailable", "invalid_branch", "detached_head",
            "operation_failed", "storage_unavailable", "source_unavailable", "interaction_expired",
            "malformed", "rate_limited", "too_large", "bounds", "output_limit",
            "idempotency_conflict", "not_serving", "busy", "tagged", "not_owner", "not_tailnet", "unavailable"
        ]
        for code in allCoaxRefusals {
            let msg = HostErrorFormatter.describe(code)
            XCTAssertFalse(msg.isEmpty, "Empty message for \(code)")
            XCTAssertNotEqual(msg, code, "Naked snake_case leaked for \(code)")
            XCTAssertFalse(msg.contains("Host refused request ("), "Fell through to default for \(code)")
        }
        for ht in ["tagged", "not_owner", "not_tailnet", "unavailable", "not_serving", "busy", "malformed"] {
            let msg = ReachabilityState.humanRefusal(ht)
            XCTAssertFalse(msg.contains("Refused ("), "Tailnet refusal fell through for \(ht)")
        }

        // 2. Typed C FFI diagnostics & MobileCorePool reuse (#14006)
        let pool = MobileCorePool(maxCached: 2)
        let w1 = try XCTUnwrap(pool.checkout())
        XCTAssertNil(MobileCore.encodeEnvironmentRequest(work: w1, method: "OpenSession", text: String(repeating: "x", count: 16_385)))
        XCTAssertEqual(w1.lastError, .textTooLong)
        XCTAssertNil(MobileCore.encodeEnvironmentRequest(work: w1, method: "OpenSession", text: "hi", model: "inkling-2-mini"))
        XCTAssertEqual(w1.lastError, .invalidModel)
        XCTAssertNil(MobileCore.encodeEnvironmentRequest(work: w1, method: "OpenSession", text: "hi", folder: "../etc"))
        XCTAssertEqual(w1.lastError, .invalidFolder)
        XCTAssertNil(MobileCore.encodeEnvironmentRequest(work: w1, method: "OpenSession", text: "hi", mode: "bogus"))
        XCTAssertEqual(w1.lastError, .invalidMode)
        XCTAssertFalse(MobileCore.validateView(work: w1, host: String(repeating: "a", count: 64), method: "ReadSession", raw: Data("not json".utf8)))
        XCTAssertEqual(w1.lastError, .jsonSyntax)
        XCTAssertFalse(MobileCore.validateView(work: w1, host: String(repeating: "a", count: 64), method: "ReadSession", raw: Data("{\"bad\":1}".utf8)))
        XCTAssertEqual(w1.lastError, .schemaMismatch)
        let ptr1 = w1.raw
        pool.checkin(w1)
        let w2 = try XCTUnwrap(pool.checkout())
        XCTAssertEqual(w2.raw, ptr1, "MobileCorePool should reuse pooled arena without reallocating")
        pool.checkin(w2)
        pool.drain()

        // 3. MagicDNS-only dialOrder & AppTailnetBook limit = 32 & exponential backoff (#14007)
        let dnsOnly = AppTailnetComputer(host: String(repeating: "a", count: 64), ip: "", dns: "studio.ts.net", port: 47109, name: "Studio")
        XCTAssertEqual(dnsOnly.dialOrder, ["studio.ts.net"])
        let both = AppTailnetComputer(host: String(repeating: "a", count: 64), ip: "100.64.0.5", dns: "studio.ts.net", port: 47109, name: "Studio")
        XCTAssertEqual(both.dialOrder, ["100.64.0.5", "studio.ts.net"])
        XCTAssertEqual(AppTailnetBook.limit, 32)
        XCTAssertEqual(TailscaleClient.backoffDelaySeconds(attempt: 0, jitterUnit: 0), 1.0)
        XCTAssertEqual(TailscaleClient.backoffDelaySeconds(attempt: 1, jitterUnit: 0), 2.0)
        XCTAssertEqual(TailscaleClient.backoffDelaySeconds(attempt: 2, jitterUnit: 0), 4.0)
        XCTAssertEqual(TailscaleClient.backoffDelaySeconds(attempt: 3, jitterUnit: 0), 8.0)
        XCTAssertEqual(TailscaleClient.backoffDelaySeconds(attempt: 6, jitterUnit: 1), 15.0)

        // 4. Corrupt paired JSON triggers backup instead of silent wipe (#14007)
        let tmpDir = FileManager.default.temporaryDirectory.appendingPathComponent("oa-corrupt-test-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: tmpDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: tmpDir) }
        let pairedFile = tmpDir.appendingPathComponent("own-computer-paired-v1.json")
        try Data("{corrupt json".utf8).write(to: pairedFile)
        let devStore = MobileDeviceStore(
            tailnetBookURL: tmpDir.appendingPathComponent("book.json"),
            cacheURL: tmpDir.appendingPathComponent("cache.json"),
            pairedFileURL: pairedFile
        )
        XCTAssertTrue(devStore.pairedComputers.isEmpty)
        XCTAssertNotNil(devStore.lastPersistenceWarning)
        let files = try FileManager.default.contentsOfDirectory(atPath: tmpDir.path)
        XCTAssertTrue(files.contains(where: { $0.hasPrefix("own-computer-paired-v1.json.corrupt.") }))
    }

    func testWave3BWorkspaceFileViewerGitStagingAndDaemonSessionMeta() throws {
        let work = try XCTUnwrap(MobileCoreWork())
        let host = String(repeating: "a", count: 64)
        let rev = String(repeating: "1", count: 64)
        let cmd = String(repeating: "f", count: 32)
        let nowMs: Int64 = 1_790_000_000_000

        // 1. New Workspace file-viewer & git staging/commit requests encode cleanly via Coax (#14012)
        let treeReq = try XCTUnwrap(MobileCore.encodeWorkspaceRequest(
            work: work, method: "ReadWorkspaceTree", title: "apps/", cursor: 0
        ))
        XCTAssertTrue(String(decoding: treeReq, as: UTF8.self).contains("\"ReadWorkspaceTree\""))

        let fileReq = try XCTUnwrap(MobileCore.encodeWorkspaceRequest(
            work: work, method: "ReadWorkspaceFile", title: "README.md", cursor: 1
        ))
        XCTAssertTrue(String(decoding: fileReq, as: UTF8.self).contains("\"ReadWorkspaceFile\""))

        // Traversal paths MUST be refused by Coax ws_rel_path_ok!
        XCTAssertNil(MobileCore.encodeWorkspaceRequest(
            work: work, method: "ReadWorkspaceFile", title: "../etc/passwd", cursor: 1
        ))

        let stageReq = try XCTUnwrap(MobileCore.encodeWorkspaceRequest(
            work: work, method: "StageFile", title: "README.md", expectedRevision: rev, command: cmd, issuedMs: nowMs, expiresMs: nowMs + 60_000
        ))
        XCTAssertTrue(String(decoding: stageReq, as: UTF8.self).contains("\"StageFile\""))

        let commitReq = try XCTUnwrap(MobileCore.encodeWorkspaceRequest(
            work: work, method: "CommitChanges", title: "feat: update", body: "details", number: 1, expectedRevision: rev, command: cmd, issuedMs: nowMs, expiresMs: nowMs + 60_000
        ))
        XCTAssertTrue(String(decoding: commitReq, as: UTF8.self).contains("\"CommitChanges\""))

        // 2. Daemon ListSessionMeta & UpdateSessionMeta encode & validate via Coax (#14013)
        let listMeta = try XCTUnwrap(MobileCore.encodeDaemonRequest(work: work, method: "ListSessionMeta"))
        XCTAssertTrue(String(decoding: listMeta, as: UTF8.self).contains("\"ListSessionMeta\""))

        let updateMeta = try XCTUnwrap(MobileCore.encodeDaemonRequest(
            work: work,
            method: "UpdateSessionMeta",
            session: "0123456789abcdef0123456789abcdef",
            command: cmd,
            title: "My Pinned Chat",
            pinned: true,
            archived: false,
            issuedMs: nowMs,
            expiresMs: nowMs + 60_000
        ))
        let updateStr = String(decoding: updateMeta, as: UTF8.self)
        XCTAssertTrue(updateStr.contains("\"UpdateSessionMeta\""))
        XCTAssertTrue(updateStr.contains("\"pinned\":true"))
        XCTAssertTrue(updateStr.contains("\"My Pinned Chat\""))

        let rawDaemonResp = """
        {"schema":"openagents.environment.daemon.v1alpha1","ok":true,"host":"\(host)","label":"Studio","method":"ListSessionMeta","pid":123,"started_at":1790000000,"paired_devices":1,"active_slots":0,"max_slots":4,"relay":"","workspace":"/repo","foreman":{"tier":1,"active_runs":0,"total_sessions":1,"workspace":"/repo","prompt":"","updated":0},"sessions_meta":[{"id":"0123456789abcdef0123456789abcdef","title":"Pinned Thread","folder":"apps","pinned":true,"archived":false,"updated":1790000001}]}
        """
        let rawData = Data(rawDaemonResp.utf8)
        XCTAssertTrue(MobileCore.validateDaemonView(work: work, host: host, method: "ListSessionMeta", raw: rawData))
        let parsed = try XCTUnwrap(ParsedDaemonView.parse(rawData))
        XCTAssertTrue(parsed.ok)
        XCTAssertEqual(parsed.sessions.count, 1)
        XCTAssertEqual(parsed.sessions[0].title, "Pinned Thread")
        XCTAssertTrue(parsed.sessions[0].pinned)

        // 3. Inline review comment formatting (#14012)
        let comment = InlineReviewComment(filePath: "apps/openagents-ios/Sources/ChatView.swift", lineRange: "L42-L55", comment: "Extract helper")
        XCTAssertEqual(comment.renderedMarkdown, "- `apps/openagents-ios/Sources/ChatView.swift #L42-L55`: Extract helper")
    }
}
