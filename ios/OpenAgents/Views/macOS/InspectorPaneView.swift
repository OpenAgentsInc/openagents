import SwiftUI
import OpenAgentsCore
#if os(macOS)
import AppKit
import UniformTypeIdentifiers
#endif

#if os(macOS)
struct InspectorPaneView: View {
    @EnvironmentObject private var bridge: BridgeManager
    @State private var mode: ViewMode = .formatted
    @State private var lastSavedURLByCallId: [String: URL] = [:]

    enum ViewMode: String, CaseIterable { case formatted = "Formatted", raw = "Raw" }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            Divider().background(OATheme.Colors.textTertiary.opacity(0.15))
            content
        }
        .background(OATheme.Colors.sidebarBackground)
    }

    private var header: some View {
        HStack(spacing: 8) {
            Text("Inspector")
                .font(OAFonts.mono(.body, 12))
                .foregroundStyle(OATheme.Colors.textSecondary)
            Spacer()
            if let id = bridge.selectedToolCallId {
                Picker("", selection: $mode) {
                    ForEach(ViewMode.allCases, id: \.self) { m in
                        Text(m.rawValue).tag(m)
                    }
                }
                .pickerStyle(.segmented)
                .frame(width: 200)
                Button(action: clearSelection) {
                    Image(systemName: "xmark.circle")
                }
                .buttonStyle(.plain)
                Button(action: { copyJSON(for: id) }) {
                    Image(systemName: "doc.on.doc")
                }
                .buttonStyle(.plain)
                Button(action: { saveJSON(for: id) }) {
                    Image(systemName: "square.and.arrow.down")
                }
                .buttonStyle(.plain)
                .disabled(currentJSON(for: id) == nil)
                Button(action: { revealInFinder(for: id) }) {
                    Image(systemName: "folder")
                }
                .buttonStyle(.plain)
                .disabled(lastSavedURLByCallId[id] == nil)
                Button(action: { openInEditor(for: id) }) {
                    Image(systemName: "arrow.up.right.square")
                }
                .buttonStyle(.plain)
                .disabled(lastSavedURLByCallId[id] == nil)
                Menu(content: {
                    Button("Export JSON…") { exportJSON() }
                    Button("Export Markdown…") { exportMarkdown() }
                }) {
                    Image(systemName: "square.and.arrow.up")
                }
                .menuStyle(.borderlessButton)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
    }

    @ViewBuilder
    private var content: some View {
        if let id = bridge.selectedToolCallId {
            ScrollView {
                VStack(alignment: .leading, spacing: 12) {
                    if let name = bridge.toolCallNames[id] {
                        LabeledRow(label: "Tool", value: name)
                    }
                    if let status = findStatus(for: id) {
                        LabeledRow(label: "Status", value: status)
                    }
                    LabeledRow(label: "Call ID", value: id)
                    // Arguments section
                    if let argsPretty = findArgumentsJSON(for: id) {
                        DisclosureGroup("Arguments") {
                            codeBlock(argsPretty)
                        }
                        .disclosureGroupStyle(.automatic)
                    }

                    // Output section
                    DisclosureGroup("Output") {
                        if let text = currentJSON(for: id) {
                            codeBlock(text)
                        } else {
                            Text("No output yet")
                                .font(OAFonts.mono(.caption, 11))
                                .foregroundStyle(OATheme.Colors.textSecondary)
                        }
                    }

                    // Error section
                    if let err = findError(for: id) {
                        DisclosureGroup("Error") {
                            Text(err)
                                .font(OAFonts.mono(.caption, 11))
                                .foregroundStyle(OATheme.Colors.danger)
                                .textSelection(.enabled)
                        }
                        .disclosureGroupStyle(.automatic)
                    }
                }
                .padding(12)
            }
        } else {
            VStack(alignment: .leading, spacing: 8) {
                Text("Nothing selected")
                    .font(OAFonts.mono(.body, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)
                Text("Click a tool call in the chat to inspect its output.")
                    .font(OAFonts.mono(.caption, 11))
                    .foregroundStyle(OATheme.Colors.textTertiary)
            }
            .padding(12)
        }
    }

    private func copyJSON(for id: String) {
        #if os(macOS)
        let s = currentJSON(for: id) ?? ""
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(s, forType: .string)
        #endif
    }

    private func clearSelection() { bridge.selectedToolCallId = nil }

    private func currentJSON(for id: String) -> String? {
        switch mode {
        case .formatted:
            return bridge.outputJSONByCallId[id] ?? bridge.rawJSONByCallId[id]
        case .raw:
            return bridge.rawJSONByCallId[id] ?? bridge.outputJSONByCallId[id]
        }
    }

    private func findStatus(for id: String) -> String? {
        for note in bridge.updates.reversed() {
            switch note.update {
            case .toolCallUpdate(let upd) where upd.call_id == id:
                return upd.status.rawValue
            default:
                continue
            }
        }
        return nil
    }

    private func findError(for id: String) -> String? {
        for note in bridge.updates.reversed() {
            switch note.update {
            case .toolCallUpdate(let upd) where upd.call_id == id:
                if let e = upd.error, !e.isEmpty { return e }
            default:
                continue
            }
        }
        return nil
    }

    private func suggestedFileName(for id: String) -> String {
        let tool = bridge.toolCallNames[id] ?? "tool"
        return "\(tool)-\(id.prefix(8)).json"
    }

    private func saveJSON(for id: String) {
        #if os(macOS)
        guard let text = currentJSON(for: id), let data = text.data(using: .utf8) else { return }
        let panel = NSSavePanel()
        panel.title = "Save JSON"
        panel.nameFieldStringValue = suggestedFileName(for: id)
        panel.allowedContentTypes = [.json]
        panel.canCreateDirectories = true
        if panel.runModal() == .OK, let url = panel.url {
            do { try data.write(to: url); lastSavedURLByCallId[id] = url } catch { }
        }
        #endif
    }

    private func revealInFinder(for id: String) {
        #if os(macOS)
        guard let url = lastSavedURLByCallId[id] else { return }
        NSWorkspace.shared.activateFileViewerSelecting([url])
        #endif
    }

    private func openInEditor(for id: String) {
        #if os(macOS)
        guard let url = lastSavedURLByCallId[id] else { return }
        NSWorkspace.shared.open(url)
        #endif
    }

    // MARK: - Export transcript (JSON / Markdown)
    private func exportJSON() {
        #if os(macOS)
        guard let sid = bridge.currentSessionId?.value else { return }
        let updates = bridge.updates.filter { $0.session_id.value == sid }
        guard let data = try? JSONEncoder().encode(updates) else { return }
        let panel = NSSavePanel()
        panel.title = "Export Transcript (JSON)"
        panel.nameFieldStringValue = "openagents-\(sid.prefix(8)).json"
        panel.allowedContentTypes = [.json]
        panel.canCreateDirectories = true
        if panel.runModal() == .OK, let url = panel.url {
            try? data.write(to: url)
        }
        #endif
    }

    private func exportMarkdown() {
        #if os(macOS)
        guard let sid = bridge.currentSessionId?.value else { return }
        let updates = bridge.updates.filter { $0.session_id.value == sid }
        let md = makeMarkdown(from: updates)
        guard let data = md.data(using: .utf8) else { return }
        let panel = NSSavePanel()
        panel.title = "Export Transcript (Markdown)"
        panel.nameFieldStringValue = "openagents-\(sid.prefix(8)).md"
        if #available(macOS 11.0, *) {
            let md = UTType(filenameExtension: "md")
            panel.allowedContentTypes = [md ?? .plainText]
        }
        panel.canCreateDirectories = true
        if panel.runModal() == .OK, let url = panel.url {
            try? data.write(to: url)
        }
        #endif
    }

    private func makeMarkdown(from updates: [ACP.Client.SessionNotificationWire]) -> String {
        var out: [String] = ["# OpenAgents Transcript\n"]
        for note in updates {
            switch note.update {
            case .userMessageChunk(let chunk):
                if case .text(let t) = chunk.content {
                    out.append("\n**User**\n\n" + t.text + "\n")
                }
            case .agentMessageChunk(let chunk):
                if case .text(let t) = chunk.content {
                    out.append("\n**Assistant**\n\n" + t.text + "\n")
                }
            case .agentThoughtChunk(let chunk):
                if case .text(let t) = chunk.content {
                    out.append("\n> _Thinking:_ " + t.text.replacingOccurrences(of: "\n", with: " ") + "\n")
                }
            case .plan(let plan):
                let bullets = plan.entries.map { "- " + $0.content }.joined(separator: "\n")
                out.append("\n**Plan**\n\n" + bullets + "\n")
            case .toolCall(let call):
                out.append("\n`tool_call` \(call.name) id=\(call.call_id)\n")
            case .toolCallUpdate(let upd):
                out.append("`tool_call_update` id=\(upd.call_id) status=\(upd.status.rawValue)\n")
            case .availableCommandsUpdate, .currentModeUpdate:
                continue
            }
        }
        return out.joined(separator: "\n")
    }

    // MARK: - Pretty code helpers
    private func codeBlock(_ text: String) -> some View {
        Text(text)
            .font(OAFonts.mono(.caption, 11))
            .foregroundStyle(OATheme.Colors.textPrimary)
            .textSelection(.enabled)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(8)
            .background(OATheme.Colors.background.opacity(0.3))
            .cornerRadius(6)
    }

    private func findArgumentsJSON(for id: String) -> String? {
        for note in bridge.updates.reversed() {
            switch note.update {
            case .toolCall(let call) where call.call_id == id:
                if let args = call.arguments,
                   let data = try? JSONEncoder().encode(args),
                   let text = String(data: data, encoding: .utf8) {
                    // Pretty print
                    if let obj = try? JSONSerialization.jsonObject(with: Data(text.utf8)),
                       let pretty = try? JSONSerialization.data(withJSONObject: obj, options: [.prettyPrinted, .sortedKeys]) {
                        return String(decoding: pretty, as: UTF8.self)
                    }
                    return text
                }
            default:
                continue
            }
        }
        return nil
    }
}

private struct LabeledRow: View {
    let label: String
    let value: String
    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label.uppercased())
                .font(OAFonts.mono(.caption, 10))
                .foregroundStyle(OATheme.Colors.textSecondary)
            Text(value)
                .font(OAFonts.mono(.body, 12))
                .foregroundStyle(OATheme.Colors.textPrimary)
        }
    }
}
#endif
