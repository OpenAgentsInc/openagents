import SwiftUI
import OpenAgentsCore

struct AcpThreadView: View {
    let url: URL?
    let maxMessages: Int = 10
    var onTitleChange: ((String) -> Void)? = nil

    @State private var isLoading = false
    @State private var error: String? = nil
    enum TimelineItem: Identifiable {
        case message(ACPMessage)
        case toolCall(ACPToolCall)
        case toolResult(ACPToolResult)
        // plan state can be added later

        var id: String {
            switch self {
            case .message(let m): return "msg_\(m.id)"
            case .toolCall(let c): return "call_\(c.id)"
            case .toolResult(let r): return "res_\(r.call_id)\(r.ts ?? 0)"
            }
        }
        var ts: Int64 {
            switch self {
            case .message(let m): return m.ts
            case .toolCall(let c): return c.ts ?? 0
            case .toolResult(let r): return r.ts ?? 0
            }
        }
    }

    @State private var timeline: [TimelineItem] = []
    @State private var threadTitle: String? = nil
    @State private var draft: String = ""

    var body: some View {
        return ZStack {
        Group {
            if url == nil {
                Text("Select a thread")
                    .font(Font.custom(BerkeleyFont.defaultName(), size: 17, relativeTo: .headline))
                    .foregroundStyle(OATheme.Colors.textSecondary)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if isLoading && timeline.isEmpty {
                VStack(spacing: 8) {
                    ProgressView()
                    Text("Loading…")
                        .font(Font.custom(BerkeleyFont.defaultName(), size: 12, relativeTo: .caption))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let e = error {
                ScrollView { Text(e).font(.footnote) }.padding()
            } else {
                ZStack(alignment: .bottom) {
                    ScrollViewReader { proxy in
                    List {
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
                                case .toolCall(let call):
                                    ToolCallView(call: call)
                                case .toolResult(let res):
                                    ToolResultView(result: res)
                                }
                            }
                            .padding(.vertical, 4)
                        }
                    }
                    .listStyle(.plain)
                    .scrollContentBackground(.hidden)
                    .background(OATheme.Colors.background)
                    // Room for composer is handled by safeAreaInset below
                    .onAppear {
                        scrollToBottom(proxy)
                    }
                    .onChange(of: timeline.count) { _ in
                        scrollToBottom(proxy)
                    }
                }
                .safeAreaInset(edge: .bottom) {
                    GlassBar {
                        Image(systemName: "rectangle.and.pencil.and.ellipsis")
                            .imageScale(.medium)
                            .foregroundStyle(OATheme.Colors.textSecondary)
                        TextField("Compose…", text: $draft, axis: .vertical)
                            .textFieldStyle(.plain)
                            .font(BerkeleyFont.font(relativeTo: .body, size: 15))
                            .foregroundStyle(OATheme.Colors.textPrimary)
                            .lineLimit(1...4)
                            .submitLabel(.send)
                            .onSubmit { sendDraft() }
                        Spacer(minLength: 8)
                        Button(action: { sendDraft() }) {
                            Image(systemName: "arrow.up.circle.fill")
                                .imageScale(.large)
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? OATheme.Colors.textTertiary : OATheme.Colors.textSecondary)
                        .disabled(draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    }
                    .background(.clear)
                    .zIndex(100)
                }
            }
        }
        }
        .background(OATheme.Colors.background)
        .onChange(of: url?.path) { _, _ in load() }
        .onAppear(perform: load)
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
        if let md = try? AttributedString(markdown: text) { return Text(md) }
        return Text(text)
    }

    func load() {
        guard let u = url else { return }
        guard !isLoading else { return }
        isLoading = true
        error = nil
        timeline = []
        DispatchQueue.global(qos: .userInitiated).async {
            do {
                let lines = try tailJSONLLines(url: u, maxBytes: 1_000_000, maxLines: 5000)
                let thread = CodexAcpTranslator.translateLines(lines, options: .init(sourceId: u.path))
                // Build a unified timeline from messages + tool calls/results
                var items: [TimelineItem] = []
                for ev in thread.events {
                    if let m = ev.message, (m.role == .user || m.role == .assistant) {
                        // ignore synthetic preface
                        let text = m.parts.compactMap { part -> String? in
                            if case let .text(t) = part { return t.text } else { return nil }
                        }.joined(separator: " ")
                        if !ConversationSummarizer.isSystemPreface(text) {
                            items.append(.message(m))
                        }
                    } else if let c = ev.tool_call {
                        items.append(.toolCall(c))
                    } else if let r = ev.tool_result {
                        items.append(.toolResult(r))
                    }
                }
                items.sort { $0.ts < $1.ts }
                if items.count > maxMessages { items = Array(items.suffix(maxMessages)) }
                DispatchQueue.main.async {
                    withAnimation { self.timeline = items }
                    self.threadTitle = thread.title
                    if let t = thread.title, !t.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                        self.onTitleChange?(t)
                    }
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
        guard let last = timeline.last else { return }
        DispatchQueue.main.async {
            withAnimation { proxy.scrollTo(last.id, anchor: .bottom) }
        }
    }

    func sendDraft() {
        let text = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        let msg = ACPMessage(
            id: UUID().uuidString,
            thread_id: nil,
            role: .user,
            parts: [.text(ACPText(text: text))],
            ts: Int64(Date().timeIntervalSince1970 * 1000)
        )
        timeline.append(.message(msg))
        draft = ""
    }
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

#Preview {
    AcpThreadView(url: nil)
}
