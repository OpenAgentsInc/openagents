import SwiftUI
import OpenAgentsCore

struct AcpThreadView: View {
    let url: URL?
    var initialLines: [String]? = nil
    let maxMessages: Int = 200
    var onTitleChange: ((String) -> Void)? = nil

    @EnvironmentObject var bridge: BridgeManager
    @State private var isLoading = false
    @State private var error: String? = nil
    
    enum TimelineItem: Identifiable {
        case message(ACPMessage)
        case reasoning(ACPMessage) // legacy, not used in render
        case reasoningSummary(ReasoningSummary)
        case toolCall(ACPToolCall)
        case toolResult(ACPToolResult)
        case plan(ACPPlanState)
        case raw(String)

        var id: String {
            switch self {
            case .message(let m):
                let text = m.parts.compactMap { part -> String? in
                    if case let .text(t) = part { return t.text } else { return nil }
                }.joined()
                return "msg_\(m.id)_\(m.ts)_\(text.hashValue)"
            case .reasoning(let m):
                let text = m.parts.compactMap { part -> String? in
                    if case let .text(t) = part { return t.text } else { return nil }
                }.joined()
                return "reason_\(m.id)_\(m.ts)_\(text.hashValue)"
            case .toolCall(let c): return "call_\(c.id)"
            case .toolResult(let r): return "res_\(r.call_id)\(r.ts ?? 0)"
            case .reasoningSummary(let rs): return "rs_\(rs.startTs)_\(rs.endTs)\(rs.messages.count)"
            case .plan(let p): return "plan_\(p.ts ?? 0)"
            case .raw(let s): return "raw_\(s.hashValue)"
            }
        }
        var ts: Int64 {
            switch self {
            case .message(let m): return m.ts
            case .reasoning(let m): return m.ts
            case .reasoningSummary(let rs): return rs.endTs
            case .toolCall(let c): return c.ts ?? 0
            case .toolResult(let r): return r.ts ?? 0
            case .plan(let p): return p.ts ?? 0
            case .raw: return 0
            }
        }
    }
    struct ReasoningSummary {
        let startTs: Int64
        let endTs: Int64
        let messages: [ACPMessage]
    }

    @State private var timeline: [TimelineItem] = []
    @State private var threadTitle: String? = nil
    // Track recent texts to avoid duplicates across non-consecutive items
    @State private var seenAssistantMessageTexts: Set<String> = []
    @State private var seenUserMessageTexts: Set<String> = []
    @State private var seenReasoningTexts: Set<String> = []
    @State private var pendingReasoning: [ACPMessage] = []
    @State private var pendingReasoningStart: Int64? = nil
    @State private var reasoningSheet: [ACPMessage]? = nil

    var body: some View {
        ZStack {
            if (url == nil && timeline.isEmpty) || (isLoading && timeline.isEmpty) {
                loadingView
            } else if let e = error, timeline.isEmpty {
                errorView(e)
            } else {
                messagesView
            }
        }
        .background(OATheme.Colors.background)
        #if os(iOS)
        .onAppear { /* no-op; List removed */ }
        #endif
        .onChange(of: url?.path) { _, _ in load() }
        .onAppear(perform: load)
        #if os(iOS)
        // When latestLines arrive from the bridge, build the initial timeline and scroll to bottom.
        .onChange(of: bridge.latestLines.count) { _, _ in
            if timeline.isEmpty, !bridge.latestLines.isEmpty {
                let (items, title) = AcpThreadView_computeTimeline(lines: bridge.latestLines, sourceId: "remote", cap: 100)
                timeline = items
                threadTitle = title
                if let t = title, !t.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { onTitleChange?(t) }
                isLoading = false
            }
        }
        .onChange(of: bridge.updates.count) { _, _ in
            if let last = bridge.updates.last {
                appendUpdate(last)
            }
        }
        #endif
    }

    // MARK: - Subviews to reduce type-check complexity
    private var loadingView: some View {
        VStack(spacing: 10) {
            ProgressView()
                    Text(statusText())
                        .font(OAFonts.ui(.caption, 12))
                .foregroundStyle(OATheme.Colors.textSecondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private func errorView(_ e: String) -> some View {
        VStack(spacing: 8) {
            Text(e)
                .font(.footnote)
                .foregroundStyle(OATheme.Colors.textSecondary)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var messagesView: some View {
        ZStack(alignment: .bottom) {
            ScrollViewReader { proxy in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 8) {
                if let title = threadTitle, !title.isEmpty {
                    Text(title)
                        .font(OAFonts.ui(.headline, 17))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                }
                        ForEach(timeline) { item in
                            messageRow(for: item)
                                .padding(.vertical, 4)
                        }
                        // Bottom sentinel
                        Color.clear.frame(height: 1).id("bottom")
                    }
                    .padding(.horizontal, 22)
                    .background(OATheme.Colors.background)
                }
                .background(OATheme.Colors.background)
                .onAppear { scrollToBottom(proxy) }
                .onChange(of: timeline.count) { _, _ in scrollToBottom(proxy) }
                .sheet(isPresented: Binding(get: { reasoningSheet != nil }, set: { v in if !v { reasoningSheet = nil } })) {
                    reasoningDetailSheet
                }
            }
        }
    }

    @ViewBuilder
    private func messageRow(for item: TimelineItem) -> some View {
        switch item {
        case .message(let msg):
            messageBody(for: msg)
        case .reasoningSummary(let rs):
            let secs = max(0, Int((rs.endTs - rs.startTs) / 1000))
            Button(action: { reasoningSheet = rs.messages }) {
                Text("Thought for \(secs)s").font(.footnote)
                .foregroundStyle(OATheme.Colors.textSecondary)
                .padding(.horizontal, 10)
                .padding(.vertical, 6)
                .background(
                    Group {
                        #if os(iOS)
                        if #available(iOS 26, *) {
                            GlassEffectContainer {
                                Capsule(style: .continuous)
                                    .fill(Color.clear)
                                    .glassEffect(.regular, in: Capsule(style: .continuous))
                            }
                        } else {
                            Capsule(style: .continuous).fill(.ultraThinMaterial)
                        }
                        #else
                        Capsule(style: .continuous).fill(.regularMaterial)
                        #endif
                    }
                )
                .background(
                    Capsule(style: .continuous)
                        .fill(LinearGradient(colors: [Color.black.opacity(0.14), Color.black.opacity(0.05)], startPoint: .top, endPoint: .bottom))
                )
                .overlay(
                    Capsule(style: .continuous)
                        .strokeBorder(OATheme.Colors.border.opacity(0.6), lineWidth: 1)
                )
                .clipShape(Capsule(style: .continuous))
            }
        case .reasoning:
            EmptyView()
        case .toolCall(let call):
            if Features.showRawJSON { ToolCallView(call: call) }
        case .toolResult(let res):
            if Features.showRawJSON { ToolResultView(result: res) }
        case .plan(let ps):
            PlanStateView(state: ps)
        case .raw(let line):
            if Features.showRawJSON { RawEventView(line: line) }
        }
    }

    @ViewBuilder
    private func messageBody(for msg: ACPMessage) -> some View {
        let isUser = (msg.role == .user)
        ForEach(Array(msg.parts.enumerated()), id: \.0) { pair in
            let idx = pair.0
            if case let .text(t) = msg.parts[idx] {
                renderMessageMarkdown(t.text, isUser: isUser)
            }
        }
    }

    // Customized Markdown renderer for messages: bullet styling + paragraph spacing
    @ViewBuilder
    private func renderMessageMarkdown(_ text: String, isUser: Bool) -> some View {
        let color: Color = isUser ? Color(hex: "#7A7A7A") : OATheme.Colors.textPrimary
        let normalized = text.replacingOccurrences(of: "\r\n", with: "\n")
        let paras = normalized.components(separatedBy: "\n\n")
        struct Item: Identifiable { let id = UUID(); let level: Int; let bullet: Bool; let content: String }
        var items: [Item] = []
        var lastWasPara = false
        for para in paras {
            if isBulletBlock(para) {
                let lines = para.split(separator: "\n").map(String.init)
                // Compute baseline indent across this block
                var minIndent = Int.max
                var infos: [(indent:Int, content:String)] = []
                for l in lines {
                    let (isB, ind, content) = bulletInfo(l)
                    if isB { minIndent = min(minIndent, ind); infos.append((ind, content)) }
                }
                if minIndent == Int.max { minIndent = 0 }
                let basePad = lastWasPara ? 1 : 0
                for inf in infos {
                    let lvl = max(0, (inf.indent - minIndent)/2) + basePad
                    items.append(Item(level: lvl, bullet: true, content: inf.content))
                }
                lastWasPara = false
            } else {
                // Paragraph; indent one level if following a bullet section
                let lvl = lastWasPara ? 0 : 0
                items.append(Item(level: lvl, bullet: false, content: para))
                lastWasPara = true
            }
        }
        VStack(alignment: .leading, spacing: 10) {
            ForEach(items) { it in
                if it.bullet {
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Circle().fill(color).frame(width: 5, height: 5).padding(.top, 3)
                        markdownText(it.content)
                            .font(OAFonts.ui(.body, 14))
                            .lineLimit(isUser ? 5 : nil)
                            .truncationMode(.tail)
                            .foregroundStyle(color)
                            .textSelection(.enabled)
                    }
                    .padding(.leading, CGFloat(it.level) * 14)
                } else {
                    markdownText(it.content)
                        .font(OAFonts.ui(.body, 14))
                        .lineLimit(isUser ? 5 : nil)
                        .truncationMode(.tail)
                        .foregroundStyle(color)
                        .textSelection(.enabled)
                        .padding(.leading, CGFloat(it.level) * 14)
                        .padding(.vertical, 2)
                }
            }
        }
    }

    private func isBulletBlock(_ para: String) -> Bool {
        let lines = para.split(separator: "\n").map(String.init)
        guard !lines.isEmpty else { return false }
        for l in lines {
            let (isB, _, _) = bulletInfo(l)
            if !isB { return false }
        }
        return true
    }

    private func bulletInfo(_ s: String) -> (isBullet: Bool, indent: Int, content: String) {
        var indent = 0
        var idx = s.startIndex
        while idx < s.endIndex && s[idx] == " " { indent += 1; idx = s.index(after: idx) }
        let trimmed = String(s[idx...])
        if trimmed.hasPrefix("- ") { return (true, indent, String(trimmed.dropFirst(2))) }
        if trimmed.hasPrefix("* ") { return (true, indent, String(trimmed.dropFirst(2))) }
        // numeric bullets like "1. " or "2) "
        var j = trimmed.startIndex
        var hadDigit = false
        while j < trimmed.endIndex, trimmed[j].isNumber { hadDigit = true; j = trimmed.index(after: j) }
        if hadDigit, j < trimmed.endIndex, (trimmed[j] == "." || trimmed[j] == ")"), trimmed.index(after: j) < trimmed.endIndex, trimmed[trimmed.index(after: j)] == " " {
            let content = String(trimmed[trimmed.index(j, offsetBy: 2)...])
            return (true, indent, content)
        }
        return (false, indent, trimmed)
    }

    // Reasoning detail sheet
    @ViewBuilder
    private var reasoningDetailSheet: some View {
        if let messages = reasoningSheet {
            NavigationStack {
                List {
                    ForEach(Array(messages.enumerated()), id: \.0) { pair in
                        let idx = pair.0
                        let m = messages[idx]
                        ForEach(Array(m.parts.enumerated()), id: \.0) { p in
                            if case let .text(t) = m.parts[p.0] {
                                markdownText(t.text)
                                    .font(OAFonts.ui(.body, 14))
                                    .foregroundStyle(OATheme.Colors.textPrimary)
                                    .textSelection(.enabled)
                            }
                        }
                    }
                }
                #if os(iOS)
                .listStyle(.insetGrouped)
                #else
                .listStyle(.inset)
                #endif
                .navigationTitle("Thoughts")
                .toolbar {
                    #if os(iOS)
                    ToolbarItem(placement: .topBarLeading) { Button("Back") { reasoningSheet = nil } }
                    #else
                    ToolbarItem(placement: .navigation) { Button("Back") { reasoningSheet = nil } }
                    #endif
                }
            }
        }
    }

    func statusText() -> String {
        switch bridge.status {
        case .connecting(let h, let p): return "Connecting to \(h):\(p)…"
        case .handshaking(let h, let p): return "Handshaking \(h):\(p)…"
        case .connected: return timeline.isEmpty ? "Loading latest thread…" : ""
        case .discovering: return "Discovering bridge…"
        case .advertising(let port): return "Advertising :\(port)"
        case .error(let e): return "Bridge error: \(e)"
        case .idle: return "Connecting…"
        }
    }

    func roleBadge(_ role: ACPRole) -> some View {
        let (label, sys) : (String, String) = {
            switch role {
            case .user: return ("User", "person")
            case .assistant: return ("Assistant", "sparkles")
            case .system: return ("System", "gear")
            case .tool: return ("Tool", "wrench")
            }
        }()
        return Label(label, systemImage: sys)
            .font(InterFont.font(relativeTo: .caption, size: 11))
            .foregroundStyle(OATheme.Colors.textSecondary)
    }

    func dateLabel(ms: Int64) -> String {
        let s = Date(timeIntervalSince1970: Double(ms) / 1000)
        let fmt = DateFormatter()
        fmt.dateStyle = .none
        fmt.timeStyle = .short
        return fmt.string(from: s)
    }

    // Minimal Markdown renderer using AttributedString
    func markdownText(_ text: String) -> Text {
        if let md = try? AttributedString(markdown: text, options: AttributedString.MarkdownParsingOptions(interpretedSyntax: .inlineOnlyPreservingWhitespace)) { return Text(md) }
        return Text(text)
    }

    // Hide low-signal provider-native events
    func shouldHideLine(_ line: String) -> Bool {
        guard let data = line.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return false }
        let t = ((obj["type"] as? String) ?? (obj["event"] as? String) ?? "").lowercased()
        if t == "turn_context" { return true }
        if t == "event_msg",
           let p = obj["payload"] as? [String: Any], ((p["type"] as? String) ?? "").lowercased() == "token_count" { return true }
        return false
    }

    // Detect provider-native reasoning events for italic rendering
    func isReasoningLine(_ line: String) -> Bool {
        guard let data = line.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return false }
        let type = ((obj["type"] as? String) ?? (obj["event"] as? String) ?? "").lowercased()
        if type == "event_msg", let p = obj["payload"] as? [String: Any], ((p["type"] as? String) ?? "").lowercased() == "agent_reasoning" { return true }
        if let item = (obj["item"] as? [String: Any]) ?? (obj["msg"] as? [String: Any]) ?? (obj["payload"] as? [String: Any]),
           ((item["type"] as? String) ?? "").lowercased() == "agent_reasoning" { return true }
        if type == "response_item", let p = obj["payload"] as? [String: Any], ((p["type"] as? String) ?? "").lowercased() == "reasoning" { return true }
        return false
    }

    func load() {
        guard !isLoading else { return }
        if let lines = initialLines, !lines.isEmpty, timeline.isEmpty {
            isLoading = true; error = nil; timeline = []
            DispatchQueue.global(qos: .userInitiated).async {
                let (items, title) = AcpThreadView_computeTimeline(lines: lines, sourceId: "remote", cap: 100)
                DispatchQueue.main.async {
                    self.timeline = items
                    self.threadTitle = title
                    if let t = title, !t.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { self.onTitleChange?(t) }
                    self.isLoading = false
                }
            }
            return
        }
        guard let u = url else { return }
        isLoading = true
        error = nil
        timeline = []
        DispatchQueue.global(qos: .userInitiated).async {
            do {
                let lines = try tailJSONLLines(url: u, maxBytes: 1_000_000, maxLines: 5000)
                let (items, title) = AcpThreadView_computeTimeline(lines: lines, sourceId: u.path, cap: maxMessages)
                DispatchQueue.main.async {
                    self.timeline = items
                    self.threadTitle = title
                    if let t = title, !t.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty { self.onTitleChange?(t) }
                    self.isLoading = false
                }
            } catch {
                DispatchQueue.main.async {
                    self.error = "Failed to load: \(error.localizedDescription)"
                    self.isLoading = false
                }
            }
        }
    }
    func scrollToBottom(_ proxy: ScrollViewProxy) {
        // Scroll without animation so the thread appears already at the bottom
        DispatchQueue.main.async {
            var tx = Transaction()
            tx.disablesAnimations = true
            withTransaction(tx) { proxy.scrollTo("bottom", anchor: .bottom) }
        }
        // Double-fire shortly after to cover any layout churn, still without animation
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
            var tx = Transaction()
            tx.disablesAnimations = true
            withTransaction(tx) { proxy.scrollTo("bottom", anchor: .bottom) }
        }
    }

    // Intentionally not @MainActor: heavy compute can run off-main; caller updates UI on main.
    

    // Composer removed

    #if os(iOS)
    func appendUpdate(_ note: ACP.Client.SessionNotificationWire) {
        switch note.update {
        case .userMessageChunk(let chunk):
            if case let .text(s) = chunk.content {
                let newText = s.text.trimmingCharacters(in: .whitespacesAndNewlines)
                let now = nowMs()
                flushPendingReasoning(endMs: now)
                let m = ACPMessage(id: UUID().uuidString, thread_id: nil, role: .user, parts: [.text(ACPText(text: newText))], ts: now)
                timeline.append(.message(m))
            }
        case .agentMessageChunk(let chunk):
            switch chunk.content {
            case let .text(s):
                let newText = s.text.trimmingCharacters(in: .whitespacesAndNewlines)
                let now = nowMs()
                flushPendingReasoning(endMs: now)
                let m = ACPMessage(id: UUID().uuidString, thread_id: nil, role: .assistant, parts: [.text(ACPText(text: newText))], ts: now)
                timeline.append(.message(m))
            case let .resource_link(link):
                let s = "\(link.title ?? "Link") — \(link.uri)"
                let now = nowMs()
                flushPendingReasoning(endMs: now)
                let m = ACPMessage(id: UUID().uuidString, thread_id: nil, role: .assistant, parts: [.text(ACPText(text: s))], ts: now)
                timeline.append(.message(m))
            case let .image(img):
                let s = "[image] \(img.uri ?? "") \(img.mimeType)"
                let now = nowMs()
                flushPendingReasoning(endMs: now)
                let m = ACPMessage(id: UUID().uuidString, thread_id: nil, role: .assistant, parts: [.text(ACPText(text: s))], ts: now)
                timeline.append(.message(m))
            default:
                break
            }
        case .agentThoughtChunk(let chunk):
            if case let .text(s) = chunk.content {
                let text = s.text.trimmingCharacters(in: .whitespacesAndNewlines)
                let now = nowMs()
                if pendingReasoningStart == nil { pendingReasoningStart = now }
                let m = ACPMessage(id: UUID().uuidString, thread_id: nil, role: .assistant, parts: [.text(ACPText(text: text))], ts: now)
                pendingReasoning.append(m)
            }
        case .plan(let p):
            let ps = ACPPlanState(status: .running, summary: nil, steps: p.entries.map { $0.content }, ts: nowMs())
            timeline.append(.plan(ps))
        case .toolCall(let call):
            let args = call.arguments?.map { "\($0.key)=\($0.value)" }.joined(separator: ", ") ?? ""
            let txt = "[tool call] \(call.name)(\(args))"
            let m = ACPMessage(id: UUID().uuidString, thread_id: nil, role: .assistant, parts: [.text(ACPText(text: txt))], ts: nowMs())
            timeline.append(.message(m))
        case .toolCallUpdate(let upd):
            let status = upd.status.rawValue
            let msg = upd.error != nil ? "error: \(upd.error!)" : "ok"
            let txt = "[tool \(status)] call_id=\(upd.call_id) \(msg)"
            let m = ACPMessage(id: UUID().uuidString, thread_id: nil, role: .assistant, parts: [.text(ACPText(text: txt))], ts: nowMs())
            timeline.append(.message(m))
        case .availableCommandsUpdate(_):
            // No-op in this view; header observes from BridgeManager
            break
        case .currentModeUpdate(_):
            // No-op in this view; header observes from BridgeManager
            break
        }
    }
    #endif

    func nowMs() -> Int64 { Int64(Date().timeIntervalSince1970 * 1000) }

    private func flushPendingReasoning(endMs: Int64) {
        guard !pendingReasoning.isEmpty else { return }
        let start = pendingReasoningStart ?? pendingReasoning.first?.ts ?? endMs
        let rs = ReasoningSummary(startTs: start, endTs: endMs, messages: pendingReasoning)
        timeline.append(.reasoningSummary(rs))
        pendingReasoning.removeAll()
        pendingReasoningStart = nil
    }
}
// Close AcpThreadView struct

// MARK: - Efficient JSONL tail reader
private func tailJSONLLines(url: URL, maxBytes: Int, maxLines: Int) throws -> [String] {
    let fh = try FileHandle(forReadingFrom: url)
    defer { try? fh.close() }
    let chunk = 64 * 1024
    let fileSize = (try FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.intValue ?? 0
    var offset = fileSize
    var buffer = Data()
    var totalRead = 0
    while offset > 0 && totalRead < maxBytes {
        let toRead = min(chunk, offset)
        offset -= toRead
        try fh.seek(toOffset: UInt64(offset))
        let data = try fh.read(upToCount: toRead) ?? Data()
        buffer.insert(contentsOf: data, at: 0) // prepend
        totalRead += data.count
        if buffer.count >= maxBytes { break }
    }
    // Ensure we start at a line boundary: drop incomplete first line if needed
    var text = String(data: buffer, encoding: .utf8) ?? String(decoding: buffer, as: UTF8.self)
    if !text.hasSuffix("\n") { text.append("\n") }
    var lines = text.split(separator: "\n", omittingEmptySubsequences: true).map(String.init)
    if lines.count > maxLines { lines = Array(lines.suffix(maxLines)) }
    return lines
}

// MARK: - Timeline compute (pure function)
// Returns computed items and a derived thread title. Runs on any queue.
func AcpThreadView_computeTimeline(lines: [String], sourceId: String, cap: Int) -> ([AcpThreadView.TimelineItem], String?) {
    var items: [AcpThreadView.TimelineItem] = []
    var seenAssistant: Set<String> = []
    var seenUser: Set<String> = []
    var reasoningBuffer: [ACPMessage] = []
    var lastMessageTs: Int64 = 0
    var lastNonReasoningTs: Int64? = nil
    var fallbackClock: Int64 = 0 // approximate ms, +1000 per line when ts missing
    var monoMs: Int64 = 0        // monotonic ms fallback for all items

    func flushReasoningBuffer(nextTs: Int64, prevTs: Int64?) {
        guard !reasoningBuffer.isEmpty else { return }
        let start = prevTs ?? reasoningBuffer.first?.ts ?? nextTs
        let rs = AcpThreadView.ReasoningSummary(startTs: start, endTs: nextTs, messages: reasoningBuffer)
        items.append(.reasoningSummary(rs))
#if DEBUG
        let secs = max(0, Int((nextTs - start) / 1000))
        print("[timeline] reasoning summary start=\(start) end=\(nextTs) secs=\(secs) count=\(reasoningBuffer.count)")
#endif
        reasoningBuffer.removeAll()
    }

    for (idx, line) in lines.enumerated() {
        fallbackClock += 1000
        monoMs += 1000
        // Hide low-signal provider-native events
        if AcpThreadView_shouldHideLine(line) { continue }
        let t = CodexAcpTranslator.translateLines([line], options: .init(sourceId: sourceId))
        if let m = t.events.compactMap({ $0.message }).first {
            let text = m.parts.compactMap { part -> String? in
                if case let .text(t) = part { return t.text } else { return nil }
            }.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)

            if AcpThreadView_isReasoningLine(line) {
                var msg = m
                if msg.ts == 0 { msg.ts = resolveTsMs(fromLine: line) ?? monoMs }
                monoMs = max(monoMs, msg.ts)
                reasoningBuffer.append(msg)
                continue
            }

            // Non-reasoning message
            if !ConversationSummarizer.isSystemPreface(text) {
                // Any pending reasoning attaches before this message
                var ts = m.ts
                if ts == 0 { ts = resolveTsMs(fromLine: line) ?? monoMs }
                monoMs = max(monoMs, ts)
                flushReasoningBuffer(nextTs: ts, prevTs: lastNonReasoningTs)
                lastMessageTs = ts
                lastNonReasoningTs = ts
                items.append(.message(m))
                continue
            }
        } else if let c = t.events.compactMap({ $0.tool_call }).first {
            if Features.showRawJSON { items.append(.toolCall(c)) }
            continue
        } else if let r = t.events.compactMap({ $0.tool_result }).first {
            if Features.showRawJSON { items.append(.toolResult(r)) }
            continue
        }
        if Features.showRawJSON { items.append(.raw(line)) }

        // At end of input, if last lines were reasoning, flush using lastMessageTs (or own ts)
        if idx == lines.count - 1 && !reasoningBuffer.isEmpty {
            let endTs = max(lastMessageTs, reasoningBuffer.last?.ts ?? (resolveTsMs(fromLine: line) ?? monoMs))
            flushReasoningBuffer(nextTs: endTs, prevTs: lastNonReasoningTs)
        }
    }
    if items.count > cap { items = Array(items.suffix(cap)) }
    let thread = CodexAcpTranslator.translateLines(lines, options: .init(sourceId: sourceId))
    return (items, thread.title)
}

// These thin wrappers allow the pure function to reuse the same logic as the view without capturing self.
private func AcpThreadView_shouldHideLine(_ line: String) -> Bool {
    guard let data = line.data(using: .utf8),
          let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return false }
    let t = ((obj["type"] as? String) ?? (obj["event"] as? String) ?? "").lowercased()
    if t == "turn_context" { return true }
    if t == "event_msg",
       let p = obj["payload"] as? [String: Any], ((p["type"] as? String) ?? "").lowercased() == "token_count" { return true }
    return false
}
private func AcpThreadView_isReasoningLine(_ line: String) -> Bool {
    guard let data = line.data(using: .utf8),
          let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return false }
    let type = ((obj["type"] as? String) ?? (obj["event"] as? String) ?? "").lowercased()
    if type == "event_msg", let p = obj["payload"] as? [String: Any], ((p["type"] as? String) ?? "").lowercased() == "agent_reasoning" { return true }
    if let item = (obj["item"] as? [String: Any]) ?? (obj["msg"] as? [String: Any]) ?? (obj["payload"] as? [String: Any]),
       ((item["type"] as? String) ?? "").lowercased() == "agent_reasoning" { return true }
    if type == "response_item", let p = obj["payload"] as? [String: Any], ((p["type"] as? String) ?? "").lowercased() == "reasoning" { return true }
    return false
}

private func resolveTsMs(fromLine line: String) -> Int64? {
    guard let data = line.data(using: .utf8),
          let root = try? JSONSerialization.jsonObject(with: data) else { return nil }
    return scanForTimestamp(root)
}

private func extractISO(_ any: Any?) -> Int64? {
    guard let s = any as? String else { return nil }
    let fmt = ISO8601DateFormatter()
    if let date = fmt.date(from: s) { return Int64(date.timeIntervalSince1970 * 1000) }
    return nil
}

private func scanForTimestamp(_ any: Any?, depth: Int = 0) -> Int64? {
    if depth > 6 { return nil }
    if let n = any as? NSNumber { return n.int64Value }
    if let d = extractISO(any) { return d }
    if let dict = any as? [String: Any] {
        // direct keys
        let numericKeys = ["ts", "time_ms", "timestamp_ms", "created_at_ms", "updated_at_ms"]
        for k in numericKeys { if let v = dict[k] as? NSNumber { return v.int64Value } }
        let isoKeys = ["timestamp", "time", "created_at", "updated_at", "date"]
        for k in isoKeys { if let v = extractISO(dict[k]) { return v } }
        // nested common containers
        let nestedKeys = ["payload", "item", "msg", "event", "meta"]
        for k in nestedKeys { if let v = dict[k] { if let found = scanForTimestamp(v, depth: depth + 1) { return found } } }
        // scan all values as fallback
        for (_, v) in dict { if let found = scanForTimestamp(v, depth: depth + 1) { return found } }
    } else if let arr = any as? [Any] {
        for v in arr { if let found = scanForTimestamp(v, depth: depth + 1) { return found } }
    }
    return nil
}

//#Preview {
//    AcpThreadView(url: nil)
//}
