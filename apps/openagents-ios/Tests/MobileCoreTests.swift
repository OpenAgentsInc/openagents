// Firm reason: 6 (XCTest unit suite verifying OpenAgents iOS Cloud Mode: #authority Coax Cloud Chat v2, durable submit/stop, Tier 1 Cloud Tools allowlist, Auto placement, thought_signature preservation, Environments/Projects/Usage parsing, Full Auto, and Bi-Directional Voice Duplex).
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers
import XCTest
@testable import OpenAgents

@MainActor
final class MobileCoreTests: XCTestCase {
    func testCloudChatV2ComputeProvenanceAndMetaCodecs() throws {
        // #authority cc_compute_valid: local compute MUST have 0 msat cost.
        XCTAssertTrue(CloudCore.isValidCompute("local", costMsat: 0))
        XCTAssertFalse(CloudCore.isValidCompute("local", costMsat: 1))
        XCTAssertTrue(CloudCore.isValidCompute("cloud", costMsat: 0))
        XCTAssertTrue(CloudCore.isValidCompute("cloud", costMsat: 250))
        XCTAssertTrue(CloudCore.isValidCompute("sandbox", costMsat: 5625))
        XCTAssertFalse(CloudCore.isValidCompute("bogus", costMsat: 0))

        // #authority cc_meta_write encodes closed pin/rename/archive mutations.
        let metaData = try XCTUnwrap(CloudCore.encodeMeta(
            operation: "ios:meta:1",
            title: "Cloud Thread",
            pinned: true,
            archived: false
        ))
        let metaJSON = try XCTUnwrap(JSONSerialization.jsonObject(with: metaData) as? [String: Any])
        XCTAssertEqual(metaJSON["schema"] as? String, ["openagents", "chat", "meta", "v2"].joined(separator: "."))
        XCTAssertEqual(metaJSON["operation_id"] as? String, "ios:meta:1")
        XCTAssertEqual(metaJSON["title"] as? String, "Cloud Thread")
        XCTAssertEqual(metaJSON["pinned"] as? Bool, true)
        XCTAssertEqual(metaJSON["archived"] as? Bool, false)
    }

    func testDurableSubmitAndStopCodecs() throws {
        // Fresh durable turn (no conversation ID => expected_revision must be 0).
        let fresh = try XCTUnwrap(CloudCore.encodeSubmit(
            operation: "ios:sub:1",
            conversation: "",
            revision: 0,
            project: "prj_0123456789abcdef0123456789abcdef",
            projectRevision: 3,
            prompt: "Run coax test on enrolled computer"
        ))
        let freshJSON = try XCTUnwrap(JSONSerialization.jsonObject(with: fresh) as? [String: Any])
        XCTAssertEqual(freshJSON["schema"] as? String, ["openagents", "chat", "submit", "v2alpha1"].joined(separator: "."))
        XCTAssertEqual(freshJSON["conversation_id"] as? String, "")
        XCTAssertEqual(freshJSON["expected_revision"] as? Int, 0)
        XCTAssertEqual(freshJSON["project_id"] as? String, "prj_0123456789abcdef0123456789abcdef")
        XCTAssertEqual(freshJSON["expected_project_revision"] as? Int, 3)

        // Existing conversation requires valid cnv_<32hex> and revision >= 1.
        XCTAssertNil(CloudCore.encodeSubmit(
            operation: "ios:sub:bad",
            conversation: "not_a_conv_id",
            revision: 1,
            prompt: "hi"
        ))
        let followUp = try XCTUnwrap(CloudCore.encodeSubmit(
            operation: "ios:sub:2",
            conversation: "cnv_0123456789abcdef0123456789abcdef",
            revision: 2,
            prompt: "Follow up"
        ))
        let followJSON = try XCTUnwrap(JSONSerialization.jsonObject(with: followUp) as? [String: Any])
        XCTAssertEqual(followJSON["conversation_id"] as? String, "cnv_0123456789abcdef0123456789abcdef")
        XCTAssertEqual(followJSON["expected_revision"] as? Int, 2)

        // Canonical lowercase UUIDs are generated and accepted alongside legacy prefixed IDs.
        let newConv = MobileCloudChat.newConversationID()
        let newTurn = MobileCloudChat.newTurnID()
        let detTurn1 = MobileCloudChat.turnID(conversationID: newConv, ordinal: 1)
        let detTurn2 = MobileCloudChat.turnID(conversationID: newConv, ordinal: 2)
        XCTAssertTrue(MobileCloudChat.isCanonicalUUID(newConv) && MobileCloudChat.isValidConversationID(newConv))
        XCTAssertTrue(MobileCloudChat.isCanonicalUUID(newTurn))
        XCTAssertTrue(MobileCloudChat.isCanonicalUUID(detTurn1) && detTurn1 == MobileCloudChat.turnID(conversationID: newConv, ordinal: 1) && detTurn1 != detTurn2)
        XCTAssertTrue(MobileCloudChat.isValidConversationID("cnv_0123456789abcdef0123456789abcdef"))
        XCTAssertFalse(MobileCloudChat.isValidConversationID("550E8400-e29b-41d4-a716-446655440000"))
        XCTAssertFalse(MobileCloudChat.isValidConversationID("trn_0123456789abcdef0123456789abcdef"))

        let uuidConv = "550e8400-e29b-41d4-a716-446655440000"
        let uuidProj = "660e8400-e29b-41d4-a716-446655440000"
        let uuidTurn = "770e8400-e29b-41d4-a716-446655440000"
        let uuidFollow = try XCTUnwrap(CloudCore.encodeSubmit(
            operation: "ios:sub:uuid",
            conversation: uuidConv,
            revision: 3,
            project: uuidProj,
            projectRevision: 1,
            prompt: "UUID follow up"
        ))
        let uuidFollowJSON = try XCTUnwrap(JSONSerialization.jsonObject(with: uuidFollow) as? [String: Any])
        XCTAssertEqual(uuidFollowJSON["conversation_id"] as? String, uuidConv)
        XCTAssertEqual(uuidFollowJSON["project_id"] as? String, uuidProj)

        let uuidStop = try XCTUnwrap(CloudCore.encodeStop(operation: "ios:stop:uuid", turn: uuidTurn))
        let uuidStopJSON = try XCTUnwrap(JSONSerialization.jsonObject(with: uuidStop) as? [String: Any])
        XCTAssertEqual(uuidStopJSON["turn_id"] as? String, uuidTurn)

        // Durable turn stop request validation (legacy trn_ remains supported).
        let stop = try XCTUnwrap(CloudCore.encodeStop(
            operation: "ios:stop:1",
            turn: "trn_0123456789abcdef0123456789abcdef"
        ))
        let stopJSON = try XCTUnwrap(JSONSerialization.jsonObject(with: stop) as? [String: Any])
        XCTAssertEqual(stopJSON["schema"] as? String, ["openagents", "chat", "stop", "v2alpha1"].joined(separator: "."))
        XCTAssertEqual(stopJSON["turn_id"] as? String, "trn_0123456789abcdef0123456789abcdef")
        XCTAssertNil(CloudCore.encodeStop(operation: "ios:stop:2", turn: "invalid_turn"))
    }

    func testTier1CloudToolsAllowlistSchemaAndPlacementClassifier() throws {
        // Tier 1 read-only allowlist enforced by Coax tool-wire.coax.
        for allowed in ["web_search", "web_fetch", "github_tree", "github_read_file", "github_search_code", "github_list_issues", "github_get_pr"] {
            XCTAssertTrue(CloudCore.isCloudToolAllowed(allowed), "Expected Tier 1 tool \(allowed) to be allowed")
        }
        for refused in ["bash", "shell", "write_file", "rm", ""] {
            XCTAssertFalse(CloudCore.isCloudToolAllowed(refused), "Expected non-Tier-1 tool \(refused) to be refused")
        }

        let tools = CloudCore.webToolsObjects()
        XCTAssertEqual(tools.count, 7)

        // Auto placement prompt classification via Coax placement-policy.coax.
        XCTAssertEqual(CloudCore.classifyPrompt("Explain how Bitcoin Taproot Assets work"), CloudCore.workReadOnly)
        XCTAssertEqual(CloudCore.classifyPrompt("Run coax test and fix the build error"), CloudCore.workCodeExec)
    }

    func testStreamingInferenceChunkAccumulatesToolCallsAndPreservesThoughtSignature() throws {
        var text = ""
        var thought = ""
        var callsByIndex: [Int: StreamedToolCallAccumulator] = [:]

        let chunk1: [String: Any] = [
            "choices": [[
                "delta": [
                    "reasoning_content": "Checking GitHub repo tree. ",
                    "tool_calls": [[
                        "index": 0,
                        "id": "call_gh_1",
                        "type": "function",
                        "function": ["name": "github_tree", "arguments": "{\"repo\":\"OpenAgentsInc/"],
                        "extra_content": ["google": ["thought_signature": "AY89sigPart1"]],
                    ]],
                ],
            ]],
        ]
        let chunk2: [String: Any] = [
            "choices": [[
                "delta": [
                    "content": "Inspecting repository...",
                    "tool_calls": [[
                        "index": 0,
                        "function": ["arguments": "openagents\"}"],
                        "extra_content": ["google": ["thought_signature": "+Part2=="]],
                    ]],
                ],
            ]],
        ]

        MobileCloudChat.applyInferenceChunk(chunk1, text: &text, thought: &thought, toolCallsByIndex: &callsByIndex)
        MobileCloudChat.applyInferenceChunk(chunk2, text: &text, thought: &thought, toolCallsByIndex: &callsByIndex)

        XCTAssertEqual(thought, "Checking GitHub repo tree. ")
        XCTAssertEqual(text, "Inspecting repository...")
        let call = try XCTUnwrap(callsByIndex[0])
        XCTAssertEqual(call.id, "call_gh_1")
        XCTAssertEqual(call.name, "github_tree")
        XCTAssertEqual(call.arguments, "{\"repo\":\"OpenAgentsInc/openagents\"}")
        XCTAssertEqual(call.thoughtSignature, "AY89sigPart1+Part2==")

        // Echoed assistant tool_calls message preserves extra_content.google.thought_signature verbatim!
        let msg = MobileCloudChat.encodeAssistantToolCallMessage(content: text, toolCalls: [call])
        XCTAssertEqual(msg["role"] as? String, "assistant")
        let encodedCalls = try XCTUnwrap(msg["tool_calls"] as? [[String: Any]])
        XCTAssertEqual(encodedCalls.count, 1)
        let extra = try XCTUnwrap(encodedCalls[0]["extra_content"] as? [String: Any])
        let google = try XCTUnwrap(extra["google"] as? [String: Any])
        XCTAssertEqual(google["thought_signature"] as? String, "AY89sigPart1+Part2==")
    }

    func testCloudEnvironmentsProjectsUsageAndExecutionTargetPersistence() throws {
        let host = String(repeating: "a", count: 64)
        let envList: [[String: Any]] = [[
            "host_pubkey": host,
            "display_name": "Studio Mac",
            "os": "macos",
            "online": true,
            "last_seen_ms": 1_790_000_000_000,
            "workspaces": [[
                "workspace_id": "default",
                "name": "openagents",
                "local_path": "/Users/dev/openagents",
                "repo_remote": "https://github.com/OpenAgentsInc/openagents",
                "repo_canonical": "github.com/openagentsinc/openagents",
                "branch": "main",
            ]],
        ]]
        let envs = MobileCloudChat.parseEnvironments(envList)
        XCTAssertEqual(envs.count, 1)
        XCTAssertEqual(envs[0].displayName, "Studio Mac")
        XCTAssertEqual(envs[0].osLabel, "macOS")
        XCTAssertTrue(envs[0].online)
        XCTAssertEqual(envs[0].workspaces.first?.branch, "main")

        let projList: [[String: Any]] = [[
            "id": "prj_0123456789abcdef0123456789abcdef",
            "revision": 4,
            "name": "OpenAgents Core",
            "description": "Coax cloud engine",
            "repo_canonical": "github.com/openagentsinc/openagents",
            "updated_at_ms": 1_790_000_000_000,
        ]]
        let projs = MobileCloudChat.parseProjects(projList)
        XCTAssertEqual(projs.count, 1)
        XCTAssertEqual(projs[0].name, "OpenAgents Core")
        XCTAssertEqual(projs[0].revision, 4)

        let usageRoot: [String: Any] = [
            "today": ["usd_micros": 420_000, "requests": 12],
            "total": ["calls": 28, "in_tokens": 35_000, "out_tokens": 15_000, "usd_micros": 12_800_000],
            "credit_msat": 5_000_000,
            "promo_micros": 15_000_000,
            "promo_expires_at": "2027-02-25T00:00:00Z",
            "sandbox": ["turns": 2, "seconds": 180, "msat": 1125],
            "local": ["turns": 9, "msat": 0],
        ]
        let usage = MobileCloudChat.parseUsageSnapshot(usageRoot)
        XCTAssertEqual(usage.todayUsdMicros, 420_000)
        XCTAssertEqual(usage.totalUsdMicros, 12_800_000)
        XCTAssertEqual(usage.totalTokens, 50_000)
        XCTAssertEqual(usage.totalCalls, 28)
        XCTAssertEqual(usage.creditMsat, 5_000_000)
        XCTAssertEqual(usage.promoMicros, 15_000_000)
        XCTAssertEqual(usage.promoExpiresAt, "2027-02-25T00:00:00Z")
        XCTAssertEqual(usage.localTurns, 9)
        XCTAssertEqual(CloudUsageSnapshot.formatUsd(micros: 420_000), "$0.42")
        XCTAssertEqual(CloudUsageSnapshot.formatTokens(50_000), "50.0k")

        // Paid + Promo credit parsing from /api/auth/life.
        let credits = MobileAccountController.parseCreditFields([
            "credit_msat": 25_000_000,
            "promo_micros": 5_000_000,
            "promo_expires_at": "2027-02-25T00:00:00Z",
        ])
        XCTAssertEqual(credits.creditMsat, 25_000_000)
        XCTAssertEqual(credits.promoMicros, 5_000_000)
        XCTAssertEqual(credits.promoExpiresAt, "2027-02-25T00:00:00Z")

        // Canonical Cloud model catalog includes OpenAgents Auto first and routes cleanly.
        XCTAssertEqual(CloudModelOption.catalog.first?.id, "openagents/auto")
        XCTAssertEqual(CloudModelOption.catalog.first?.label, "OpenAgents Auto")
        XCTAssertEqual(CloudModelOption.catalog.first?.wireModel, "google/gemini-4-neon")
        XCTAssertEqual(CloudModelOption.catalog.count, 5)

        // Execution target round-trip persistence.
        let targets: [CloudExecutionTarget] = [
            .auto,
            .cloud,
            .sandbox(size: "small"),
            .computer(hostPubkey: host, name: "Studio Mac"),
        ]
        for target in targets {
            XCTAssertEqual(CloudExecutionTarget.fromPersisted(target.persistedString), target)
        }
    }

    func testFullAutoAndBiDirectionalVoiceDuplexPolicies() {
        XCTAssertEqual(CloudCore.fullAutoDecidePrompt(fullAuto: true, autoAnswerQuestions: true, kind: 0), 1)
        XCTAssertEqual(CloudCore.fullAutoDecidePrompt(fullAuto: true, autoAnswerQuestions: true, kind: 1), 1)
        XCTAssertEqual(CloudCore.fullAutoDecidePrompt(fullAuto: true, autoAnswerQuestions: false, kind: 1), 0)
        XCTAssertEqual(CloudCore.fullAutoDecidePrompt(fullAuto: false, autoAnswerQuestions: true, kind: 0), 0)
        XCTAssertFalse(CloudCore.fullAutoShouldGate(fullAuto: true, approvalMode: true))
        XCTAssertTrue(CloudCore.fullAutoShouldGate(fullAuto: false, approvalMode: true))

        // VAD silence auto-send & 3-frame barge-in via #authority voice-duplex.coax.
        var vd: [Int64] = [1, 0, 0, 0, 1200, 800, 2200]
        XCTAssertEqual(CloudCore.voiceDuplexVadStep(state: &vd, rmsBp: 1500, dtMs: 100), 0)
        XCTAssertEqual(CloudCore.voiceDuplexVadStep(state: &vd, rmsBp: 100, dtMs: 600), 0)
        XCTAssertEqual(CloudCore.voiceDuplexVadStep(state: &vd, rmsBp: 100, dtMs: 700), 1)

        var vdSpeak: [Int64] = [3, 0, 0, 0, 1200, 800, 2200]
        XCTAssertEqual(CloudCore.voiceDuplexVadStep(state: &vdSpeak, rmsBp: 3000, dtMs: 50), 0)
        XCTAssertEqual(CloudCore.voiceDuplexVadStep(state: &vdSpeak, rmsBp: 3000, dtMs: 50), 0)
        XCTAssertEqual(CloudCore.voiceDuplexVadStep(state: &vdSpeak, rmsBp: 3000, dtMs: 50), 2)

        let sanitized = VoiceDictationController.sanitizeForSpeech("**Done!** Check https://openagents.com/docs and:\n```swift\nlet x = 1\n```")
        XCTAssertFalse(sanitized.contains("*"))
        XCTAssertFalse(sanitized.contains("```"))
        XCTAssertFalse(sanitized.contains("https://"))
        XCTAssertTrue(sanitized.contains("Code block omitted"))

        var streamBuf = "First sentence is ready. Second sentence is still streaming"
        let firstBatch = VoiceDictationController.extractCompleteSentences(from: &streamBuf, flushAll: false)
        XCTAssertEqual(firstBatch, ["First sentence is ready."])
        XCTAssertEqual(streamBuf, "Second sentence is still streaming")
        let finalBatch = VoiceDictationController.extractCompleteSentences(from: &streamBuf, flushAll: true)
        XCTAssertEqual(finalBatch, ["Second sentence is still streaming"])
    }

    @MainActor
    func testIncomingOpenAgentsAskDeepLinkAndAttachmentHelpers() throws {
        let chat = MobileCloudChat(auth: AppAuth(origin: "https://openagents.com"))
        chat.draftText = ""
        let url = try XCTUnwrap(URL(string: "openagents://ask?prompt=Check%20Cloud%20status"))
        chat.handleIncomingURL(url)
        XCTAssertEqual(chat.draftText, "Check Cloud status")

        // UTF-8 byte-safe truncation preserves multi-byte emoji boundaries.
        let emoji = String(repeating: "🚀", count: 10)
        let cut = emoji.truncatedToUTF8Bytes(10)
        XCTAssertLessThanOrEqual(cut.utf8.count, 10)
        XCTAssertEqual(cut, "🚀🚀")

        // Image preparation produces valid JPEG data.
        let cs = CGColorSpaceCreateDeviceRGB()
        let ctx = try XCTUnwrap(CGContext(
            data: nil, width: 64, height: 64, bitsPerComponent: 8, bytesPerRow: 0,
            space: cs, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ))
        ctx.setFillColor(CGColor(red: 0.2, green: 0.7, blue: 0.4, alpha: 1.0))
        ctx.fill(CGRect(x: 0, y: 0, width: 64, height: 64))
        let cg = try XCTUnwrap(ctx.makeImage())
        let out = NSMutableData()
        let dest = try XCTUnwrap(CGImageDestinationCreateWithData(out, UTType.png.identifier as CFString, 1, nil))
        CGImageDestinationAddImage(dest, cg, nil)
        XCTAssertTrue(CGImageDestinationFinalize(dest))
        let att = CloudAttachment.prepareImage(data: out as Data, name: "test shot.png")
        XCTAssertTrue(att.isImage)
        XCTAssertEqual(att.name, "test_shot.png")
        XCTAssertEqual(att.mimeType, "image/jpeg")

        // Multimodal user message preserves text + base64 JPEG image_url part.
        let plainMsg = MobileCloudChat.encodeUserMessage(prompt: "Hello", imageAttachments: [])
        XCTAssertEqual(plainMsg["content"] as? String, "Hello")
        let mmMsg = MobileCloudChat.encodeUserMessage(prompt: "Describe this", imageAttachments: [att])
        let parts = try XCTUnwrap(mmMsg["content"] as? [[String: Any]])
        XCTAssertEqual(parts.count, 2)
        XCTAssertEqual(parts[0]["type"] as? String, "text")
        XCTAssertEqual(parts[1]["type"] as? String, "image_url")
        let imgObj = try XCTUnwrap(parts[1]["image_url"] as? [String: Any])
        XCTAssertTrue((imgObj["url"] as? String)?.hasPrefix("data:image/jpeg;base64,") == true)
    }
}
