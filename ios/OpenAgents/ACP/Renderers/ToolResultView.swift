import SwiftUI
import OpenAgentsCore

struct ToolResultView: View {
    let result: ACPToolResult
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Image(systemName: result.ok ? "checkmark.seal" : "xmark.seal")
                    .imageScale(.small)
                    .foregroundStyle(result.ok ? OATheme.Colors.success : OATheme.Colors.danger)
                Text(result.ok ? "Result" : "Error")
                    .font(OAFonts.ui(.subheadline, 13))
                    .foregroundStyle(OATheme.Colors.textPrimary)
            }
            if let err = result.error, !err.isEmpty {
                Text(err)
                    .font(OAFonts.ui(.footnote, 12))
                    .foregroundStyle(OATheme.Colors.danger)
            }
            if let v = result.result, let pretty = try? prettyJSON(v) {
                ScrollView(.horizontal, showsIndicators: false) {
                    Text(pretty)
                        .font(OAFonts.ui(.footnote, 12))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                        .textSelection(.enabled)
                }
            }
        }
    }
}

private func prettyJSON(_ v: JSONValue) throws -> String {
    let data = try JSONEncoder().encode(v)
    let obj = try JSONSerialization.jsonObject(with: data)
    let pd = try JSONSerialization.data(withJSONObject: obj, options: [.prettyPrinted, .sortedKeys])
    return String(data: pd, encoding: .utf8) ?? String(decoding: pd, as: UTF8.self)
}
