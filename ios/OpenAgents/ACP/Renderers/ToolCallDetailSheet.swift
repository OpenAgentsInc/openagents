import SwiftUI
import OpenAgentsCore

/// Detail sheet for tool calls, showing full params and result JSON
struct ToolCallDetailSheet: View {
    let call: ACPToolCall
    let result: ACPToolResult?
    @Environment(\.dismiss) private var dismiss

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
                            Text("Arguments")
                                .font(OAFonts.ui(.subheadline, 14))
                                .foregroundStyle(OATheme.Colors.textSecondary)

                            if let pretty = try? prettyJSON(call.arguments) {
                                Text(pretty)
                                    .font(OAFonts.mono(.footnote, 12))
                                    .foregroundStyle(OATheme.Colors.textPrimary)
                                    .textSelection(.enabled)
                                    .padding(12)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .background(OATheme.Colors.card.opacity(0.5))
                                    .cornerRadius(8)
                            }
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

                            if let error = result.error, !error.isEmpty {
                                VStack(alignment: .leading, spacing: 8) {
                                    Text("Error")
                                        .font(OAFonts.ui(.subheadline, 14))
                                        .foregroundStyle(OATheme.Colors.textSecondary)

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

                            if let resultValue = result.result {
                                VStack(alignment: .leading, spacing: 8) {
                                    Text("Result Data")
                                        .font(OAFonts.ui(.subheadline, 14))
                                        .foregroundStyle(OATheme.Colors.textSecondary)

                                    if let pretty = try? prettyJSON(resultValue) {
                                        Text(pretty)
                                            .font(OAFonts.mono(.footnote, 12))
                                            .foregroundStyle(OATheme.Colors.textPrimary)
                                            .textSelection(.enabled)
                                            .padding(12)
                                            .frame(maxWidth: .infinity, alignment: .leading)
                                            .background(OATheme.Colors.card.opacity(0.5))
                                            .cornerRadius(8)
                                    }
                                }
                            }
                        }
                    }

                    Spacer()
                }
                .padding(20)
            }
            .navigationTitle("Tool Call Details")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Done") {
                        dismiss()
                    }
                }
            }
        }
    }
}

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
