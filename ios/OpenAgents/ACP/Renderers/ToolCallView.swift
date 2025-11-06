import SwiftUI
import OpenAgentsCore

struct ToolCallView: View {
    let call: ACPToolCall
    let result: ACPToolResult? // Optional result for status determination
    @State private var showingDetail = false

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                statusIcon
                    .imageScale(.small)

                Text(call.tool_name)
                    .font(OAFonts.ui(.subheadline, 13))
                    .foregroundStyle(OATheme.Colors.textPrimary)

                Spacer()

                statusBadge
            }

            if let inline = inlineParams {
                Text(inline)
                    .font(OAFonts.ui(.footnote, 12))
                    .foregroundStyle(OATheme.Colors.textSecondary)
                    .lineLimit(1)
                    .textSelection(.enabled)
            }
        }
        .contentShape(Rectangle())
        .onTapGesture {
            showingDetail = true
        }
        .sheet(isPresented: $showingDetail) {
            ToolCallDetailSheet(call: call, result: result)
        }
    }

    // MARK: - Status UI

    private var statusIcon: some View {
        Group {
            if let result = result {
                if result.ok {
                    Image(systemName: "checkmark.circle.fill")
                        .foregroundStyle(OATheme.Colors.success)
                } else {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(OATheme.Colors.danger)
                }
            } else {
                Image(systemName: "clock.circle")
                    .foregroundStyle(.yellow)
            }
        }
    }

    private var statusBadge: some View {
        Group {
            if let result = result {
                if result.ok {
                    Text("completed")
                        .font(OAFonts.ui(.caption, 10))
                        .foregroundStyle(OATheme.Colors.success)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(OATheme.Colors.success.opacity(0.1))
                        .cornerRadius(4)
                } else {
                    Text("error")
                        .font(OAFonts.ui(.caption, 10))
                        .foregroundStyle(OATheme.Colors.danger)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(OATheme.Colors.danger.opacity(0.1))
                        .cornerRadius(4)
                }
            } else {
                Text("pending")
                    .font(OAFonts.ui(.caption, 10))
                    .foregroundStyle(.yellow)
                    .padding(.horizontal, 6)
                    .padding(.vertical, 2)
                    .background(Color.yellow.opacity(0.1))
                    .cornerRadius(4)
            }
        }
    }

    // MARK: - Inline Params

    private var inlineParams: String? {
        let toolName = call.tool_name.lowercased()
        let args = unwrapArgumentsJSON(call.arguments)

        // Bash/Shell - show command
        if toolName == "bash" || toolName == "shell" || toolName.hasSuffix(".shell") {
            return prettyShellCommand(call: call)
        }

        // Read - show file_path
        if toolName == "read" || toolName.hasSuffix(".read") {
            if case .object(let obj) = args,
               case .string(let path)? = obj["file_path"] {
                return "📄 \(path)"
            }
        }

        // Write - show file_path
        if toolName == "write" || toolName.hasSuffix(".write") {
            if case .object(let obj) = args,
               case .string(let path)? = obj["file_path"] {
                return "✏️ \(path)"
            }
        }

        // Edit - show file_path
        if toolName == "edit" || toolName.hasSuffix(".edit") {
            if case .object(let obj) = args,
               case .string(let path)? = obj["file_path"] {
                return "✏️ \(path)"
            }
        }

        // Glob - show pattern
        if toolName == "glob" || toolName.hasSuffix(".glob") {
            if case .object(let obj) = args,
               case .string(let pattern)? = obj["pattern"] {
                return "🔍 \(pattern)"
            }
        }

        // Grep - show pattern
        if toolName == "grep" || toolName.hasSuffix(".grep") {
            if case .object(let obj) = args,
               case .string(let pattern)? = obj["pattern"] {
                return "🔍 \(pattern)"
            }
        }

        return nil
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
    let args = unwrapArgumentsJSON(call.arguments)
    guard let parts = parseCommandArray(from: args) else { return nil }
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
        } else if case let .string(s)? = obj["arguments"],
                  let data = s.data(using: .utf8),
                  let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let arr = dict["command"] as? [Any] {
            return arr.compactMap { a in
                if let s = a as? String { return s }
                if let n = a as? NSNumber { return n.stringValue }
                if let b = a as? Bool { return b ? "true" : "false" }
                return nil
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

private func unwrapArgumentsJSON(_ v: JSONValue) -> JSONValue {
    if case .object(let obj) = v, let inner = obj["arguments"], case .string(let s) = inner {
        if let data = s.data(using: .utf8),
           let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            var out: [String: JSONValue] = [:]
            for (k, val) in dict {
                if let sv = val as? String { out[k] = .string(sv) }
                else if let n = val as? NSNumber { out[k] = .number(n.doubleValue) }
                else if let b = val as? Bool { out[k] = .bool(b) }
            }
            return .object(out)
        }
    }
    return v
}
