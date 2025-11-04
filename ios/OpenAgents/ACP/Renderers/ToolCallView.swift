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
            if let cmd = prettyShellCommand(call: call) {
                Text(cmd)
                    .font(BerkeleyFont.font(relativeTo: .footnote, size: 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)
                    .lineLimit(1)
                    .textSelection(.enabled)
            } else if let pretty = try? prettyJSON(call.arguments) {
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

// MARK: - Shell command prettifier
private func prettyShellCommand(call: ACPToolCall) -> String? {
    let name = call.tool_name.lowercased()
    guard name == "shell" || name.hasSuffix(".shell") else { return nil }
    guard let parts = parseCommandArray(from: call.arguments) else { return nil }
    if parts.count >= 3 && parts[0] == "bash" && parts[1] == "-lc" {
        return parts[2]
    }
    // Generic join with quoting whitespace args
    let joined = parts.map { p in
        if p.contains(" ") || p.contains("\t") { return "\"\(p)\"" } else { return p }
    }.joined(separator: " ")
    return joined
}

private func parseCommandArray(from args: JSONValue) -> [String]? {
    switch args {
    case .object(let obj):
        if case let .array(arr)? = obj["command"] {
            return arr.compactMap { v in
                switch v {
                case .string(let s): return s
                case .number(let n): return String(n)
                case .bool(let b): return b ? "true" : "false"
                default: return nil
                }
            }
        }
        return nil
    case .string(let s):
        if let data = s.data(using: .utf8),
           let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let arr = dict["command"] as? [Any] {
            return arr.compactMap { a in
                if let s = a as? String { return s }
                if let n = a as? NSNumber { return n.stringValue }
                return nil
            }
        }
        return nil
    default:
        return nil
    }
}
