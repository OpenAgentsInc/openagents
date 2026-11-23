// Firm reason: 6 (cross-Apple SwiftUI pure-black OLED transcript rows, Markdown/code/diff blocks, subagent inspector trigger, and interactive approval banner shared by OpenAgents iOS and OpenAgents macOS).
import SwiftUI

struct TranscriptRowView: View {
    let item: TranscriptItem
    var onOpenSubagent: ((TranscriptItem) -> Void)? = nil
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
                HStack(spacing: 8) {
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
                            } else if !item.subagentNow.isEmpty {
                                Text(item.subagentNow)
                                    .font(.system(size: 12))
                                    .foregroundStyle(OATheme.textSecondary)
                                    .lineLimit(1)
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
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)

                    if item.isSubagent, let onOpenSubagent {
                        Button {
                            onOpenSubagent(item)
                        } label: {
                            HStack(spacing: 4) {
                                Image(systemName: "arrow.up.right.square")
                                    .font(.system(size: 11))
                                Text("Inspect")
                                    .font(.system(size: 11, weight: .semibold))
                            }
                            .foregroundStyle(OATheme.textPrimary)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(OATheme.elevated, in: Capsule())
                            .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                        }
                        .buttonStyle(.plain)
                        .accessibilityIdentifier("transcript.inspectSubagent")
                    }
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))

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
        if item.isSubagent { return "person.2.wave.2" }
        let lower = item.toolName.lowercased()
        if lower.contains("bash") || lower.contains("command") || lower.contains("shell") { return "terminal" }
        if lower.contains("edit") || lower.contains("write") { return "doc.badge.gearshape" }
        if lower.contains("read") || lower.contains("view") { return "doc.text" }
        if lower.contains("search") || lower.contains("grep") || lower.contains("find") { return "magnifyingglass" }
        return "wrench"
    }
}

struct ApprovalBannerView: View {
    let interaction: PendingInteraction
    let onRespond: (Bool, String) -> Void

    @State private var note = ""
    @State private var busy = false

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                Image(systemName: "hand.raised.fill")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(OATheme.statusWarn)
                Text("Approval Requested")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(OATheme.textPrimary)
                Spacer()
                Text(interaction.id.prefix(8))
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(OATheme.textMuted)
            }

            Text(interaction.prompt)
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textSecondary)
                .lineLimit(4)

            TextField("Optional note or answer…", text: $note)
                .textFieldStyle(.plain)
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textPrimary)
                .padding(.horizontal, 10)
                .padding(.vertical, 7)
                .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))

            HStack(spacing: 10) {
                Button {
                    busy = true
                    onRespond(true, note)
                } label: {
                    Label("Allow", systemImage: "checkmark")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(Color.black)
                        .padding(.horizontal, 14)
                        .padding(.vertical, 7)
                        .background(Color.white, in: Capsule())
                }
                .buttonStyle(.plain)
                .disabled(busy)
                .accessibilityIdentifier("interaction.allow")

                Button {
                    busy = true
                    onRespond(false, note)
                } label: {
                    Label("Deny", systemImage: "xmark")
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(OATheme.textPrimary)
                        .padding(.horizontal, 14)
                        .padding(.vertical, 7)
                        .background(OATheme.elevated, in: Capsule())
                        .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                }
                .buttonStyle(.plain)
                .disabled(busy)
                .accessibilityIdentifier("interaction.deny")

                Spacer()
            }
        }
        .padding(12)
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(OATheme.statusWarn.opacity(0.45), lineWidth: 1))
    }
}

struct MarkdownContentView: View {
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

struct CodeBlockView: View {
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
                    OAClipboard.copy(code)
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

struct DiffBlockView: View {
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
        if line.hasPrefix("+") && !line.hasPrefix("+++") { return OATheme.diffAddText }
        if line.hasPrefix("-") && !line.hasPrefix("---") { return OATheme.diffDelText }
        if line.hasPrefix("@@") { return OATheme.textPrimary }
        return OATheme.textSecondary
    }

    private func diffBgColor(_ line: String) -> Color {
        if line.hasPrefix("+") && !line.hasPrefix("+++") { return OATheme.diffAddBg }
        if line.hasPrefix("-") && !line.hasPrefix("---") { return OATheme.diffDelBg }
        return Color.clear
    }
}
