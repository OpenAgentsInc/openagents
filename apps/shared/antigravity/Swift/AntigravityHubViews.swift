// Firm reason: 6 (cross-Apple pure-SwiftUI Antigravity Control Hub: Artifacts & pure-SwiftUI Generative UI viewer, Swarms & Subagents, Automations, Extensions/MCP/Permissions/Sandbox, and Voice/Browser/Quotas companion — ZERO WebKit/JS, #14014-#14018).
import AVKit
import SwiftUI

struct AntigravityHubView: View {
    @ObservedObject var store: AntigravityStore
    @StateObject private var micBridge = AntigravityMicBridge()
    @Environment(\.dismiss) private var dismiss

    enum Tab: String, CaseIterable, Identifiable {
        case artifacts = "Artifacts"
        case swarms = "Swarms"
        case automations = "Automations"
        case extensions = "Security & MCP"
        case companion = "Voice & Quotas"
        var id: String { rawValue }
    }

    @State private var selectedTab: Tab = .artifacts
    @State private var swarmSearch: String = ""
    @State private var swarmTarget: String = "coordinator"
    @State private var swarmMessage: String = ""
    @State private var artifactCommentDraft: String = ""
    @State private var imageZoom: CGFloat = 1.0
    @State private var micPortText: String = "4713"
    @State private var copiedExport: Bool = false

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(Tab.allCases) { tab in
                            Button {
                                selectedTab = tab
                                loadTab(tab)
                            } label: {
                                Text(tab.rawValue)
                                    .font(.system(size: 12, weight: selectedTab == tab ? .semibold : .medium))
                                    .foregroundStyle(selectedTab == tab ? Color.black : OATheme.textSecondary)
                                    .padding(.horizontal, 11)
                                    .padding(.vertical, 6)
                                    .background(selectedTab == tab ? Color.white : OATheme.surface, in: Capsule())
                                    .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(.horizontal, 14)
                    .padding(.vertical, 10)
                }

                Divider().overlay(OATheme.hairline)

                ScrollView {
                    VStack(alignment: .leading, spacing: 14) {
                        switch selectedTab {
                        case .artifacts:
                            artifactsSection
                        case .swarms:
                            swarmsSection
                        case .automations:
                            automationsSection
                        case .extensions:
                            extensionsSection
                        case .companion:
                            companionSection
                        }
                    }
                    .padding(14)
                }
            }
            .background(OATheme.background)
            .navigationTitle("Antigravity Hub")
            #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            #endif
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") {
                        micBridge.stop()
                        dismiss()
                    }
                }
            }
        }
        #if os(macOS)
        .frame(minWidth: 640, minHeight: 560)
        #endif
        .preferredColorScheme(.dark)
        .onAppear {
            loadTab(selectedTab)
            Task { await store.refreshAntigravityConfig() }
        }
        .onDisappear {
            micBridge.stop()
        }
    }

    private func loadTab(_ tab: Tab) {
        Task {
            switch tab {
            case .artifacts:
                await store.refreshArtifacts()
            case .swarms:
                await store.refreshSwarm()
            case .automations:
                await store.refreshAutomations()
            case .extensions, .companion:
                await store.refreshAntigravityConfig()
            }
        }
    }

    // MARK: - 1. Artifacts & Pure-SwiftUI Generative UI Viewer (#14014)

    @ViewBuilder
    private var artifactsSection: some View {
        // Artifact Review Policy selector
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("ARTIFACT REVIEW POLICY")
                    .font(.system(size: 10.5, weight: .semibold))
                    .tracking(0.6)
                    .foregroundStyle(OATheme.textMuted)
                Spacer()
                Button {
                    Task { await store.refreshArtifacts() }
                } label: {
                    Image(systemName: "arrow.clockwise")
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.textSecondary)
                }
                .buttonStyle(.plain)
            }

            HStack(spacing: 6) {
                let current = store.agyConfig?.artifactReviewPolicy ?? .asksForReview
                ForEach(ArtifactReviewPolicy.allCases) { pol in
                    Button {
                        Task { await store.updateArtifactReviewPolicy(pol) }
                    } label: {
                        Text(pol.label)
                            .font(.system(size: 11, weight: current == pol ? .semibold : .regular))
                            .foregroundStyle(current == pol ? Color.black : OATheme.textSecondary)
                            .padding(.horizontal, 10)
                            .padding(.vertical, 5)
                            .background(current == pol ? Color.white : OATheme.surface, in: Capsule())
                            .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                    }
                    .buttonStyle(.plain)
                }
            }
        }

        if let doc = store.activeArtifact {
            artifactViewerCard(doc)
        }

        if store.isLoadingArtifacts && store.artifacts.isEmpty {
            HStack(spacing: 8) {
                ProgressView().controlSize(.small)
                Text("Loading artifacts from host brain directory…")
                    .font(.system(size: 12.5))
                    .foregroundStyle(OATheme.textSecondary)
            }
            .padding(12)
        } else if store.artifacts.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                Text("No artifacts in this session yet")
                    .font(.system(size: 13.5, weight: .semibold))
                    .foregroundStyle(OATheme.textPrimary)
                Text("Run /plan, /learn, or /generative_ui to produce Implementation Plans, Walkthroughs, Screenshots, and pure-SwiftUI Generative UI cards.")
                    .font(.system(size: 12))
                    .foregroundStyle(OATheme.textSecondary)
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        } else {
            VStack(spacing: 8) {
                ForEach(store.artifacts) { art in
                    VStack(alignment: .leading, spacing: 6) {
                        HStack(spacing: 8) {
                            Image(systemName: art.icon)
                                .font(.system(size: 13))
                                .foregroundStyle(art.requestFeedback ? OATheme.statusWarn : OATheme.textSecondary)
                            VStack(alignment: .leading, spacing: 2) {
                                Text(art.displayName)
                                    .font(.system(size: 13, weight: .semibold))
                                    .foregroundStyle(OATheme.textPrimary)
                                Text(art.id)
                                    .font(.system(size: 11, design: .monospaced))
                                    .foregroundStyle(OATheme.textMuted)
                            }
                            Spacer()
                            if art.requestFeedback {
                                Button("Proceed") {
                                    Task { await store.proceedArtifactPlan(art.id) }
                                }
                                .font(.system(size: 11.5, weight: .semibold))
                                .foregroundStyle(Color.black)
                                .padding(.horizontal, 11)
                                .padding(.vertical, 5)
                                .background(Color.white, in: Capsule())
                                .buttonStyle(.plain)
                            }
                            Button("Inspect") {
                                imageZoom = 1.0
                                Task { await store.readArtifact(art.id) }
                            }
                            .font(.system(size: 11.5, weight: .medium))
                            .foregroundStyle(OATheme.textPrimary)
                            .padding(.horizontal, 10)
                            .padding(.vertical, 5)
                            .background(OATheme.elevated, in: Capsule())
                            .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                            .buttonStyle(.plain)
                        }
                        if !art.summary.isEmpty {
                            Text(art.summary)
                                .font(.system(size: 12))
                                .foregroundStyle(OATheme.textSecondary)
                                .lineLimit(3)
                        }
                    }
                    .padding(11)
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                }
            }
        }
    }

    @ViewBuilder
    private func artifactViewerCard(_ doc: ArtifactDocument) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Text(doc.name)
                    .font(.system(size: 13, weight: .semibold, design: .monospaced))
                    .foregroundStyle(OATheme.textPrimary)
                Spacer()
                Button {
                    OAClipboard.copy(doc.content)
                    copiedExport = true
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { copiedExport = false }
                } label: {
                    Label(copiedExport ? "Copied" : "Export", systemImage: copiedExport ? "checkmark" : "square.and.arrow.up")
                        .font(.system(size: 11, weight: .medium))
                        .foregroundStyle(OATheme.textSecondary)
                }
                .buttonStyle(.plain)
                Button {
                    store.activeArtifact = nil
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 14))
                        .foregroundStyle(OATheme.textMuted)
                }
                .buttonStyle(.plain)
            }

            if doc.isImage, let imgData = doc.imageData {
                VStack(alignment: .leading, spacing: 6) {
                    InlineCGImageView(data: imgData, maxWidth: 520, maxHeight: 360, cornerRadius: 10)
                        .scaleEffect(imageZoom)
                        .gesture(MagnificationGesture().onChanged { val in
                            imageZoom = min(3.0, max(1.0, val))
                        })
                    Text("Pinch to zoom · Add region annotation below")
                        .font(.system(size: 10.5))
                        .foregroundStyle(OATheme.textMuted)
                }
            } else if doc.mimeType == "text/html" {
                PureSwiftHTMLArtifactView(html: doc.content)
            } else {
                MarkdownContentView(text: doc.content, fontSize: 13.5, onOpenFileLink: { path, line in
                    store.isHubPresented = false
                    store.openFileFromLink(line.map { "\(path)#L\($0)" } ?? path)
                })
            }

            // Inline artifact comment composer -> queues into inlineReviewComments
            HStack(spacing: 8) {
                TextField("Comment on \(doc.name)…", text: $artifactCommentDraft)
                    .textFieldStyle(.plain)
                    .font(.system(size: 12.5))
                    .foregroundStyle(OATheme.textPrimary)
                    .padding(8)
                    .background(OATheme.background, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                Button("Add Comment") {
                    store.addInlineComment(filePath: doc.name, lineRange: "artifact", comment: artifactCommentDraft)
                    artifactCommentDraft = ""
                }
                .font(.system(size: 11.5, weight: .semibold))
                .foregroundStyle(Color.black)
                .padding(.horizontal, 11)
                .padding(.vertical, 7)
                .background(Color.white, in: Capsule())
                .buttonStyle(.plain)
                .disabled(artifactCommentDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }
        .padding(12)
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(OATheme.hairlineStrong, lineWidth: 1))
    }

    // MARK: - 2. Multi-Agent Swarms, Subagents & Dynamic Workflows (#14015)

    @ViewBuilder
    private var swarmsSection: some View {
        HStack {
            Text("SWARM MEMBERS & SUBAGENTS")
                .font(.system(size: 10.5, weight: .semibold))
                .tracking(0.6)
                .foregroundStyle(OATheme.textMuted)
            Spacer()
            Button {
                Task { await store.refreshSwarm() }
            } label: {
                Image(systemName: "arrow.clockwise")
                    .font(.system(size: 12))
                    .foregroundStyle(OATheme.textSecondary)
            }
            .buttonStyle(.plain)
        }

        let members = store.swarmSnapshot?.members ?? []
        if members.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                Text("No active swarm workers or subagents")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(OATheme.textPrimary)
                Text("Use /swarm, /teamwork, /boost, or invoke_subagent to coordinate parallel agents. Any quota-paused worker can switch models in one tap below.")
                    .font(.system(size: 12))
                    .foregroundStyle(OATheme.textSecondary)
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        } else {
            ForEach(members) { member in
                HStack(spacing: 10) {
                    Circle()
                        .fill(member.isActive ? OATheme.statusReady : (member.state.contains("quota") ? OATheme.statusWarn : OATheme.textMuted))
                        .frame(width: 8, height: 8)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("@\(member.name)")
                            .font(.system(size: 13, weight: .semibold, design: .monospaced))
                            .foregroundStyle(OATheme.textPrimary)
                        Text("\(member.model) · \(member.state)")
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundStyle(OATheme.textSecondary)
                    }
                    Spacer()
                    Menu("Switch Model") {
                        ForEach(store.availableModels.prefix(8)) { model in
                            Button(model.label) {
                                Task { await store.switchSwarmMemberModel(member: member.name, model: model.id) }
                            }
                        }
                    }
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(OATheme.textPrimary)
                }
                .padding(10)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
            }
        }

        // Swarm Message Board & @mention composer
        Text("SWARM BOARD")
            .font(.system(size: 10.5, weight: .semibold))
            .tracking(0.6)
            .foregroundStyle(OATheme.textMuted)

        HStack(spacing: 8) {
            TextField("Filter board messages…", text: $swarmSearch)
                .textFieldStyle(.plain)
                .font(.system(size: 12))
                .foregroundStyle(OATheme.textPrimary)
                .padding(8)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        }

        let posts = (store.swarmSnapshot?.posts ?? []).filter {
            swarmSearch.isEmpty || $0.message.localizedCaseInsensitiveContains(swarmSearch) || $0.author.localizedCaseInsensitiveContains(swarmSearch)
        }
        if !posts.isEmpty {
            ForEach(posts) { post in
                VStack(alignment: .leading, spacing: 4) {
                    Text("@\(post.author)")
                        .font(.system(size: 11, weight: .semibold, design: .monospaced))
                        .foregroundStyle(OATheme.textSecondary)
                    Text(post.message)
                        .font(.system(size: 12.5))
                        .foregroundStyle(OATheme.textPrimary)
                        .textSelection(.enabled)
                }
                .padding(10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
            }
        }

        HStack(spacing: 8) {
            TextField("@coordinator", text: $swarmTarget)
                .textFieldStyle(.plain)
                .font(.system(size: 12, design: .monospaced))
                .foregroundStyle(OATheme.textPrimary)
                .padding(8)
                .frame(width: 115)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
            TextField("Post @mention to swarm…", text: $swarmMessage)
                .textFieldStyle(.plain)
                .font(.system(size: 12.5))
                .foregroundStyle(OATheme.textPrimary)
                .padding(8)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
            Button("Send") {
                let msg = swarmMessage
                let target = swarmTarget
                swarmMessage = ""
                Task { await store.sendSwarmMention(to: target, message: msg) }
            }
            .font(.system(size: 12, weight: .semibold))
            .foregroundStyle(Color.black)
            .padding(.horizontal, 12)
            .padding(.vertical, 7)
            .background(Color.white, in: Capsule())
            .buttonStyle(.plain)
            .disabled(swarmMessage.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
        }
    }

    // MARK: - 3. Schedules & Persistent /automation Sidecars (#14016)

    @ViewBuilder
    private var automationsSection: some View {
        HStack {
            Text("SCHEDULES & /AUTOMATION SIDECARS")
                .font(.system(size: 10.5, weight: .semibold))
                .tracking(0.6)
                .foregroundStyle(OATheme.textMuted)
            Spacer()
            Button {
                Task { await store.refreshAutomations() }
            } label: {
                Image(systemName: "arrow.clockwise")
                    .font(.system(size: 12))
                    .foregroundStyle(OATheme.textSecondary)
            }
            .buttonStyle(.plain)
        }

        if store.isLoadingAutomations && store.automations.isEmpty {
            ProgressView().controlSize(.small)
        } else if store.automations.isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                Text("No scheduled tasks or Antigravity sidecars configured")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(OATheme.textPrimary)
                Text("Create recurring automations with /automation or /schedule on your host computer; they will appear here with Pause/Resume and Run Now controls.")
                    .font(.system(size: 12))
                    .foregroundStyle(OATheme.textSecondary)
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        } else {
            ForEach(store.automations) { auto in
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 8) {
                        Circle()
                            .fill(auto.enabled ? OATheme.statusReady : OATheme.textMuted)
                            .frame(width: 7, height: 7)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(auto.name)
                                .font(.system(size: 13.5, weight: .semibold))
                                .foregroundStyle(OATheme.textPrimary)
                            Text("\(auto.cron) · \(auto.model)\(auto.isSidecar ? " · sidecar" : "")")
                                .font(.system(size: 11, design: .monospaced))
                                .foregroundStyle(OATheme.textSecondary)
                        }
                        Spacer()
                        if !auto.isSidecar {
                            Button(auto.enabled ? "Pause" : "Resume") {
                                Task { await store.toggleAutomation(auto) }
                            }
                            .font(.system(size: 11, weight: .medium))
                            .foregroundStyle(OATheme.textPrimary)
                            .padding(.horizontal, 9)
                            .padding(.vertical, 4)
                            .background(OATheme.elevated, in: Capsule())
                            .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                            .buttonStyle(.plain)

                            Button("Run Now") {
                                Task { await store.runAutomationNow(auto) }
                            }
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(Color.black)
                            .padding(.horizontal, 10)
                            .padding(.vertical, 4)
                            .background(Color.white, in: Capsule())
                            .buttonStyle(.plain)
                        }
                    }
                    if !auto.prompt.isEmpty {
                        Text(auto.prompt)
                            .font(.system(size: 12))
                            .foregroundStyle(OATheme.textSecondary)
                            .lineLimit(2)
                    }
                    HStack {
                        if auto.lastRunMs > 0 {
                            Text("Last run: \(OATimeFormat.shortAge(updatedAtMs: auto.lastRunMs))")
                                .font(.system(size: 10.5, design: .monospaced))
                                .foregroundStyle(OATheme.textMuted)
                        }
                        Spacer()
                        if !auto.lastChatID.isEmpty,
                           let targetSession = store.sessions.first(where: { $0.id == auto.lastChatID }) {
                            Button("Open Last Run") {
                                store.selectSession(targetSession)
                                dismiss()
                            }
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(OATheme.textPrimary)
                            .buttonStyle(.plain)
                        }
                    }
                }
                .padding(11)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
            }
        }
    }

    // MARK: - 4. Permissions, Sandbox, MCP Servers & Customizations (#14017)

    @ViewBuilder
    private var extensionsSection: some View {
        let cfg = store.agyConfig
        VStack(alignment: .leading, spacing: 10) {
            Text("PERMISSIONS & OS SANDBOX")
                .font(.system(size: 10.5, weight: .semibold))
                .tracking(0.6)
                .foregroundStyle(OATheme.textMuted)

            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    Text("Tool Permission Baseline")
                        .font(.system(size: 12.5, weight: .medium))
                        .foregroundStyle(OATheme.textPrimary)
                    Spacer()
                    Menu(cfg?.toolPermission ?? "request-review") {
                        ForEach(["request-review", "proceed-in-sandbox", "strict", "always-proceed"], id: \.self) { opt in
                            Button(opt) {
                                Task { await store.updateAntigravityConfig(patch: ["toolPermission": opt]) }
                            }
                        }
                    }
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(OATheme.textPrimary)
                }

                Toggle("Enable OS Terminal Sandbox", isOn: Binding(
                    get: { cfg?.enableTerminalSandbox ?? false },
                    set: { val in Task { await store.updateAntigravityConfig(patch: ["enableTerminalSandbox": val]) } }
                ))
                .font(.system(size: 12.5))
                .foregroundStyle(OATheme.textPrimary)

                HStack(spacing: 10) {
                    badgeChip(label: cfg?.sandboxProxy == true ? "Sandbox Proxy: Active" : "Sandbox Proxy: Off", ok: cfg?.sandboxProxy == true)
                    badgeChip(label: cfg?.adminSettingsPresent == true ? "Enterprise Admin Ceilings Active" : "Personal Policy", ok: true)
                }

                if let rules = cfg?.permissionRules, !rules.isEmpty {
                    Divider().overlay(OATheme.hairline)
                    ForEach(rules) { rule in
                        HStack(spacing: 6) {
                            Text(rule.bucket.uppercased())
                                .font(.system(size: 9.5, weight: .bold, design: .monospaced))
                                .foregroundStyle(rule.bucket == "deny" ? OATheme.statusError : (rule.bucket == "allow" ? OATheme.statusReady : OATheme.statusWarn))
                                .frame(width: 44, alignment: .leading)
                            Text(rule.rule)
                                .font(.system(size: 11.5, design: .monospaced))
                                .foregroundStyle(OATheme.textPrimary)
                                .lineLimit(1)
                        }
                    }
                }
            }
            .padding(12)
            .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        }

        // MCP Servers & MCP Apps (ui://) pure-SwiftUI inspector
        VStack(alignment: .leading, spacing: 8) {
            Text("MCP SERVERS & UI:// APPS")
                .font(.system(size: 10.5, weight: .semibold))
                .tracking(0.6)
                .foregroundStyle(OATheme.textMuted)
            if let mcps = cfg?.mcpServers, !mcps.isEmpty {
                ForEach(mcps) { srv in
                    HStack {
                        Circle()
                            .fill(srv.enabled ? OATheme.statusReady : OATheme.textMuted)
                            .frame(width: 7, height: 7)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(srv.name)
                                .font(.system(size: 13, weight: .semibold, design: .monospaced))
                                .foregroundStyle(OATheme.textPrimary)
                            Text(srv.endpointOrCommand)
                                .font(.system(size: 11, design: .monospaced))
                                .foregroundStyle(OATheme.textMuted)
                                .lineLimit(1)
                        }
                        Spacer()
                        Text(srv.enabled ? "Enabled" : "Disabled")
                            .font(.system(size: 10.5, design: .monospaced))
                            .foregroundStyle(srv.enabled ? OATheme.statusReady : OATheme.textMuted)
                    }
                    .padding(10)
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                }
            } else {
                Text("No custom MCP servers found in ~/.gemini/antigravity/mcp_config.json.")
                    .font(.system(size: 12))
                    .foregroundStyle(OATheme.textMuted)
            }
        }

        // Skills, Rules, Custom Agents (--agent), Plugins
        if let customs = cfg?.customizations, !customs.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text("SKILLS, RULES, CUSTOM AGENTS & PLUGINS (\(customs.count))")
                    .font(.system(size: 10.5, weight: .semibold))
                    .tracking(0.6)
                    .foregroundStyle(OATheme.textMuted)
                ForEach(customs) { item in
                    HStack {
                        Text(item.kind.uppercased())
                            .font(.system(size: 9.5, weight: .bold, design: .monospaced))
                            .foregroundStyle(OATheme.textSecondary)
                            .frame(width: 56, alignment: .leading)
                        Text(item.name)
                            .font(.system(size: 12.5, design: .monospaced))
                            .foregroundStyle(OATheme.textPrimary)
                        Spacer()
                        if item.kind == "agent" {
                            Button(store.selectedCustomAgent == item.name ? "Active" : "Use") {
                                store.selectedCustomAgent = store.selectedCustomAgent == item.name ? "" : item.name
                            }
                            .font(.system(size: 10.5, weight: .semibold))
                            .foregroundStyle(store.selectedCustomAgent == item.name ? Color.black : OATheme.textPrimary)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 3)
                            .background(store.selectedCustomAgent == item.name ? Color.white : OATheme.elevated, in: Capsule())
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 7)
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                }
            }
        }
    }

    // MARK: - 5. Voice PCM Bridge, Browser Companion & Official Quotas/Credits (#14018)

    @ViewBuilder
    private var companionSection: some View {
        // Voice Mode (`agy mic-serve` 16 kHz PCM bridge over Tailscale)
        VStack(alignment: .leading, spacing: 10) {
            Text("VOICE LIVE BRIDGE (AGY MIC-SERVE PCM)")
                .font(.system(size: 10.5, weight: .semibold))
                .tracking(0.6)
                .foregroundStyle(OATheme.textMuted)

            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    let hostAddr = store.activeComputer?.isLocalMac == true
                        ? "127.0.0.1"
                        : ((store.activeComputer?.ip.isEmpty == false ? store.activeComputer?.ip : store.activeComputer?.dns) ?? "")
                    TextField("4713", text: $micPortText)
                        .textFieldStyle(.plain)
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(OATheme.textPrimary)
                        .frame(width: 68)
                        .padding(7)
                        .background(OATheme.background, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                    Button(micBridge.isStreaming ? "Stop PCM Stream" : "Start Live Mic Bridge") {
                        let port = UInt16(micPortText) ?? 4713
                        micBridge.toggle(host: hostAddr, port: port, hasPendingApproval: store.pendingInteraction != nil)
                    }
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(micBridge.isStreaming ? Color.white : Color.black)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 7)
                    .background(micBridge.isStreaming ? OATheme.statusError : Color.white, in: Capsule())
                    .buttonStyle(.plain)
                    Spacer()
                }
                Text(micBridge.statusLabel)
                    .font(.system(size: 11.5, design: .monospaced))
                    .foregroundStyle(OATheme.textSecondary)
                if micBridge.isStreaming {
                    ProgressView(value: micBridge.audioLevel)
                        .tint(OATheme.statusReady)
                }
                if let err = micBridge.lastError {
                    Text(err)
                        .font(.system(size: 11.5))
                        .foregroundStyle(OATheme.statusWarn)
                }
                Text("Safety policy: Destructive tool approvals cannot be confirmed by voice and always require an explicit tap.")
                    .font(.system(size: 11))
                    .foregroundStyle(OATheme.textMuted)
            }
            .padding(12)
            .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        }

        // Browser Use Companion (CAPTCHA handoff & video/screenshot media)
        VStack(alignment: .leading, spacing: 8) {
            Text("BROWSER USE COMPANION")
                .font(.system(size: 10.5, weight: .semibold))
                .tracking(0.6)
                .foregroundStyle(OATheme.textMuted)

            if let handoff = store.browserHandoffWarning {
                HStack(alignment: .top, spacing: 8) {
                    Image(systemName: "hand.raised.fill")
                        .foregroundStyle(OATheme.statusWarn)
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Human Sign-In / CAPTCHA Handoff Required")
                            .font(.system(size: 12.5, weight: .semibold))
                            .foregroundStyle(OATheme.statusWarn)
                        Text(handoff)
                            .font(.system(size: 12))
                            .foregroundStyle(OATheme.textPrimary)
                    }
                }
                .padding(11)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(OATheme.statusWarn.opacity(0.12), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.statusWarn.opacity(0.4), lineWidth: 1))
            } else {
                Text("No active browser SSO/MFA/CAPTCHA pause. Browser screenshots and .mp4 recordings generated during sessions appear in Artifacts and inline in chat.")
                    .font(.system(size: 12))
                    .foregroundStyle(OATheme.textSecondary)
                    .padding(11)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
            }
        }

        // Official agy Quotas, AI Credit Overages & Smart Rate-Limit State (ZERO undocumented HTTP endpoints!)
        VStack(alignment: .leading, spacing: 10) {
            Text("QUOTAS, AI CREDITS & RATE LIMITS (OFFICIAL AGY ONLY)")
                .font(.system(size: 10.5, weight: .semibold))
                .tracking(0.6)
                .foregroundStyle(OATheme.textMuted)

            VStack(alignment: .leading, spacing: 10) {
                HStack {
                    Text("Runtime")
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.textSecondary)
                    Spacer()
                    Text("agy \(store.agyConfig?.agyVersion.isEmpty == false ? (store.agyConfig?.agyVersion ?? "") : (store.activeHostMeta?.agyVersion ?? "installed")) · \(store.agyConfig?.authMode.isEmpty == false ? (store.agyConfig?.authMode ?? "") : "OAuth")")
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(OATheme.textPrimary)
                }

                Toggle("Allow AI Credit Overages (useG1Credits)", isOn: Binding(
                    get: { store.agyConfig?.useG1Credits ?? false },
                    set: { val in Task { await store.updateAntigravityConfig(patch: ["useG1Credits": val]) } }
                ))
                .font(.system(size: 12.5))
                .foregroundStyle(OATheme.textPrimary)

                if let rl = store.rateLimitBanner {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(rl.isHardExhaustion ? "Quota Exhausted — Switch Model" : "Transient 429 — Auto-Retrying")
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(rl.isHardExhaustion ? OATheme.statusError : OATheme.statusWarn)
                        Text(rl.message)
                            .font(.system(size: 11.5))
                            .foregroundStyle(OATheme.textPrimary)
                        if rl.isHardExhaustion, let fallback = store.availableModels.first(where: { $0.id != store.selectedModelID }) {
                            Button("Switch to \(fallback.label)") {
                                store.chooseModel(fallback.id)
                                store.rateLimitBanner = nil
                            }
                            .font(.system(size: 11.5, weight: .semibold))
                            .foregroundStyle(Color.black)
                            .padding(.horizontal, 11)
                            .padding(.vertical, 5)
                            .background(Color.white, in: Capsule())
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                }
            }
            .padding(12)
            .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        }
    }

    private func badgeChip(label: String, ok: Bool) -> some View {
        Text(label)
            .font(.system(size: 10.5, weight: .medium, design: .monospaced))
            .foregroundStyle(ok ? OATheme.textPrimary : OATheme.textMuted)
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(OATheme.elevated, in: Capsule())
            .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
    }
}

/// Pure-SwiftUI Generative UI HTML Artifact Viewer (extracts title, headings, text blocks, and code cleanly — ZERO WebKit/JS!).
private struct PureSwiftHTMLArtifactView: View {
    let html: String
    @State private var showRawSource = false

    private var extractedText: String {
        var s = html
        // Strip <style> and <script> blocks cleanly via regex
        for tag in ["style", "script"] {
            if let rx = try? NSRegularExpression(pattern: "<\(tag)[^>]*>[\\s\\S]*?</\(tag)>", options: [.caseInsensitive]) {
                s = rx.stringByReplacingMatches(in: s, range: NSRange(s.startIndex..., in: s), withTemplate: "")
            }
        }
        // Convert headings and block tags to markdown-like lines
        s = s.replacingOccurrences(of: "<br[^>]*>", with: "\n", options: .regularExpression)
        s = s.replacingOccurrences(of: "</(p|div|li|tr|h[1-6])>", with: "\n", options: .regularExpression)
        if let tagRx = try? NSRegularExpression(pattern: "<[^>]+>", options: []) {
            s = tagRx.stringByReplacingMatches(in: s, range: NSRange(s.startIndex..., in: s), withTemplate: "")
        }
        s = s.replacingOccurrences(of: "&amp;", with: "&")
            .replacingOccurrences(of: "&lt;", with: "<")
            .replacingOccurrences(of: "&gt;", with: ">")
            .replacingOccurrences(of: "&quot;", with: "\"")
        let lines = s.split(separator: "\n").map { $0.trimmingCharacters(in: .whitespaces) }.filter { !$0.isEmpty }
        return lines.joined(separator: "\n\n")
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Label("Generative UI Artifact (Native Safe Preview)", systemImage: "rectangle.3.group")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(OATheme.textSecondary)
                Spacer()
                Button(showRawSource ? "Rendered Text" : "HTML Source") {
                    showRawSource.toggle()
                }
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(OATheme.textPrimary)
                .buttonStyle(.plain)
            }
            if showRawSource {
                CodeBlockView(language: "html", code: html)
            } else {
                MarkdownContentView(text: extractedText.isEmpty ? "Interactive HTML widget (\(html.utf8.count) bytes). Tap Export to open externally." : extractedText, fontSize: 13)
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(OATheme.background, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            }
        }
    }
}
