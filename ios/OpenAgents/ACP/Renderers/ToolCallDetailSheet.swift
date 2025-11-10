import SwiftUI
import OpenAgentsCore
#if os(macOS)
import AppKit
#endif

/// Detail sheet for tool calls, showing full params and result JSON
struct ToolCallDetailSheet: View {
    let call: ACPToolCall
    let result: ACPToolResult?
    @Environment(\.dismiss) private var dismiss
    @EnvironmentObject private var bridge: BridgeManager

    // Resolve arguments from the call or from the timeline (when update rows lack args)
    private var resolvedArgumentsJSON: String {
        if let s = try? prettyJSON(call.arguments), s != "{}" { return s }
        if let wire = bridge.updates.compactMap({ note -> ACPToolCallWire? in
            if case .toolCall(let w) = note.update, w.call_id == call.id { return w }
            return nil
        }).first, let args = wire.arguments {
            var obj: [String: JSONValue] = [:]
            for (k, v) in args { obj[k] = v.toJSONValue() }
            if let s = try? prettyJSON(.object(obj)) { return s }
        }
        return "{}"
    }

    private var resolvedResultJSON: String? {
        if let r = result?.result, let s = try? prettyJSON(r) { return s }
        if let s = bridge.outputJSONByCallId[call.id], !s.isEmpty { return s }
        return nil
    }

    private var errorText: String? { result?.error }

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                VStack(alignment: .leading, spacing: 4) {
                    Text(call.tool_name)
                        .font(OAFonts.ui(.headline, 18))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                    HStack(spacing: 10) {
                        DetailBadge(label: "ID", value: call.id)
                        if let ts = call.ts { DetailBadge(label: "TS", value: String(ts)) }
                        if let r = result { DetailBadge(label: "Status", value: r.ok ? "completed" : "error", color: r.ok ? OATheme.Colors.success : OATheme.Colors.danger) }
                    }
                }
                Spacer()
                HStack(spacing: 8) {
                    Button("Copy raw JSON") { copyRawJSON() }
                        .buttonStyle(.plain)
                        .font(OAFonts.ui(.caption, 12))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                    Button("Done") { dismiss() }
                        .buttonStyle(.plain)
                        .font(OAFonts.ui(.caption, 12))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                }
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 14)
            .background(OATheme.Colors.background)

            Divider().overlay(OATheme.Colors.border)

            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    // Arguments
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Text("Arguments")
                                .font(OAFonts.ui(.subheadline, 14))
                                .foregroundStyle(OATheme.Colors.textSecondary)
                            Spacer()
                            Button(action: { copyToClipboard(resolvedArgumentsJSON) }) {
                                Label("Copy", systemImage: "doc.on.doc")
                                    .font(OAFonts.ui(.caption, 12))
                            }
                            .buttonStyle(.plain)
                            .foregroundStyle(OATheme.Colors.textSecondary)
                        }
                        if resolvedArgumentsJSON.trimmingCharacters(in: .whitespacesAndNewlines) == "{}" {
                            Text("(no structured arguments)")
                                .font(OAFonts.mono(.caption, 11))
                                .foregroundStyle(OATheme.Colors.textTertiary)
                        }
                        JSONTextView(text: resolvedArgumentsJSON)
                    }

                    // Result
                    if let resultJSON = resolvedResultJSON {
                        VStack(alignment: .leading, spacing: 8) {
                            HStack {
                                Text("Output")
                                    .font(OAFonts.ui(.subheadline, 14))
                                    .foregroundStyle(OATheme.Colors.textSecondary)
                                Spacer()
                                Button(action: { copyToClipboard(resultJSON) }) {
                                    Label("Copy", systemImage: "doc.on.doc")
                                        .font(OAFonts.ui(.caption, 12))
                                }
                                .buttonStyle(.plain)
                                .foregroundStyle(OATheme.Colors.textSecondary)
                            }
                            JSONTextView(text: resultJSON)
                        }
                    } else {
                        HStack(spacing: 8) {
                            ProgressView().scaleEffect(0.6)
                            Text("Awaiting result…")
                                .font(OAFonts.mono(.footnote, 12))
                                .foregroundStyle(OATheme.Colors.textSecondary)
                        }
                    }

                    if let error = errorText, !error.isEmpty {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Error")
                                .font(OAFonts.ui(.subheadline, 14))
                                .foregroundStyle(OATheme.Colors.textSecondary)
                            Text(error)
                                .font(OAFonts.mono(.footnote, 12))
                                .foregroundStyle(OATheme.Colors.danger)
                                .textSelection(.enabled)
                        }
                    }
                }
                .padding(20)
            }
        }
        .frame(minWidth: 820, minHeight: 560)
        .background(OATheme.Colors.background)
    }

    // MARK: - Helper Methods
    private func copyToClipboard(_ text: String) {
        #if os(iOS)
        UIPasteboard.general.string = text
        #elseif os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        #endif
    }

    private func copyRawJSON() {
        // Prefer raw tool_call_update from timeline; else synthesize combined object
        if let raw = bridge.rawJSONByCallId[call.id] { copyToClipboard(raw); return }
        struct ToolCallWithResult: Codable { let call: ACPToolCall; let result: ACPToolResult? }
        let combined = ToolCallWithResult(call: call, result: result)
        let json = encodeJSONPretty(combined)
        copyToClipboard(json)
    }

    private func encodeJSONPretty<T: Encodable>(_ value: T) -> String {
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys]
        if let d = try? enc.encode(value), let s = String(data: d, encoding: .utf8) { return s }
        return "{}"
    }
}

// MARK: - JSON Text View

/// Efficient text view for large JSON content with text selection enabled
private struct JSONTextView: View {
    let text: String

    var body: some View {
        Text(text)
            .font(OAFonts.mono(.footnote, 12))
            .foregroundStyle(OATheme.Colors.textPrimary)
            .textSelection(.enabled)
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(OATheme.Colors.card.opacity(0.5))
            .cornerRadius(8)
    }
}

// MARK: - Detail Row

private struct DetailRow: View {
    let label: String
    let value: String
    var valueColor: Color = OATheme.Colors.textPrimary

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Text(label + ":")
                .font(OAFonts.ui(.subheadline, 13))
                .foregroundStyle(OATheme.Colors.textSecondary)
                .frame(width: 80, alignment: .leading)

            Text(value.isEmpty ? "—" : value)
                .font(OAFonts.mono(.subheadline, 13))
                .foregroundStyle(valueColor)
                .textSelection(.enabled)

            Spacer()
        }
    }
}

private func prettyJSON(_ v: JSONValue) throws -> String {
    let data = try JSONEncoder().encode(v)
    let obj = try JSONSerialization.jsonObject(with: data)
    let pd = try JSONSerialization.data(withJSONObject: obj, options: [.prettyPrinted, .sortedKeys])
    return String(data: pd, encoding: .utf8) ?? String(decoding: pd, as: UTF8.self)
}

// MARK: - Badges

private struct DetailBadge: View {
    let label: String
    let value: String
    var color: Color = OATheme.Colors.textSecondary
    var body: some View {
        HStack(spacing: 6) {
            Text(label)
                .font(OAFonts.ui(.caption, 11))
                .foregroundStyle(OATheme.Colors.textTertiary)
            Text(value.isEmpty ? "—" : value)
                .font(OAFonts.mono(.caption, 11))
                .foregroundStyle(color)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(OATheme.Colors.card.opacity(0.5))
        .cornerRadius(6)
    }
}

