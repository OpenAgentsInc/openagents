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

    // Pre-compute JSON strings to avoid pasteboard timeout
    private let argumentsJSON: String
    private let resultJSON: String?
    private let errorText: String?

    init(call: ACPToolCall, result: ACPToolResult?) {
        self.call = call
        self.result = result

        // Pre-compute arguments JSON (sync, but only once)
        self.argumentsJSON = (try? prettyJSON(call.arguments)) ?? "{}"

        // Pre-compute result JSON if available
        if let resultValue = result?.result {
            self.resultJSON = try? prettyJSON(resultValue)
        } else {
            self.resultJSON = nil
        }

        self.errorText = result?.error
    }

    var body: some View {
        NavigationView {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    // Tool Call Section
                    VStack(alignment: .leading, spacing: 12) {
                        Text("Tool Call")
                            .font(OAFonts.ui(.headline, 16))
                            .foregroundStyle(OATheme.Colors.textPrimary)

                        VStack(alignment: .leading, spacing: 8) {
                            DetailRow(label: "ID", value: call.id)
                            DetailRow(label: "Tool", value: call.tool_name)
                            if let ts = call.ts {
                                DetailRow(label: "Timestamp", value: String(ts))
                            }
                        }

                        VStack(alignment: .leading, spacing: 8) {
                            HStack {
                                Text("Arguments")
                                    .font(OAFonts.ui(.subheadline, 14))
                                    .foregroundStyle(OATheme.Colors.textSecondary)
                                Spacer()
                                Button(action: { copyToClipboard(argumentsJSON) }) {
                                    Label("Copy", systemImage: "doc.on.doc")
                                        .font(OAFonts.ui(.caption, 12))
                                }
                            }

                            JSONTextView(text: argumentsJSON)
                        }
                    }

                    // Result Section
                    if let result = result {
                        Divider()

                        VStack(alignment: .leading, spacing: 12) {
                            Text("Tool Result")
                                .font(OAFonts.ui(.headline, 16))
                                .foregroundStyle(OATheme.Colors.textPrimary)

                            VStack(alignment: .leading, spacing: 8) {
                                DetailRow(label: "Call ID", value: result.call_id)
                                DetailRow(
                                    label: "Status",
                                    value: result.ok ? "Success" : "Error",
                                    valueColor: result.ok ? OATheme.Colors.success : OATheme.Colors.danger
                                )
                                if let ts = result.ts {
                                    DetailRow(label: "Timestamp", value: String(ts))
                                }
                            }

                            if let error = errorText, !error.isEmpty {
                                VStack(alignment: .leading, spacing: 8) {
                                    HStack {
                                        Text("Error")
                                            .font(OAFonts.ui(.subheadline, 14))
                                            .foregroundStyle(OATheme.Colors.textSecondary)
                                        Spacer()
                                        Button(action: { copyToClipboard(error) }) {
                                            Label("Copy", systemImage: "doc.on.doc")
                                                .font(OAFonts.ui(.caption, 12))
                                        }
                                    }

                                    Text(error)
                                        .font(OAFonts.mono(.footnote, 12))
                                        .foregroundStyle(OATheme.Colors.danger)
                                        .textSelection(.enabled)
                                        .padding(12)
                                        .frame(maxWidth: .infinity, alignment: .leading)
                                        .background(OATheme.Colors.danger.opacity(0.1))
                                        .cornerRadius(8)
                                }
                            }

                            if let resultJSON = resultJSON {
                                VStack(alignment: .leading, spacing: 8) {
                                    HStack {
                                        Text("Result Data")
                                            .font(OAFonts.ui(.subheadline, 14))
                                            .foregroundStyle(OATheme.Colors.textSecondary)
                                        Spacer()
                                        Button(action: { copyToClipboard(resultJSON) }) {
                                            Label("Copy", systemImage: "doc.on.doc")
                                                .font(OAFonts.ui(.caption, 12))
                                        }
                                    }

                                    JSONTextView(text: resultJSON)
                                }
                            }
                        }
                    }

                    Spacer()
                }
                .padding(20)
            }
            .navigationTitle("Tool Call Details")
            #if os(iOS)
            .navigationBarTitleDisplayMode(.inline)
            #endif
            .toolbar {
                #if os(iOS)
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Done") {
                        dismiss()
                    }
                }
                #else
                ToolbarItem(placement: .automatic) {
                    Button("Done") {
                        dismiss()
                    }
                }
                #endif
            }
        }
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

            Text(value)
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
