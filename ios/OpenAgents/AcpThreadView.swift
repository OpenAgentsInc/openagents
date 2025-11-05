import SwiftUI
import OpenAgentsCore

struct AcpThreadView: View {
    let url: URL?
    var initialLines: [String]? = nil
    let maxMessages: Int = 25
    var onTitleChange: ((String) -> Void)? = nil

    @EnvironmentObject var bridge: BridgeManager
    @State private var isLoading = false
    @State private var error: String? = nil
    
    enum TimelineItem: Identifiable {
        case message(ACPMessage)
        case reasoning(ACPMessage)
        case toolCall(ACPToolCall)
        case toolResult(ACPToolResult)
        case plan(ACPPlanState)
        case raw(String)

        var id: String {
            switch self {
            case .message(let m): return "msg_\(m.id)"
            case .reasoning(let m):
                let text = m.parts.compactMap { part -> String? in
                    if case let .text(t) = part { return t.text } else { return nil }
                }.joined()
                return "reason_\(m.id)_\(m.ts)_\(text.hashValue)"
            case .toolCall(let c): return "call_\(c.id)"
            case .toolResult(let r): return "res_\(r.call_id)\(r.ts ?? 0)"
            case .plan(let p): return "plan_\(p.ts ?? 0)"
            case .raw(let s): return "raw_\(s.hashValue)"
            }
        }
        var ts: Int64 {
            switch self {
            case .message(let m): return m.ts
            case .reasoning(let m): return m.ts
            case .toolCall(let c): return c.ts ?? 0
            case .toolResult(let r): return r.ts ?? 0
            case .plan(let p): return p.ts ?? 0
            case .raw: return 0
            }
        }
    }

    @State private var timeline: [TimelineItem] = []
    @State private var threadTitle: String? = nil
    // Track recent texts to avoid duplicates across non-consecutive items
    @State private var seenAssistantMessageTexts: Set<String> = []
    @State private var seenUserMessageTexts: Set<String> = []
    @State private var seenReasoningTexts: Set<String> = []

    var body: some View {
        return ZStack {
        Group {
            if (url == nil && timeline.isEmpty) || (isLoading && timeline.isEmpty) {
                VStack(spacing: 10) {
                    ProgressView()
                    Text(statusText())
                        .font(Font.custom(BerkeleyFont.defaultName(), size: 12, relativeTo: .caption))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let e = error, timeline.isEmpty {
                VStack(spacing: 8) {
                    Text(e)
                        .font(.footnote)
                        .foregroundStyle(OATheme.Colors.textSecondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ZStack(alignment: .bottom) {
                    ScrollViewReader { proxy in
                    List {
                #if os(iOS)
                // Mode and commands summary header
                HStack(spacing: 8) {
                    Text("Mode:")
                        .font(Font.custom(BerkeleyFont.defaultName(), size: 11, relativeTo: .caption2))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                    Text("\(bridge.currentMode.rawValue)")
                        .font(Font.custom(BerkeleyFont.defaultName(), size: 11, relativeTo: .caption2))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                    Spacer()
                    if !bridge.availableCommands.isEmpty {
                        Text("Commands: \(bridge.availableCommands.count)")
                            .font(Font.custom(BerkeleyFont.defaultName(), size: 11, relativeTo: .caption2))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                    }
                }
                .listRowBackground(Color.clear)
                #endif
                if let title = threadTitle, !title.isEmpty {
                    Section { Text(title).font(Font.custom(BerkeleyFont.defaultName(), size: 17, relativeTo: .headline)).foregroundStyle(OATheme.Colors.textPrimary) }
                }
                        ForEach(timeline) { item in
                            VStack(alignment: .leading, spacing: 6) {
                                switch item {
                                case .message(let msg):
                                    HStack(spacing: 6) {
                                        roleBadge(msg.role)
                                        Text(dateLabel(ms: msg.ts))
                                            .font(Font.custom(BerkeleyFont.defaultName(), size: 10, relativeTo: .caption2))
                                            .foregroundStyle(OATheme.Colors.textSecondary)
                                    }
                                    ForEach(msg.parts.indices, id: \.self) { idx in
                                        if case let .text(t) = msg.parts[idx] {
                                            markdownText(t.text)
                                                .textSelection(.enabled)
                                                .font(BerkeleyFont.font(relativeTo: .body, size: 15))
                                                .foregroundStyle(OATheme.Colors.textPrimary)
                                        }
                                    }
                                case .reasoning(let msg):
                                    HStack(spacing: 6) {
                                        roleBadge(.assistant)
                                        Text(dateLabel(ms: msg.ts))
                                            .font(Font.custom(BerkeleyFont.defaultName(), size: 10, relativeTo: .caption2))
                                            .foregroundStyle(OATheme.Colors.textSecondary)
                                    }
                                    ForEach(msg.parts.indices, id: \.self) { idx in
                                        if case let .text(t) = msg.parts[idx] {
                                            markdownText(t.text)
                                                .italic()
                                                .textSelection(.enabled)
                                                .font(BerkeleyFont.font(relativeTo: .body, size: 15))
                                                .foregroundStyle(OATheme.Colors.textPrimary)
                                        }
                                    }
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
                            .padding(.vertical, 4)
                        }
                        // Bottom sentinel to ensure we can always scroll truly to the end
                        Color.clear.frame(height: 1).id("bottom")
                    }
                    .listStyle(.plain)
                    .scrollContentBackground(.hidden)
                    .background(OATheme.Colors.background)
                    // Room for composer is handled by safeAreaInset below
                    .onAppear {
                        scrollToBottom(proxy)
                    }
                     .onChange(of: timeline.count) { _, _ in
                        scrollToBottom(proxy)
                    }
                }
                // Composer removed: messages-only view per request
            }
        }
        }
        .background(OATheme.Colors.background)
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
            .font(Font.custom(BerkeleyFont.defaultName(), size: 11, relativeTo: .caption))
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
                if !seenUserMessageTexts.contains(newText) {
                    seenUserMessageTexts.insert(newText)
                    let m = ACPMessage(id: UUID().uuidString, thread_id: nil, role: .user, parts: [.text(ACPText(text: newText))], ts: nowMs())
                    timeline.append(.message(m))
                }
            }
        case .agentMessageChunk(let chunk):
            switch chunk.content {
            case let .text(s):
                let newText = s.text.trimmingCharacters(in: .whitespacesAndNewlines)
                if !seenAssistantMessageTexts.contains(newText) {
                    seenAssistantMessageTexts.insert(newText)
                    let m = ACPMessage(id: UUID().uuidString, thread_id: nil, role: .assistant, parts: [.text(ACPText(text: newText))], ts: nowMs())
                    timeline.append(.message(m))
                }
            case let .resource_link(link):
                let s = "\(link.title ?? "Link") — \(link.uri)"
                let m = ACPMessage(id: UUID().uuidString, thread_id: nil, role: .assistant, parts: [.text(ACPText(text: s))], ts: nowMs())
                timeline.append(.message(m))
            case let .image(img):
                let s = "[image] \(img.uri ?? "") \(img.mimeType)"
                let m = ACPMessage(id: UUID().uuidString, thread_id: nil, role: .assistant, parts: [.text(ACPText(text: s))], ts: nowMs())
                timeline.append(.message(m))
            default:
                break
            }
        case .agentThoughtChunk(let chunk):
            if case let .text(s) = chunk.content {
                let newText = s.text.trimmingCharacters(in: .whitespacesAndNewlines)
                let m = ACPMessage(id: UUID().uuidString, thread_id: nil, role: .assistant, parts: [.text(ACPText(text: newText))], ts: nowMs())
                timeline.append(.reasoning(m))
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
private func AcpThreadView_computeTimeline(lines: [String], sourceId: String, cap: Int) -> ([AcpThreadView.TimelineItem], String?) {
    var items: [AcpThreadView.TimelineItem] = []
    var seenAssistant: Set<String> = []
    var seenUser: Set<String> = []
    var seenReasoning: Set<String> = []
    for line in lines {
        // Hide low-signal provider-native events
        if AcpThreadView_shouldHideLine(line) { continue }
        let t = CodexAcpTranslator.translateLines([line], options: .init(sourceId: sourceId))
        if let m = t.events.compactMap({ $0.message }).first, (m.role == .user || m.role == .assistant) {
            let text = m.parts.compactMap { part -> String? in
                if case let .text(t) = part { return t.text } else { return nil }
            }.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
            if !ConversationSummarizer.isSystemPreface(text) {
                if AcpThreadView_isReasoningLine(line) {
                    // Avoid collapsing distinct reasoning chunks that share similar text
                    // Use a composite key of text and message timestamp
                    let key = "\(text)|\(m.ts)"
                    if !seenReasoning.contains(key) {
                        seenReasoning.insert(key)
                        items.append(.reasoning(m))
                    }
                } else {
                    if m.role == .assistant {
                        if !seenAssistant.contains(text) { seenAssistant.insert(text); items.append(.message(m)) }
                    } else if m.role == .user {
                        if !seenUser.contains(text) { seenUser.insert(text); items.append(.message(m)) }
                    } else {
                        items.append(.message(m))
                    }
                }
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

//#Preview {
//    AcpThreadView(url: nil)
//}
}
