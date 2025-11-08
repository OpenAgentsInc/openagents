import SwiftUI
import OpenAgentsCore

/// Renders the list of non-plan session updates using UpdateRow.
/// Calls `onSelectJSON` with a pretty JSON string when a row is tapped.
struct UpdatesListView: View {
    let updates: [ACP.Client.SessionNotificationWire]
    var onSelectJSON: (String?) -> Void
    @EnvironmentObject private var bridge: BridgeManager

    var body: some View {
        let visibleIndices = updates.indices.filter { idx in
            if case .plan = updates[idx].update { return false } else { return true }
        }
        return List(visibleIndices, id: \.self) { idx in
            let note = updates[idx]
            UpdateRow(note: note) { tapped in
                // Prefer the latest output JSON for this call_id if available
                if let cid = callId(from: tapped), let out = bridge.outputJSONByCallId[cid] {
                    onSelectJSON(out)
                } else if let json = toPrettyJSON(tapped) {
                    onSelectJSON(json)
                } else {
                    onSelectJSON(nil)
                }
            }
        }
        .listStyle(.plain)
    }
}

// Single row renderer for updates, extracted from ChatHomeView for reuse.
struct UpdateRow: View {
    let note: ACP.Client.SessionNotificationWire
    var onInspect: ((ACP.Client.SessionNotificationWire) -> Void)? = nil
    @EnvironmentObject private var bridge: BridgeManager
    @Environment(\.colorScheme) private var colorScheme
    var body: some View {
        switch note.update {
        case .toolCall(let call):
            HStack {
                Image(systemName: "hammer")
                Text(call.name)
                Spacer()
                Text("call_id: \(call.call_id.prefix(8))…")
                    .foregroundStyle(.secondary)
                    .font(.footnote)
            }
            .contentShape(Rectangle())
            .onTapGesture { onInspect?(note) }
        case .toolCallUpdate(let upd):
            HStack(alignment: .firstTextBaseline) {
                let hasOutput = bridge.outputJSONByCallId[upd.call_id] != nil
                // Extract numeric progress if available
                let progressValue: Double? = {
                    if let meta = upd._meta, let p = meta["progress"]?.toJSONValue() {
                        switch p {
                        case .number(let d): return d
                        case .string(let s):
                            if s.hasSuffix("%"), let v = Double(s.dropLast()) { return v / 100.0 }
                            if let v = Double(s) { return v }
                            return nil
                        default: return nil
                        }
                    }
                    return nil
                }()
                let isCompleteish = hasOutput || upd.status == .completed || ((progressValue ?? 0.0) >= 0.999)
                // Leading status icon/spinner sized like SF Symbol
                if upd.status == .error {
                    Image(systemName: "xmark.octagon")
                        .foregroundStyle(OATheme.Colors.danger)
                } else if isCompleteish {
                    Image(systemName: "checkmark.circle")
                } else {
                    ProgressView().progressViewStyle(.circular)
                        .tint(colorScheme == .dark ? .white : OATheme.Colors.textSecondary)
                        .frame(width: 14, height: 14)
                        .alignmentGuide(.firstTextBaseline) { d in d[VerticalAlignment.center] }
                }
                // Show the original tool name if known; fall back to the call_id
                let name = bridge.toolCallNames[upd.call_id] ?? "call \(upd.call_id.prefix(8))…"
                if let pv = progressValue, !isCompleteish {
                    Text("\(name) (\(Int((pv * 100).rounded()))%)")
                } else {
                    Text(name)
                }
                Spacer()
                let statusText = isCompleteish ? ACPToolCallUpdateWire.Status.completed.rawValue : upd.status.rawValue
                Text(statusText)
                    .font(.footnote)
                    .foregroundStyle(upd.status == .error ? .red : .secondary)
            }
            .contentShape(Rectangle())
            .onTapGesture { onInspect?(note) }
        case .agentMessageChunk(let chunk):
            // Render agent message content without extra header label/icon
            VStack(alignment: .leading, spacing: 4) {
                markdownText(extractText(from: chunk))
                    .font(.body)
            }
        case .userMessageChunk(let chunk):
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Image(systemName: "person")
                        .foregroundStyle(.green)
                    Text("User")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Text(extractText(from: chunk))
                    .font(.body)
            }
        case .agentThoughtChunk(let chunk):
            HStack {
                Image(systemName: "brain")
                    .foregroundStyle(.purple)
                Text(extractText(from: chunk))
                    .italic()
                    .foregroundStyle(.secondary)
            }
        case .plan:
            // Hide inline plan rows; plan is rendered once at the top
            EmptyView()
        case .availableCommandsUpdate(let ac):
            Text("Available: \(ac.available_commands.count) commands")
                .foregroundStyle(.secondary)
        case .currentModeUpdate(let cur):
            Text("Mode: \(cur.current_mode_id.rawValue)")
                .foregroundStyle(.secondary)
        }
    }

    /// Extract text content from a ContentChunk
    private func extractText(from chunk: ACP.Client.ContentChunk) -> String {
        switch chunk.content {
        case .text(let textContent):
            return textContent.text
        case .image:
            return "[Image]"
        case .audio:
            return "[Audio]"
        case .resource_link(let link):
            return "Resource: \(link.uri)"
        case .resource(let embeddedResource):
            // EmbeddedResource has a nested resource field which is an enum
            switch embeddedResource.resource {
            case .text(let textResource):
                return "Embedded text: \(textResource.uri)"
            case .blob(let blobResource):
                return "Embedded blob: \(blobResource.uri)"
            }
        }
    }

    /// Render markdown text
    private func markdownText(_ text: String) -> Text {
        if let md = try? AttributedString(markdown: text, options: AttributedString.MarkdownParsingOptions(interpretedSyntax: .inlineOnlyPreservingWhitespace)) {
            return Text(md)
        }
        return Text(text)
    }
}

