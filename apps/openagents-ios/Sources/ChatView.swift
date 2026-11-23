// Firm reason: 6 (iOS SwiftUI chat transcript with Markdown/code blocks, collapsible thoughts/tools/diffs, and native Antigravity composer bar).
import SwiftUI

struct ChatView: View {
    @ObservedObject var store: AppStore
    @FocusState private var composerFocused: Bool

    var body: some View {
        VStack(spacing: 0) {
            if let error = store.bannerError, !error.isEmpty {
                errorBanner(error)
            }

            Group {
                if store.activeComputer?.canSendRPC != true && store.items.isEmpty {
                    tailscaleOnboardingState
                } else if store.items.isEmpty {
                    readyEmptyChatState
                } else {
                    transcriptScrollView
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .contentShape(Rectangle())
            .onTapGesture {
                composerFocused = false
            }

            Divider().overlay(OATheme.hairline)
            composerBar
        }
        .background(OATheme.background)
    }

    // MARK: - Error banner

    private func errorBanner(_ text: String) -> some View {
        HStack(spacing: 10) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 13))
                .foregroundStyle(OATheme.statusWarn)
            Text(text)
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textPrimary)
                .lineLimit(3)
            Spacer()
            Button {
                store.bannerError = nil
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(OATheme.textSecondary)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(OATheme.surface)
        .overlay(alignment: .bottom) { Divider().overlay(OATheme.hairline) }
    }

    // MARK: - Empty states

    private var tailscaleOnboardingState: some View {
        ScrollView {
            VStack(spacing: 18) {
                Spacer(minLength: 44)
                AntigravityMark(size: 36, color: OATheme.textPrimary)

                VStack(spacing: 8) {
                    Text("Antigravity over Tailscale")
                        .font(.system(size: 20, weight: .semibold))
                        .foregroundStyle(OATheme.textPrimary)
                    Text("Connect directly to Antigravity running on your Mac over your private Tailscale network — no cloud relay or account required.")
                        .font(.system(size: 14))
                        .foregroundStyle(OATheme.textSecondary)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 320)
                }

                VStack(alignment: .leading, spacing: 10) {
                    stepRow(number: "1", text: "Sign into Tailscale on this iPhone and on your Mac.")
                    stepRow(number: "2", text: "Keep OpenAgents running on your Mac (port 47109).")
                    stepRow(number: "3", text: "Tap Connect Computer to discover nearby or enter its MagicDNS / 100.x IP.")
                }
                .padding(14)
                .frame(maxWidth: 340, alignment: .leading)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))

                Button {
                    store.isComputersPresented = true
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: "desktopcomputer")
                            .font(.system(size: 14, weight: .semibold))
                        Text("Connect Computer")
                            .font(.system(size: 15, weight: .semibold))
                    }
                    .foregroundStyle(Color.black)
                    .padding(.horizontal, 20)
                    .padding(.vertical, 11)
                    .background(Color.white, in: Capsule())
                }
                .accessibilityIdentifier("chat.connectTailscale")

                Spacer(minLength: 32)
            }
            .padding(.horizontal, 20)
        }
    }

    private func stepRow(number: String, text: String) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Text(number)
                .font(.system(size: 11, weight: .bold, design: .monospaced))
                .foregroundStyle(OATheme.textPrimary)
                .frame(width: 20, height: 20)
                .background(OATheme.elevated, in: Circle())
                .overlay(Circle().stroke(OATheme.hairline, lineWidth: 1))
            Text(text)
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textSecondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private var readyEmptyChatState: some View {
        VStack(spacing: 14) {
            Spacer()
            AntigravityMark(size: 30, color: OATheme.textPrimary)
            Text(store.activeComputer?.name ?? "Antigravity")
                .font(.system(size: 18, weight: .semibold))
                .foregroundStyle(OATheme.textPrimary)
            Text(store.workspaceName.isEmpty
                 ? "Ready for a new Antigravity task over Tailscale."
                 : "Workspace: \(store.workspaceName)")
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textSecondary)

            if !store.folders.isEmpty {
                Menu {
                    Button("Workspace root") { store.selectedFolder = "" }
                    ForEach(store.folders, id: \.self) { folder in
                        Button(folder) { store.selectedFolder = folder }
                    }
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "folder")
                            .font(.system(size: 12))
                        Text(store.selectedFolder.isEmpty ? "Workspace root" : store.selectedFolder)
                            .font(.system(size: 12, weight: .medium, design: .monospaced))
                        Image(systemName: "chevron.down")
                            .font(.system(size: 9))
                    }
                    .foregroundStyle(OATheme.textSecondary)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .background(OATheme.surface, in: Capsule())
                    .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                }
            }
            Spacer()
        }
        .padding(.horizontal, 24)
    }

    // MARK: - Transcript

    private var transcriptScrollView: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 14) {
                    if let note = store.activeSession?.note, !note.isEmpty {
                        noticeCard(text: note, kind: "note")
                    }
                    ForEach(store.items) { item in
                        TranscriptRowView(item: item)
                            .id(item.id)
                    }
                    if store.activeSession?.busy == true {
                        HStack(spacing: 8) {
                            ProgressView()
                                .controlSize(.small)
                                .tint(OATheme.textSecondary)
                            Text("Antigravity is working…")
                                .font(.system(size: 13))
                                .foregroundStyle(OATheme.textSecondary)
                        }
                        .padding(.top, 4)
                        .id("working-indicator")
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 16)
            }
            .scrollDismissesKeyboard(.interactively)
            .onChange(of: store.items.count) { _, _ in
                if let last = store.items.last {
                    withAnimation(.easeOut(duration: 0.2)) {
                        proxy.scrollTo(last.id, anchor: .bottom)
                    }
                }
            }
        }
    }

    private func noticeCard(text: String, kind: String) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: kind == "note" ? "info.circle" : "exclamationmark.circle")
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textSecondary)
            Text(text)
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textSecondary)
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
    }

    // MARK: - Composer

    private var composerBar: some View {
        VStack(spacing: 8) {
            TextField("Ask Antigravity…", text: $store.draftText, axis: .vertical)
                .lineLimit(1...6)
                .font(.system(size: 15))
                .foregroundStyle(OATheme.textPrimary)
                .focused($composerFocused)
                .padding(.horizontal, 12)
                .padding(.top, 10)
                .accessibilityIdentifier("chat.composer")

            HStack(spacing: 6) {
                // Antigravity Model Picker
                Menu {
                    ForEach(store.availableModels) { model in
                        Button {
                            store.chooseModel(model.id)
                        } label: {
                            if model.id == store.selectedModelID {
                                Label(model.label, systemImage: "checkmark")
                            } else {
                                Text(model.label)
                            }
                        }
                    }
                } label: {
                    chipLabel(icon: nil, text: store.selectedModelOption.label, showMark: true)
                }
                .accessibilityIdentifier("chat.modelPicker")

                // Thinking Effort Picker
                Menu {
                    ForEach(store.supportedEffortsForSelectedModel) { effort in
                        Button {
                            store.chooseEffort(effort)
                        } label: {
                            if effort == store.selectedEffort {
                                Label(effort.label, systemImage: "checkmark")
                            } else {
                                Text(effort.label)
                            }
                        }
                    }
                } label: {
                    chipLabel(icon: "sparkles", text: store.selectedEffort.label, showMark: false)
                }
                .accessibilityIdentifier("chat.effortPicker")

                // Review Mode Chip (Auto / Ask / Plan)
                Menu {
                    ForEach(ReviewMode.allCases) { mode in
                        Button {
                            store.chooseReviewMode(mode)
                        } label: {
                            if mode == store.reviewMode {
                                Label("\(mode.label) — \(mode.description)", systemImage: "checkmark")
                            } else {
                                Text("\(mode.label) — \(mode.description)")
                            }
                        }
                    }
                } label: {
                    chipLabel(icon: store.reviewMode.icon, text: store.reviewMode.label, showMark: false)
                }
                .accessibilityIdentifier("chat.reviewMode")

                Spacer(minLength: 4)

                actionButton
            }
            .padding(.horizontal, 10)
            .padding(.bottom, 10)
        }
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(OATheme.background)
    }

    private func chipLabel(icon: String?, text: String, showMark: Bool) -> some View {
        HStack(spacing: 4) {
            if showMark {
                AntigravityMark(size: 10, color: OATheme.textSecondary)
            } else if let icon {
                Image(systemName: icon)
                    .font(.system(size: 10, weight: .medium))
                    .foregroundStyle(OATheme.textSecondary)
            }
            Text(text)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(OATheme.textSecondary)
                .lineLimit(1)
        }
        .padding(.horizontal, 9)
        .padding(.vertical, 5)
        .background(OATheme.elevated, in: Capsule())
        .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
    }

    @ViewBuilder
    private var actionButton: some View {
        let hasText = !store.draftText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        let isRunning = store.activeSession?.busy == true

        if isRunning && !hasText {
            Button {
                Task { await store.stopActiveTurn() }
            } label: {
                Image(systemName: "stop.fill")
                    .font(.system(size: 12, weight: .bold))
                    .foregroundStyle(Color.black)
                    .frame(width: 30, height: 30)
                    .background(Color.white, in: Circle())
            }
            .accessibilityIdentifier("chat.stop")
            .accessibilityLabel("Stop turn")
        } else {
            Button {
                composerFocused = false
                Task { await store.sendMessage() }
            } label: {
                Group {
                    if store.isSending {
                        ProgressView()
                            .controlSize(.mini)
                            .tint(Color.black)
                    } else {
                        Image(systemName: "arrow.up")
                            .font(.system(size: 13, weight: .bold))
                            .foregroundStyle(hasText ? Color.black : OATheme.textMuted)
                    }
                }
                .frame(width: 30, height: 30)
                .background(hasText ? Color.white : OATheme.elevated, in: Circle())
            }
            .disabled(!hasText || store.isSending)
            .accessibilityIdentifier("chat.send")
            .accessibilityLabel(isRunning ? "Steer Antigravity" : "Send message")
        }
    }
}

// MARK: - Transcript Row

private struct TranscriptRowView: View {
    let item: TranscriptItem
    @State private var expanded = false

    var body: some View {
        switch item.role {
        case .user:
            HStack {
                Spacer(minLength: 48)
                Text(item.text)
                    .font(.system(size: 15))
                    .foregroundStyle(OATheme.textPrimary)
                    .textSelection(.enabled)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 10)
                    .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
            }

        case .assistant:
            VStack(alignment: .leading, spacing: 8) {
                MarkdownContentView(text: item.text)
            }
            .frame(maxWidth: .infinity, alignment: .leading)

        case .thought:
            VStack(alignment: .leading, spacing: 6) {
                Button {
                    expanded.toggle()
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: expanded || !item.complete ? "chevron.down" : "chevron.right")
                            .font(.system(size: 10, weight: .semibold))
                        Text(item.complete ? "Thought process" : "Thinking…")
                            .font(.system(size: 12, weight: .medium))
                        Spacer()
                    }
                    .foregroundStyle(OATheme.textMuted)
                }
                .buttonStyle(.plain)

                if expanded || !item.complete {
                    Text(item.text)
                        .font(.system(size: 13))
                        .foregroundStyle(OATheme.textSecondary)
                        .textSelection(.enabled)
                        .padding(.leading, 10)
                        .overlay(alignment: .leading) {
                            Rectangle().fill(OATheme.hairline).frame(width: 2)
                        }
                }
            }

        case .tool:
            VStack(alignment: .leading, spacing: 6) {
                Button {
                    expanded.toggle()
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: toolIcon)
                            .font(.system(size: 12))
                            .foregroundStyle(OATheme.textSecondary)
                        Text(item.toolName.isEmpty ? "Tool" : item.toolName)
                            .font(.system(size: 12, weight: .semibold, design: .monospaced))
                            .foregroundStyle(OATheme.textPrimary)
                        if !item.path.isEmpty {
                            Text(item.path)
                                .font(.system(size: 11, design: .monospaced))
                                .foregroundStyle(OATheme.textSecondary)
                                .lineLimit(1)
                                .truncationMode(.middle)
                        } else if !item.text.isEmpty {
                            Text(item.text.replacingOccurrences(of: "\n", with: " "))
                                .font(.system(size: 12))
                                .foregroundStyle(OATheme.textSecondary)
                                .lineLimit(1)
                        }
                        Spacer()
                        if item.subagentCalls > 0 {
                            Text("\(item.subagentCalls) calls")
                                .font(.system(size: 11, design: .monospaced))
                                .foregroundStyle(OATheme.textMuted)
                        }
                        Image(systemName: expanded ? "chevron.up" : "chevron.down")
                            .font(.system(size: 10))
                            .foregroundStyle(OATheme.textMuted)
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 8)
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                }
                .buttonStyle(.plain)

                if expanded {
                    if !item.diff.isEmpty {
                        DiffBlockView(path: item.path, diff: item.diff, truncated: item.diffCut)
                    } else if !item.text.isEmpty {
                        ScrollView(.horizontal, showsIndicators: false) {
                            Text(item.text)
                                .font(.system(size: 12, design: .monospaced))
                                .foregroundStyle(OATheme.textSecondary)
                                .textSelection(.enabled)
                                .padding(10)
                        }
                        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                    }
                }
            }

        case .notice, .unknown:
            Text(item.text)
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textSecondary)
                .padding(10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        }
    }

    private var toolIcon: String {
        let lower = item.toolName.lowercased()
        if lower.contains("bash") || lower.contains("command") || lower.contains("shell") { return "terminal" }
        if lower.contains("edit") || lower.contains("write") { return "doc.badge.gearshape" }
        if lower.contains("read") || lower.contains("view") { return "doc.text" }
        if lower.contains("search") || lower.contains("grep") || lower.contains("find") { return "magnifyingglass" }
        return "wrench"
    }
}

// MARK: - Markdown + Code Blocks

private struct MarkdownContentView: View {
    let text: String

    private enum Segment: Identifiable {
        case prose(id: Int, content: String)
        case code(id: Int, language: String, content: String)

        var id: Int {
            switch self {
            case .prose(let id, _), .code(let id, _, _): return id
            }
        }
    }

    private var segments: [Segment] {
        var result: [Segment] = []
        let lines = text.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        var proseBuf: [String] = []
        var codeBuf: [String] = []
        var inCode = false
        var codeLang = ""
        var nextID = 0

        func flushProse() {
            let joined = proseBuf.joined(separator: "\n").trimmingCharacters(in: .newlines)
            if !joined.isEmpty {
                result.append(.prose(id: nextID, content: joined))
                nextID += 1
            }
            proseBuf.removeAll()
        }

        for line in lines {
            if line.hasPrefix("```") {
                if inCode {
                    result.append(.code(id: nextID, language: codeLang, content: codeBuf.joined(separator: "\n")))
                    nextID += 1
                    codeBuf.removeAll()
                    codeLang = ""
                    inCode = false
                } else {
                    flushProse()
                    codeLang = String(line.dropFirst(3)).trimmingCharacters(in: .whitespaces)
                    inCode = true
                }
            } else if inCode {
                codeBuf.append(line)
            } else {
                proseBuf.append(line)
            }
        }
        if inCode {
            result.append(.code(id: nextID, language: codeLang, content: codeBuf.joined(separator: "\n")))
        } else {
            flushProse()
        }
        return result
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            ForEach(segments) { seg in
                switch seg {
                case .prose(_, let content):
                    if let attributed = try? AttributedString(
                        markdown: content,
                        options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
                    ) {
                        Text(attributed)
                            .font(.system(size: 15))
                            .foregroundStyle(OATheme.textPrimary)
                            .textSelection(.enabled)
                    } else {
                        Text(content)
                            .font(.system(size: 15))
                            .foregroundStyle(OATheme.textPrimary)
                            .textSelection(.enabled)
                    }
                case .code(_, let language, let code):
                    CodeBlockView(language: language, code: code)
                }
            }
        }
    }
}

private struct CodeBlockView: View {
    let language: String
    let code: String
    @State private var copied = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text(language.isEmpty ? "code" : language)
                    .font(.system(size: 11, weight: .medium, design: .monospaced))
                    .foregroundStyle(OATheme.textMuted)
                Spacer()
                Button {
                    UIPasteboard.general.string = code
                    copied = true
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { copied = false }
                } label: {
                    Label(copied ? "Copied" : "Copy", systemImage: copied ? "checkmark" : "doc.on.doc")
                        .font(.system(size: 11, weight: .medium))
                        .foregroundStyle(OATheme.textSecondary)
                }
                .buttonStyle(.plain)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(OATheme.elevated)

            Divider().overlay(OATheme.hairline)

            ScrollView(.horizontal, showsIndicators: false) {
                Text(code)
                    .font(.system(size: 12.5, design: .monospaced))
                    .foregroundStyle(OATheme.textPrimary)
                    .textSelection(.enabled)
                    .padding(10)
            }
        }
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

private struct DiffBlockView: View {
    let path: String
    let diff: String
    let truncated: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            if !path.isEmpty {
                Text(path)
                    .font(.system(size: 11, weight: .semibold, design: .monospaced))
                    .foregroundStyle(OATheme.textSecondary)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 6)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(OATheme.elevated)
                Divider().overlay(OATheme.hairline)
            }
            ScrollView(.horizontal, showsIndicators: false) {
                VStack(alignment: .leading, spacing: 1) {
                    ForEach(Array(diff.split(separator: "\n", omittingEmptySubsequences: false).enumerated()), id: \.offset) { _, rawLine in
                        let line = String(rawLine)
                        Text(line.isEmpty ? " " : line)
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundStyle(diffTextColor(line))
                            .padding(.horizontal, 8)
                            .padding(.vertical, 1)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .background(diffBgColor(line))
                    }
                    if truncated {
                        Text("… diff truncated by wire budget …")
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundStyle(OATheme.textMuted)
                            .padding(8)
                    }
                }
            }
        }
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
    }

    private func diffTextColor(_ line: String) -> Color {
        if line.hasPrefix("+") { return OATheme.diffAddText }
        if line.hasPrefix("-") { return OATheme.diffDelText }
        return OATheme.textSecondary
    }

    private func diffBgColor(_ line: String) -> Color {
        if line.hasPrefix("+") { return OATheme.diffAddBg }
        if line.hasPrefix("-") { return OATheme.diffDelBg }
        return Color.clear
    }
}
