// Firm reason: 6 (macOS XCTest unit + live dual-host E2E suite verifying shared Coax C core, multimodal images, thought rendering, Tailscale admission/direct-grant/push codecs, Workspace Git wire, and real NIP-44 TCP conversations inside OpenAgents macOS).
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers
import XCTest
@testable import OpenAgents

final class MacClientTests: XCTestCase {
    func testSharedCoaxAntigravityPolicyAndReviewModes() throws {
        XCTAssertTrue(MobileCore.isCoreModel("oa:auto"))
        XCTAssertTrue(MobileCore.isCoreModel("oa:cloud"))
        XCTAssertTrue(MobileCore.isCoreModel("oa:local"))
        XCTAssertTrue(MobileCore.isCoreModel("oa:gemini-4-neon"))
        XCTAssertTrue(MobileCore.isCoreModel("oa:inkling-2-mini"))
        XCTAssertEqual(MobileCore.normalizeCoreModel("oa:cloud"), "oa:gemini-4-neon")
        XCTAssertEqual(MobileCore.normalizeCoreModel("oa:local"), "oa:inkling-2-mini")
        XCTAssertEqual(MobileCore.normalizeCoreModel("antigravity"), "oa:auto")
        XCTAssertTrue(MobileCore.isAntigravityModel("oa:auto"))
        XCTAssertTrue(MobileCore.isAntigravityModel("gemini-4-neon"))
        XCTAssertTrue(MobileCore.isAntigravityModel("gemini-3.8-flash"))
        XCTAssertTrue(MobileCore.isAntigravityModel("claude-opus-5-5"))
        XCTAssertTrue(MobileCore.isAntigravityModel("gemini-pro-orchestrator"))
        XCTAssertFalse(MobileCore.isAntigravityModel("inkling-2-mini"))
        XCTAssertFalse(MobileCore.isAntigravityModel("default"))
        XCTAssertEqual(MobileCore.normalizeAntigravityModel("claude-opus-5-5"), "claude-opus-5-5")
        XCTAssertEqual(MobileCore.normalizeAntigravityModel("inkling-2-mini"), "gemini-4-neon")
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

    private func makeRedTestPNG(width: Int = 192, height: Int = 192) throws -> Data {
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

    func testMultimodalImagePreparationAndThoughtParsingOnMacOS() throws {
        let png = try makeRedTestPNG()
        let att = FileAttachment.prepareImage(data: png, name: "mac-drop.png")
        XCTAssertTrue(att.isImage)
        XCTAssertLessThanOrEqual(att.imageData?.count ?? 99_999, 3_400)
        let parsed = ParsedUserMessage.parse("Analyze screenshot" + att.renderedPromptBlock())
        XCTAssertEqual(parsed.text, "Analyze screenshot")
        XCTAssertEqual(parsed.images.count, 1)
        XCTAssertNotNil(parsed.images[0].data)

        let heading = ThoughtSummaryParser.extractHeading("**Auditing Workspace**\n\nScanning git diff.")
        XCTAssertEqual(heading, "Auditing Workspace")
        XCTAssertFalse(heading.contains("*"))
    }

    @MainActor
    func testLiveEndToEndTailscaleClientAgainstE2EHosts() async throws {
        let env = ProcessInfo.processInfo.environment
        guard let legacyPortStr = env["OA_MAC_E2E_LEGACY_PORT"], let legacyPort = UInt16(legacyPortStr),
              let v2PortStr = env["OA_MAC_E2E_V2_PORT"], let v2Port = UInt16(v2PortStr) else {
            throw XCTSkip("Run via apps/openagents-macos/e2e-mac-test.sh to execute live dual-host macOS E2E tests.")
        }

        let devStore = MobileDeviceStore()
        devStore.resetForTests()
        let store = AntigravityStore(store: devStore)
        store.client.browsesLocalNetwork = false
        store.allowLoopbackForTests = true

        // 1. Strict 16-key legacy v1 host: pair, DescribeEnvironment, default Auto send, Plan auto-downgrade, and real image attachment!
        let legacyErr = await store.addManualComputer(address: "127.0.0.1", name: "Legacy V1 Mac", port: legacyPort, autoPair: true)
        XCTAssertNil(legacyErr, "Direct-grant pairing to legacy v1 host failed: \(legacyErr ?? "none")")
        XCTAssertEqual(store.workspaceName, "openagents")
        XCTAssertNil(store.bannerError)
        XCTAssertTrue(store.availableModels.allSatisfy { MobileCore.isAntigravityModel($0.id) })

        let imgAtt = FileAttachment.prepareImage(data: try makeRedTestPNG(width: 96, height: 96), name: "swatch.png")
        store.addAttachment(imgAtt)
        store.draftText = "Hello macOS image"
        await store.sendMessage()
        XCTAssertNil(store.bannerError, "Legacy v1 Auto send with image failed: \(store.bannerError ?? "")")
        XCTAssertNotNil(store.localImageData(for: "swatch.png"))
        XCTAssertTrue(store.items.contains(where: { $0.role == .thought && $0.text.contains("Confirming System Functionality") }))
        XCTAssertTrue(store.items.contains(where: { $0.role == .assistant && $0.text.contains("Hello macOS image") && $0.text.contains("[oa-image ") }))

        // Plan mode on strict v1 host MUST transparently downgrade without leaking "unsupported"
        store.chooseReviewMode(.plan)
        store.draftText = "Plan macOS refactor"
        await store.sendMessage()
        XCTAssertNil(store.bannerError, "Legacy v1 Plan auto-downgrade leaked error: \(store.bannerError ?? "")")
        XCTAssertTrue(store.items.contains(where: { $0.role == .assistant && $0.text.contains("Plan macOS refactor") }))

        // Ask mode on strict v1 host MUST NOT silently downgrade to Auto — surfaces clear error & keeps draft (#13996)!
        store.chooseReviewMode(.ask)
        store.draftText = "Do not auto-run this"
        await store.sendMessage()
        XCTAssertNotNil(store.bannerError, "Ask mode on legacy v1 host must refuse silent downgrade to Auto")
        XCTAssertEqual(store.draftText, "Do not auto-run this")
        store.draftText = ""
        store.bannerError = nil

        // 2. Modern v2 host: pair, interactive approval with options/warning, structured attachments, subagent, paginated Workspace + RemoveWorktree!
        let v2Err = await store.addManualComputer(address: "127.0.0.1", name: "E2E Studio Mac", port: v2Port, autoPair: true)
        XCTAssertNil(v2Err, "Direct-grant pairing to v2 host failed: \(v2Err ?? "none")")
        store.chooseReviewMode(.auto)
        let v2Img = FileAttachment.prepareImage(data: try makeRedTestPNG(width: 128, height: 128), name: "v2-shot.png")
        store.addAttachment(v2Img)
        // Full Auto defaults ON (#14068); disable temporarily to verify interactive Approval banner & options.
        XCTAssertTrue(store.fullAutoEnabled)
        store.setFullAuto(false)
        store.draftText = "Run macOS build and approve"
        await store.sendMessage()
        XCTAssertNil(store.bannerError)
        let pending = try XCTUnwrap(store.pendingInteraction, "Expected interactive Approval banner from v2 host")
        XCTAssertTrue(pending.prompt.contains("cargo test"))
        XCTAssertFalse(pending.warning.isEmpty)
        XCTAssertEqual(pending.options.count, 3)
        XCTAssertTrue(store.items.contains(where: { $0.role == .assistant && $0.text.contains("[structured-attachments]") }))
        await store.respondToInteraction(allow: true, note: "Looks good", optionID: "allow_once")
        XCTAssertNil(store.pendingInteraction)
        XCTAssertTrue(store.items.contains(where: { $0.role == .assistant && $0.text == "Approved and executed cleanly." }))

        let subItem = try XCTUnwrap(store.items.first(where: { $0.isSubagent }))
        store.openSubagent(subItem)
        await store.refreshSubagent()
        XCTAssertTrue(store.subagentItems.contains(where: { $0.text.contains("Subagent inspected 12 files cleanly.") }))

        // Paginated ReadWorkspace (page=0..3) populates status/diff, 2 worktrees, 2 branches, 1 PR (#13999)
        await store.refreshWorkspace()
        XCTAssertEqual(store.workspaceSnapshot?.branch, "main")
        XCTAssertEqual(store.workspaceSnapshot?.branches, ["main", "feat/antigravity"])
        XCTAssertEqual(store.workspaceSnapshot?.worktrees.count, 2)
        XCTAssertEqual(store.workspaceSnapshot?.pullRequests.count, 1)

        // Remove secondary worktree by 64-hex tree.id (#13999)
        let secondary = try XCTUnwrap(store.workspaceSnapshot?.worktrees.first(where: { $0.path.contains("openagents-wt") }))
        await store.removeWorktree(id: secondary.id)
        XCTAssertNil(store.workspaceError)
        XCTAssertEqual(store.workspaceSnapshot?.worktrees.count, 1)

        await store.switchBranch("feat/antigravity")
        XCTAssertEqual(store.workspaceSnapshot?.branch, "feat/antigravity")
        XCTAssertNil(store.workspaceError)

        store.onBackground()
    }

    func testAntigravityAuthProbeJSONParser() throws {
        let sample = """
        {"installed":true,"binaryPath":"/opt/homebrew/bin/agy","version":"1.5.14","authStatus":"authenticated","authMethod":"oauth-personal","authLabel":"Personal Google Account","status":"ready","models":[{"slug":"gemini-4-neon","name":"Gemini 4 Neon"}]}
        """
        let parsed = try XCTUnwrap(AntigravityProbeResult.parse(Data(sample.utf8)))
        XCTAssertTrue(parsed.installed)
        XCTAssertTrue(parsed.isAuthenticated)
        XCTAssertEqual(parsed.version, "1.5.14")
        XCTAssertEqual(parsed.modelCount, 1)
        XCTAssertFalse(AntigravityAuthMethod.oauthPersonal.usesIsolatedProfile)
        XCTAssertTrue(AntigravityAuthMethod.geminiAPIKey.usesIsolatedProfile)
        XCTAssertTrue(AntigravityAuthMethod.oauthBusiness.usesIsolatedProfile)
        XCTAssertTrue(AntigravityAuthMethod.agentPlatform.usesIsolatedProfile)
    }

    @MainActor
    func testTailscaleCLIEnvironmentAndNewChatComposerFocus() throws {
        let env = TailscaleHostServer.daemonEnvironment(base: ["PATH": "/usr/bin:/bin"])
        XCTAssertEqual(env["TAILSCALE_BE_CLI"], "1")
        XCTAssertEqual(env["TERM"], "dumb")
        XCTAssertEqual(env["BROWSER"], "/usr/bin/true")

        let preserved = TailscaleHostServer.daemonEnvironment(base: ["PATH": "/usr/bin:/bin", "TERM": "xterm-256color"])
        XCTAssertEqual(preserved["TAILSCALE_BE_CLI"], "1")
        XCTAssertEqual(preserved["TERM"], "xterm-256color")

        let devStore = MobileDeviceStore()
        devStore.resetForTests()
        let store = AntigravityStore(store: devStore)
        let before = store.composerFocusNonce
        store.startNewChat()
        XCTAssertEqual(store.composerFocusNonce, before + 1)
    }
}
