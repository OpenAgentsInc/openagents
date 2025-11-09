import SwiftUI
import OpenAgentsCore

#if os(macOS)
struct InspectorPaneView: View {
    @EnvironmentObject private var bridge: BridgeManager

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            Divider().background(OATheme.Colors.textTertiary.opacity(0.15))
            content
        }
        .background(OATheme.Colors.sidebarBackground)
    }

    private var header: some View {
        HStack {
            Text("Inspector").font(OAFonts.mono(.body, 12)).foregroundStyle(OATheme.Colors.textSecondary)
            Spacer()
            if let id = bridge.selectedToolCallId {
                Button(action: { copyJSON(for: id) }) {
                    Image(systemName: "doc.on.doc")
                }.buttonStyle(.plain)
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
                    if let pretty = bridge.outputJSONByCallId[id] ?? bridge.rawJSONByCallId[id] {
                        Text(pretty)
                            .font(OAFonts.mono(.caption, 11))
                            .foregroundStyle(OATheme.Colors.textPrimary)
                            .textSelection(.enabled)
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(8)
                            .background(OATheme.Colors.background.opacity(0.3))
                            .cornerRadius(6)
                    } else {
                        Text("No output yet")
                            .font(OAFonts.mono(.caption, 11))
                            .foregroundStyle(OATheme.Colors.textSecondary)
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
        let s = bridge.outputJSONByCallId[id] ?? bridge.rawJSONByCallId[id] ?? ""
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(s, forType: .string)
        #endif
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

