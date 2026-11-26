// Firm reason: 6 (cross-Apple SwiftUI pure-black OLED transcript rows, Markdown/code/diff blocks, Desktop-parity streaming thought rows, inline image thumbnails, subagent inspector trigger, and interactive approval banner shared by OpenAgents iOS and OpenAgents macOS).
import CoreGraphics
import ImageIO
import SwiftUI

struct InlineCGImageView: View {
    let data: Data
    var maxWidth: CGFloat = 220
    var maxHeight: CGFloat = 220
    var cornerRadius: CGFloat = 12

    private var cgImage: CGImage? {
        guard let src = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
        return CGImageSourceCreateImageAtIndex(src, 0, nil)
    }

    var body: some View {
        if let cg = cgImage {
            Image(decorative: cg, scale: 1.0)
                .resizable()
                .aspectRatio(contentMode: .fill)
                .frame(maxWidth: maxWidth, maxHeight: maxHeight)
                .clipShape(RoundedRectangle(cornerRadius: cornerRadius, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: cornerRadius, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        }
    }
}

struct StreamingStatusRowView: View {
    let label: String
    @State private var pulse = false

    var body: some View {
        HStack(spacing: 8) {
            Circle()
                .fill(OATheme.textPrimary)
                .frame(width: 6, height: 6)
                .opacity(pulse ? 1.0 : 0.25)
                .animation(.easeInOut(duration: 0.75).repeatForever(autoreverses: true), value: pulse)
            Text(label)
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(OATheme.textSecondary)
        }
        .padding(.top, 4)
        .onAppear { pulse = true }
    }
}

struct TranscriptRowView: View {
    let item: TranscriptItem
    var localImageLookup: ((String) -> Data?)? = nil
    var onOpenSubagent: ((TranscriptItem) -> Void)? = nil
    var onOpenFileLink: ((String, Int?) -> Void)? = nil
    var onEditAndResend: ((TranscriptItem) -> Void)? = nil
    @State private var expanded = false
    @State private var userToggledThought = false
    @State private var thinkingPulse = false

    var body: some View {
        switch item.role {
        case .user:
            let parsed = ParsedUserMessage.parse(item.text)
            HStack {
                Spacer(minLength: 48)
                VStack(alignment: .trailing, spacing: 8) {
                    if !parsed.images.isEmpty {
                        HStack(spacing: 6) {
                            ForEach(parsed.images) { img in
                                if let data = localImageLookup?(img.name) ?? img.data {
                                    InlineCGImageView(data: data, maxWidth: 210, maxHeight: 190, cornerRadius: 12)
                                } else {
                                    HStack(spacing: 5) {
                                        Image(systemName: "photo")
                                            .font(.system(size: 11))
                                        Text(img.name)
                                            .font(.system(size: 11, design: .monospaced))
                                            .lineLimit(1)
                                    }
                                    .foregroundStyle(OATheme.textSecondary)
                                    .padding(.horizontal, 10)
                                    .padding(.vertical, 6)
                                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                                    .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                                }
                            }
                        }
                    }
                    if !parsed.text.isEmpty {
                        Text(parsed.text)
                            .font(.system(size: 15))
                            .foregroundStyle(OATheme.textPrimary)
                            .textSelection(.enabled)
                            .padding(.horizontal, 14)
                            .padding(.vertical, 10)
                            .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
                            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                            .contextMenu {
                                Button {
                                    OAClipboard.copy(parsed.text)
                                } label: {
                                    Label("Copy", systemImage: "doc.on.doc")
                                }
                                if let onEditAndResend {
                                    Button {
                                        onEditAndResend(item)
                                    } label: {
                                        Label("Edit & Resend", systemImage: "arrow.uturn.backward")
                                    }
                                }
                            }
                    }
                }
            }

        case .assistant:
            VStack(alignment: .leading, spacing: 8) {
                MarkdownContentView(text: item.text, onOpenFileLink: onOpenFileLink)
            }
            .frame(maxWidth: .infinity, alignment: .leading)

        case .thought:
            let heading = ThoughtSummaryParser.extractHeading(item.text)
            let isStreaming = !item.complete
            let isOpen = isStreaming ? (userToggledThought ? expanded : true) : expanded
            VStack(alignment: .leading, spacing: 6) {
                Button {
                    userToggledThought = true
                    expanded = !isOpen
                } label: {
                    HStack(spacing: 6) {
                        if isStreaming {
                            Circle()
                                .fill(OATheme.textSecondary)
                                .frame(width: 6, height: 6)
                                .opacity(thinkingPulse ? 1.0 : 0.25)
                                .animation(.easeInOut(duration: 0.7).repeatForever(autoreverses: true), value: thinkingPulse)
                        } else {
                            Image(systemName: isOpen ? "chevron.down" : "chevron.right")
                                .font(.system(size: 10, weight: .semibold))
                        }
                        Text(isStreaming ? "Thinking" : "Thought process")
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(isStreaming ? OATheme.textPrimary : OATheme.textMuted)
                        if !heading.isEmpty {
                            Text("· \(heading)")
                                .font(.system(size: 12))
                                .foregroundStyle(OATheme.textSecondary)
                                .lineLimit(1)
                        }
                        Spacer()
                    }
                    .foregroundStyle(OATheme.textMuted)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("transcript.thoughtHeader")
                .onAppear {
                    if isStreaming { thinkingPulse = true }
                }
                .onChange(of: item.complete) { _, nowComplete in
                    // Match Desktop: collapse cleanly once thinking finishes unless user manually opened it.
                    if nowComplete && !userToggledThought {
                        expanded = false
                    }
                }

                if isOpen {
                    MarkdownContentView(text: item.text, fontSize: 13, textColor: OATheme.textSecondary)
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
                            if item.startedAtMs > 0 && !item.complete {
                                TimelineView(.periodic(from: .now, by: 1.0)) { ctx in
                                    let elapsed = max(0, Int(ctx.date.timeIntervalSince1970) - Int(item.startedAtMs / 1000))
                                    Text(elapsed >= 60 ? "\(elapsed / 60)m \(elapsed % 60)s" : "\(elapsed)s")
                                        .font(.system(size: 11, design: .monospaced))
                                        .foregroundStyle(OATheme.statusWarn)
                                }
                            }
                            if !item.subagentType.isEmpty {
                                Text(item.subagentType)
                                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                                    .foregroundStyle(OATheme.textSecondary)
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(OATheme.elevated, in: Capsule())
                            }
                            if item.isBackground {
                                Text("bg")
                                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                                    .foregroundStyle(OATheme.statusWarn)
                            }
                            if !item.worktreeBranch.isEmpty {
                                Text(item.worktreeBranch)
                                    .font(.system(size: 10, design: .monospaced))
                                    .foregroundStyle(OATheme.textMuted)
                                    .lineLimit(1)
                            }
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
                    VStack(alignment: .leading, spacing: 6) {
                        if !item.diff.isEmpty {
                            DiffBlockView(path: item.path, diff: item.diff, truncated: item.diffCut)
                        }
                        if !item.text.isEmpty {
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
            }

        case .notice, .unknown:
            noticeBannerView
        }
    }

    private var noticeBannerView: some View {
        let style: (icon: String, tint: Color, label: String) = {
            switch item.typedNoticeKind {
            case .install:
                return ("arrow.down.circle.fill", OATheme.statusWarn, "Setup Required")
            case .login:
                return ("person.crop.circle.badge.exclamationmark", OATheme.statusWarn, "Sign-In Required")
            case .limit:
                return ("gauge.with.dots.needle.100percent", OATheme.statusError, "Quota / Rate Limit")
            case .note:
                return ("info.circle.fill", OATheme.textSecondary, "Note")
            case .none, .unknown:
                return ("exclamationmark.circle", OATheme.textSecondary, "")
            }
        }()
        return HStack(alignment: .top, spacing: 8) {
            Image(systemName: style.icon)
                .font(.system(size: 13))
                .foregroundStyle(style.tint)
                .padding(.top, 1)
            VStack(alignment: .leading, spacing: 3) {
                if !style.label.isEmpty {
                    Text(style.label)
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(style.tint)
                }
                Text(item.text)
                    .font(.system(size: 13))
                    .foregroundStyle(OATheme.textPrimary)
                    .textSelection(.enabled)
            }
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(style.tint.opacity(item.typedNoticeKind == .note ? 0.06 : 0.12), in: RoundedRectangle(cornerRadius: 9, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(style.tint.opacity(0.35), lineWidth: 1))
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
    let onRespond: (_ allow: Bool, _ note: String, _ optionID: String) async -> Void

    @State private var note = ""
    @State private var busy = false

    private func triggerResponse(allow: Bool, optionID: String) {
        guard !busy else { return }
        busy = true
        let currentNote = note
        Task {
            await onRespond(allow, currentNote, optionID)
            busy = false
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(spacing: 8) {
                Image(systemName: interaction.isQuestion ? "questionmark.circle.fill" : "hand.raised.fill")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(OATheme.statusWarn)
                Text(interaction.isQuestion ? "Input Requested" : "Approval Requested")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(OATheme.textPrimary)
                Spacer()
                Text(interaction.id.prefix(8))
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(OATheme.textMuted)
            }

            if !interaction.warning.isEmpty {
                HStack(alignment: .top, spacing: 6) {
                    Image(systemName: "exclamationmark.shield.fill")
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.statusError)
                    Text(interaction.warning)
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(OATheme.statusError)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(OATheme.statusError.opacity(0.12), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.statusError.opacity(0.4), lineWidth: 1))
                .accessibilityIdentifier("interaction.warning")
            }

            Text(interaction.prompt)
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textSecondary)
                .lineLimit(6)

            TextField(interaction.isQuestion ? "Type your answer…" : "Optional note or answer…", text: $note)
                .textFieldStyle(.plain)
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textPrimary)
                .padding(.horizontal, 10)
                .padding(.vertical, 7)
                .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))

            if !interaction.options.isEmpty {
                let firstAllowID = interaction.options.first(where: \.isAllow)?.id
                let firstDenyID = interaction.options.first(where: { !$0.isAllow })?.id
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        ForEach(interaction.options) { opt in
                            let isPrimary = opt.id == firstAllowID
                            Button {
                                triggerResponse(allow: opt.isAllow, optionID: opt.id)
                            } label: {
                                Label(opt.label, systemImage: opt.isAllow ? "checkmark" : "xmark")
                                    .font(.system(size: 13, weight: isPrimary ? .semibold : .medium))
                                    .foregroundStyle(isPrimary ? Color.black : OATheme.textPrimary)
                                    .padding(.horizontal, 13)
                                    .padding(.vertical, 7)
                                    .background(isPrimary ? Color.white : OATheme.elevated, in: Capsule())
                                    .overlay(Capsule().stroke(isPrimary ? Color.clear : OATheme.hairline, lineWidth: 1))
                            }
                            .buttonStyle(.plain)
                            .disabled(busy)
                            .accessibilityIdentifier(
                                opt.id == firstAllowID
                                    ? "interaction.allow"
                                    : (opt.id == firstDenyID ? "interaction.deny" : "interaction.option.\(opt.id)")
                            )
                        }
                    }
                }
            } else {
                HStack(spacing: 10) {
                    Button {
                        triggerResponse(allow: true, optionID: "")
                    } label: {
                        Label(interaction.isQuestion ? "Submit" : "Allow", systemImage: "checkmark")
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
                        triggerResponse(allow: false, optionID: "")
                    } label: {
                        Label(interaction.isQuestion ? "Cancel" : "Deny", systemImage: "xmark")
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
        }
        .padding(12)
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(OATheme.statusWarn.opacity(0.45), lineWidth: 1))
        .onChange(of: interaction.id) { _, _ in
            busy = false
            note = ""
        }
    }
}

struct MarkdownContentView: View {
    let text: String
    var fontSize: CGFloat = 15
    var textColor: Color = OATheme.textPrimary
    var onOpenFileLink: ((String, Int?) -> Void)? = nil

    private enum Block: Identifiable {
        case heading(id: Int, level: Int, content: String)
        case paragraph(id: Int, content: String)
        case bullet(id: Int, indent: Int, marker: String, checked: Bool?, content: String)
        case blockquote(id: Int, content: String)
        case rule(id: Int)
        case table(id: Int, headers: [String], rows: [[String]])
        case code(id: Int, language: String, content: String)

        var id: Int {
            switch self {
            case .heading(let id, _, _),
                 .paragraph(let id, _),
                 .bullet(let id, _, _, _, _),
                 .blockquote(let id, _),
                 .rule(let id),
                 .table(let id, _, _),
                 .code(let id, _, _):
                return id
            }
        }
    }

    private var blocks: [Block] {
        var result: [Block] = []
        let lines = text.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
        var paraBuf: [String] = []
        var quoteBuf: [String] = []
        var codeBuf: [String] = []
        var inCode = false
        var codeLang = ""
        var nextID = 0

        func flushPara() {
            let joined = paraBuf.joined(separator: "\n").trimmingCharacters(in: .newlines)
            if !joined.isEmpty {
                result.append(.paragraph(id: nextID, content: joined))
                nextID += 1
            }
            paraBuf.removeAll()
        }
        func flushQuote() {
            let joined = quoteBuf.joined(separator: "\n").trimmingCharacters(in: .newlines)
            if !joined.isEmpty {
                result.append(.blockquote(id: nextID, content: joined))
                nextID += 1
            }
            quoteBuf.removeAll()
        }
        func splitTableRow(_ line: String) -> [String] {
            var s = line.trimmingCharacters(in: .whitespaces)
            if s.hasPrefix("|") { s.removeFirst() }
            if s.hasSuffix("|") { s.removeLast() }
            return s.split(separator: "|", omittingEmptySubsequences: false)
                .map { $0.trimmingCharacters(in: .whitespaces) }
        }
        func isTableSeparator(_ line: String) -> Bool {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard trimmed.contains("|") && trimmed.contains("-") else { return false }
            return trimmed.allSatisfy { $0 == "|" || $0 == "-" || $0 == ":" || $0 == " " }
        }

        var i = 0
        while i < lines.count {
            let line = lines[i]
            let trimmed = line.trimmingCharacters(in: .whitespaces)

            if trimmed.hasPrefix("```") {
                if inCode {
                    result.append(.code(id: nextID, language: codeLang, content: codeBuf.joined(separator: "\n")))
                    nextID += 1
                    codeBuf.removeAll()
                    codeLang = ""
                    inCode = false
                } else {
                    flushPara(); flushQuote()
                    codeLang = String(trimmed.dropFirst(3)).trimmingCharacters(in: .whitespaces)
                    inCode = true
                }
                i += 1
                continue
            }
            if inCode {
                codeBuf.append(line)
                i += 1
                continue
            }

            if trimmed.isEmpty {
                flushPara(); flushQuote()
                i += 1
                continue
            }

            // Horizontal rule
            if (trimmed.count >= 3) && (trimmed.allSatisfy({ $0 == "-" || $0 == " " }) && trimmed.filter({ $0 == "-" }).count >= 3
                || trimmed.allSatisfy({ $0 == "*" || $0 == " " }) && trimmed.filter({ $0 == "*" }).count >= 3
                || trimmed.allSatisfy({ $0 == "_" || $0 == " " }) && trimmed.filter({ $0 == "_" }).count >= 3) {
                flushPara(); flushQuote()
                result.append(.rule(id: nextID)); nextID += 1
                i += 1
                continue
            }

            // Headings H1-H4
            if trimmed.hasPrefix("#") {
                var lvl = 0
                for ch in trimmed { if ch == "#" { lvl += 1 } else { break } }
                if lvl >= 1 && lvl <= 4 && trimmed.count > lvl && trimmed[trimmed.index(trimmed.startIndex, offsetBy: lvl)] == " " {
                    flushPara(); flushQuote()
                    let body = String(trimmed.dropFirst(lvl + 1)).trimmingCharacters(in: .whitespaces)
                    result.append(.heading(id: nextID, level: lvl, content: body)); nextID += 1
                    i += 1
                    continue
                }
            }

            // GFM Table
            if trimmed.contains("|") && i + 1 < lines.count && isTableSeparator(lines[i + 1]) {
                flushPara(); flushQuote()
                let headers = splitTableRow(trimmed)
                var rows: [[String]] = []
                i += 2
                while i < lines.count {
                    let rl = lines[i].trimmingCharacters(in: .whitespaces)
                    if rl.isEmpty || !rl.contains("|") { break }
                    rows.append(splitTableRow(rl))
                    i += 1
                }
                result.append(.table(id: nextID, headers: headers, rows: rows)); nextID += 1
                continue
            }

            // Blockquote
            if trimmed.hasPrefix(">") {
                flushPara()
                var qBody = String(trimmed.dropFirst(1))
                if qBody.hasPrefix(" ") { qBody.removeFirst() }
                quoteBuf.append(qBody)
                i += 1
                continue
            } else {
                flushQuote()
            }

            // Unordered / Task / Numbered list item
            let leadingSpaces = line.prefix(while: { $0 == " " }).count
            let indent = min(leadingSpaces / 2, 3)
            if trimmed.hasPrefix("- [ ] ") || trimmed.hasPrefix("- [x] ") || trimmed.hasPrefix("- [X] ") {
                flushPara()
                let checked = trimmed.hasPrefix("- [x] ") || trimmed.hasPrefix("- [X] ")
                result.append(.bullet(id: nextID, indent: indent, marker: "", checked: checked, content: String(trimmed.dropFirst(6))))
                nextID += 1
                i += 1
                continue
            }
            if trimmed.hasPrefix("- ") || trimmed.hasPrefix("* ") {
                flushPara()
                result.append(.bullet(id: nextID, indent: indent, marker: "•", checked: nil, content: String(trimmed.dropFirst(2))))
                nextID += 1
                i += 1
                continue
            }
            if let dotIdx = trimmed.firstIndex(of: "."), dotIdx > trimmed.startIndex,
               trimmed[trimmed.startIndex..<dotIdx].allSatisfy(\.isNumber) {
                let afterDot = trimmed.index(after: dotIdx)
                if afterDot < trimmed.endIndex && trimmed[afterDot] == " " {
                    flushPara()
                    let num = String(trimmed[trimmed.startIndex...dotIdx])
                    let rest = String(trimmed[trimmed.index(after: afterDot)...])
                    result.append(.bullet(id: nextID, indent: indent, marker: num, checked: nil, content: rest))
                    nextID += 1
                    i += 1
                    continue
                }
            }

            paraBuf.append(line)
            i += 1
        }
        if inCode {
            result.append(.code(id: nextID, language: codeLang, content: codeBuf.joined(separator: "\n")))
        } else {
            flushPara(); flushQuote()
        }
        return result
    }

    private func renderInline(_ raw: String, size: CGFloat, color: Color, weight: Font.Weight = .regular) -> some View {
        // Format inline LaTeX math spans cleanly in readable Unicode/monospaced style while preserving GFM inline markdown.
        let cleaned = raw
            .replacingOccurrences(of: "\\(", with: "`")
            .replacingOccurrences(of: "\\)", with: "`")
            .replacingOccurrences(of: "\\[", with: "`")
            .replacingOccurrences(of: "\\]", with: "`")
        return Group {
            if let attributed = try? AttributedString(
                markdown: cleaned,
                options: .init(interpretedSyntax: .inlineOnlyPreservingWhitespace)
            ) {
                Text(attributed)
                    .font(.system(size: size, weight: weight))
                    .foregroundStyle(color)
                    .textSelection(.enabled)
            } else {
                Text(cleaned)
                    .font(.system(size: size, weight: weight))
                    .foregroundStyle(color)
                    .textSelection(.enabled)
            }
        }
        .environment(\.openURL, OpenURLAction { url in
            if url.scheme == "file", let onOpenFileLink {
                let lineNum: Int? = url.fragment.flatMap { frag in
                    let digits = frag.drop(while: { !$0.isNumber }).prefix(while: \.isNumber)
                    return Int(digits)
                }
                onOpenFileLink(url.path, lineNum)
                return .handled
            }
            return .systemAction
        })
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(blocks) { block in
                switch block {
                case .heading(_, let level, let content):
                    let hSize: CGFloat = level == 1 ? fontSize + 5 : (level == 2 ? fontSize + 3 : (level == 3 ? fontSize + 1.5 : fontSize))
                    renderInline(content, size: hSize, color: OATheme.textPrimary, weight: .semibold)
                        .padding(.top, level <= 2 ? 4 : 2)

                case .paragraph(_, let content):
                    renderInline(content, size: fontSize, color: textColor)

                case .bullet(_, let indent, let marker, let checked, let content):
                    HStack(alignment: .top, spacing: 7) {
                        if let checked {
                            Image(systemName: checked ? "checkmark.square.fill" : "square")
                                .font(.system(size: max(12, fontSize - 2)))
                                .foregroundStyle(checked ? OATheme.statusReady : OATheme.textMuted)
                                .padding(.top, 2)
                        } else {
                            Text(marker)
                                .font(.system(size: fontSize, weight: .semibold))
                                .foregroundStyle(OATheme.textMuted)
                                .frame(minWidth: marker == "•" ? 10 : 18, alignment: .trailing)
                        }
                        renderInline(content, size: fontSize, color: textColor)
                            .frame(maxWidth: .infinity, alignment: .leading)
                    }
                    .padding(.leading, CGFloat(indent) * 14)

                case .blockquote(_, let content):
                    renderInline(content, size: max(13, fontSize - 0.5), color: OATheme.textSecondary)
                        .padding(.leading, 10)
                        .padding(.vertical, 3)
                        .overlay(alignment: .leading) {
                            Rectangle().fill(OATheme.textMuted.opacity(0.6)).frame(width: 2.5)
                        }

                case .rule:
                    Divider().overlay(OATheme.hairline).padding(.vertical, 2)

                case .table(_, let headers, let rows):
                    ScrollView(.horizontal, showsIndicators: false) {
                        VStack(alignment: .leading, spacing: 0) {
                            HStack(spacing: 0) {
                                ForEach(Array(headers.enumerated()), id: \.offset) { _, h in
                                    renderInline(h, size: max(12, fontSize - 2), color: OATheme.textPrimary, weight: .semibold)
                                        .padding(.horizontal, 10)
                                        .padding(.vertical, 6)
                                        .frame(minWidth: 96, alignment: .leading)
                                }
                            }
                            .background(OATheme.elevated)
                            Divider().overlay(OATheme.hairline)
                            ForEach(Array(rows.enumerated()), id: \.offset) { rIdx, row in
                                HStack(spacing: 0) {
                                    ForEach(0..<max(headers.count, row.count), id: \.self) { cIdx in
                                        let cell = cIdx < row.count ? row[cIdx] : ""
                                        renderInline(cell, size: max(12, fontSize - 2), color: textColor)
                                            .padding(.horizontal, 10)
                                            .padding(.vertical, 6)
                                            .frame(minWidth: 96, alignment: .leading)
                                    }
                                }
                                .background(rIdx % 2 == 1 ? OATheme.surface.opacity(0.6) : Color.clear)
                                if rIdx + 1 < rows.count {
                                    Divider().overlay(OATheme.hairline.opacity(0.5))
                                }
                            }
                        }
                    }
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                    .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))

                case .code(_, let language, let code):
                    CodeBlockView(language: language, code: code)
                }
            }
        }
    }
}

/// Pure-Swift lightweight syntax highlighter (ZERO WebKit/JS; supports Swift, Coax, Python, JS/TS, JSON, Bash, Rust, Go).
enum CodeSyntaxHighlighter {
    private static let keywords: Set<String> = [
        "func", "struct", "class", "enum", "protocol", "let", "var", "if", "else", "for", "while",
        "switch", "case", "default", "return", "guard", "import", "private", "public", "static",
        "async", "await", "try", "catch", "throw", "throws", "true", "false", "nil", "null", "None",
        "def", "from", "as", "with", "yield", "lambda", "const", "function", "export", "type",
        "interface", "fn", "pub", "impl", "match", "mut", "use", "mod", "defer", "cast", "ifx", "then"
    ]

    static func highlight(_ code: String, language: String) -> AttributedString {
        var result = AttributedString()
        let lines = code.split(separator: "\n", omittingEmptySubsequences: false)
        for (lineIdx, rawLine) in lines.enumerated() {
            let line = String(rawLine)
            if lineIdx > 0 { result.append(AttributedString("\n")) }
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            if trimmed.hasPrefix("//") || trimmed.hasPrefix("#") || trimmed.hasPrefix("--") {
                var c = AttributedString(line)
                c.foregroundColor = Color(red: 0.48, green: 0.54, blue: 0.60)
                result.append(c)
                continue
            }
            var idx = line.startIndex
            while idx < line.endIndex {
                let ch = line[idx]
                if ch == "\"" || ch == "'" {
                    let quote = ch
                    var end = line.index(after: idx)
                    var esc = false
                    while end < line.endIndex {
                        let c = line[end]
                        if esc { esc = false }
                        else if c == "\\" { esc = true }
                        else if c == quote { end = line.index(after: end); break }
                        end = line.index(after: end)
                    }
                    var seg = AttributedString(String(line[idx..<end]))
                    seg.foregroundColor = Color(red: 0.56, green: 0.86, blue: 0.62)
                    result.append(seg)
                    idx = end
                } else if ch.isLetter || ch == "_" {
                    var end = idx
                    while end < line.endIndex && (line[end].isLetter || line[end].isNumber || line[end] == "_") {
                        end = line.index(after: end)
                    }
                    let word = String(line[idx..<end])
                    var seg = AttributedString(word)
                    if keywords.contains(word) {
                        seg.foregroundColor = Color(red: 0.78, green: 0.58, blue: 0.96)
                    } else if word.first?.isUppercase == true {
                        seg.foregroundColor = Color(red: 0.50, green: 0.78, blue: 0.98)
                    } else {
                        seg.foregroundColor = OATheme.textPrimary
                    }
                    result.append(seg)
                    idx = end
                } else if ch.isNumber {
                    var end = idx
                    while end < line.endIndex && (line[end].isNumber || line[end] == "." || line[end] == "x" || line[end] == "_") {
                        end = line.index(after: end)
                    }
                    var seg = AttributedString(String(line[idx..<end]))
                    seg.foregroundColor = Color(red: 0.96, green: 0.76, blue: 0.44)
                    result.append(seg)
                    idx = end
                } else {
                    var seg = AttributedString(String(ch))
                    seg.foregroundColor = OATheme.textSecondary
                    result.append(seg)
                    idx = line.index(after: idx)
                }
            }
        }
        return result
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
                Text(code.count <= 16_000 ? CodeSyntaxHighlighter.highlight(code, language: language) : AttributedString(code))
                    .font(.system(size: 12.5, design: .monospaced))
                    .textSelection(.enabled)
                    .padding(10)
            }
        }
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

/// Live ACP Plan Checklist + Context Occupancy / Token Usage Strip (#14010, #14013).
struct SessionTodosAndUsageStrip: View {
    let todos: [SessionTodoItem]
    let usage: SessionUsageStats?
    let onCompact: () -> Void
    @State private var todosExpanded = false

    var body: some View {
        if !todos.isEmpty || usage != nil {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    if !todos.isEmpty {
                        let doneCount = todos.filter(\.isCompleted).count
                        Button {
                            todosExpanded.toggle()
                        } label: {
                            HStack(spacing: 5) {
                                Image(systemName: "checklist")
                                    .font(.system(size: 11))
                                Text("Plan \(doneCount)/\(todos.count)")
                                    .font(.system(size: 11, weight: .semibold, design: .monospaced))
                                Image(systemName: todosExpanded ? "chevron.up" : "chevron.down")
                                    .font(.system(size: 9))
                            }
                            .foregroundStyle(OATheme.textPrimary)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(OATheme.elevated, in: Capsule())
                            .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                        }
                        .buttonStyle(.plain)
                    }

                    if let usage {
                        HStack(spacing: 5) {
                            Image(systemName: "gauge.with.dots.needle.33percent")
                                .font(.system(size: 10))
                            Text(usage.summaryLabel)
                                .font(.system(size: 10.5, design: .monospaced))
                            if usage.contextPercent >= 1.0 {
                                Text("(\(Int(usage.contextPercent))%)")
                                    .font(.system(size: 10.5, weight: .semibold, design: .monospaced))
                                    .foregroundStyle(usage.contextPercent > 75 ? OATheme.statusWarn : OATheme.textMuted)
                            }
                        }
                        .foregroundStyle(OATheme.textSecondary)
                        .lineLimit(1)
                    }

                    Spacer()

                    if usage != nil {
                        Button(action: onCompact) {
                            Text("/compact")
                                .font(.system(size: 10.5, weight: .medium, design: .monospaced))
                                .foregroundStyle(OATheme.textSecondary)
                                .padding(.horizontal, 7)
                                .padding(.vertical, 3)
                                .background(OATheme.elevated, in: Capsule())
                                .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                        }
                        .buttonStyle(.plain)
                    }
                }

                if todosExpanded && !todos.isEmpty {
                    VStack(alignment: .leading, spacing: 4) {
                        ForEach(todos) { todo in
                            HStack(alignment: .top, spacing: 6) {
                                Image(systemName: todo.isCompleted ? "checkmark.circle.fill" : (todo.isInProgress ? "circle.inset.filled" : "circle"))
                                    .font(.system(size: 11))
                                    .foregroundStyle(todo.isCompleted ? OATheme.statusReady : (todo.isInProgress ? OATheme.statusWarn : OATheme.textMuted))
                                    .padding(.top, 1)
                                Text(todo.content)
                                    .font(.system(size: 12))
                                    .foregroundStyle(todo.isCompleted ? OATheme.textMuted : OATheme.textPrimary)
                                    .strikethrough(todo.isCompleted)
                            }
                        }
                    }
                    .padding(8)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(OATheme.background)
        }
    }
}

/// `/btw` read-only parallel side question sheet with Promote to Conversation (#14013).
struct SideQuestionSheetView: View {
    @ObservedObject var store: AntigravityStore
    @Environment(\.dismiss) private var dismiss
    @State private var prompt = ""

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 12) {
                        if store.sideQuestions.isEmpty {
                            Text("Ask a quick side question about this workspace or conversation without adding it to your main chat history.")
                                .font(.system(size: 13))
                                .foregroundStyle(OATheme.textSecondary)
                                .padding(14)
                        } else {
                            ForEach(store.sideQuestions) { entry in
                                VStack(alignment: .leading, spacing: 8) {
                                    Text("/btw \(entry.question)")
                                        .font(.system(size: 13, weight: .semibold))
                                        .foregroundStyle(OATheme.textPrimary)
                                    if entry.isLoading {
                                        HStack(spacing: 8) {
                                            ProgressView().controlSize(.small)
                                            Text("Thinking in side thread…")
                                                .font(.system(size: 12))
                                                .foregroundStyle(OATheme.textSecondary)
                                        }
                                    } else {
                                        MarkdownContentView(text: entry.answer, fontSize: 13.5)
                                        HStack {
                                            Spacer()
                                            Button {
                                                store.promoteSideQuestionToConversation(entry)
                                                dismiss()
                                            } label: {
                                                Label("Promote to Conversation", systemImage: "arrow.up.right.circle")
                                                    .font(.system(size: 12, weight: .medium))
                                                    .foregroundStyle(OATheme.textPrimary)
                                                    .padding(.horizontal, 10)
                                                    .padding(.vertical, 5)
                                                    .background(OATheme.elevated, in: Capsule())
                                                    .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                                            }
                                            .buttonStyle(.plain)
                                        }
                                    }
                                }
                                .padding(12)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                                .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                            }
                        }
                    }
                    .padding(14)
                }

                Divider().overlay(OATheme.hairline)

                HStack(spacing: 8) {
                    TextField("Ask a side question…", text: $prompt)
                        .textFieldStyle(.plain)
                        .font(.system(size: 14))
                        .foregroundStyle(OATheme.textPrimary)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 8)
                        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                        .onSubmit {
                            let q = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
                            guard !q.isEmpty else { return }
                            prompt = ""
                            Task { await store.askSideQuestion(q) }
                        }

                    Button {
                        let q = prompt.trimmingCharacters(in: .whitespacesAndNewlines)
                        guard !q.isEmpty else { return }
                        prompt = ""
                        Task { await store.askSideQuestion(q) }
                    } label: {
                        Image(systemName: "arrow.up")
                            .font(.system(size: 13, weight: .bold))
                            .foregroundStyle(Color.black)
                            .frame(width: 32, height: 32)
                            .background(Color.white, in: Circle())
                    }
                    .buttonStyle(.plain)
                    .disabled(prompt.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
                .padding(12)
                .background(OATheme.background)
            }
            .background(OATheme.background)
            .navigationTitle("/btw Side Question")
            #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            #endif
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Close") { dismiss() }
                }
            }
        }
        #if os(macOS)
        .frame(minWidth: 460, minHeight: 420)
        #endif
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
