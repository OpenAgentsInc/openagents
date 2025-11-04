import SwiftUI
import OpenAgentsCore

struct ToolCallView: View {
    let call: ACPToolCall
    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Image(systemName: "wrench.and.screwdriver")
                    .imageScale(.small)
                    .foregroundStyle(OATheme.Colors.textSecondary)
                Text("Tool: \(call.tool_name)")
                    .font(BerkeleyFont.font(relativeTo: .subheadline, size: 13))
                    .foregroundStyle(OATheme.Colors.textPrimary)
            }
            if let pretty = try? prettyJSON(call.arguments) {
                ScrollView(.horizontal, showsIndicators: false) {
                    Text(pretty)
                        .font(BerkeleyFont.font(relativeTo: .footnote, size: 12))
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

