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
                    .fixedSize()

                // For Bash: show command inline, truncated if needed
                if let cmd = inlineCommand {
                    Text(cmd)
                        .font(OAFonts.ui(.footnote, 12))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                        .lineLimit(1)
                        .truncationMode(.tail)
                        .layoutPriority(1)
                }

                Spacer(minLength: 8)

                statusBadge
                    .fixedSize()
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

    // MARK: - Inline Display

    /// Command shown inline next to tool name (Bash only)
    private var inlineCommand: String? {
        return ShellCommandFormatter.format(call: call)
    }

    /// Details shown below tool name
    private var inlineParams: String? {
        let toolName = call.tool_name.lowercased()
        let args = unwrapArgumentsJSON(call.arguments)

        // Bash/Shell - show description
        if toolName == "bash" || toolName == "shell" || toolName.hasSuffix(".shell") {
            if case .object(let obj) = args,
               case .string(let desc)? = obj["description"] {
                return desc
            }
            return nil
        }

        // Codex delegation summary
        if toolName == "codex.run" {
            if case .object(let obj) = args {
                var parts: [String] = []
                if case .string(let task)? = obj["task"] { parts.append(task) }
                if case .string(let desc)? = obj["description"] { parts.append(desc) }
                if case .string(let up)? = obj["user_prompt"] { parts.append(String(up.prefix(60))) }
                if !parts.isEmpty { return parts.joined(separator: " — ") }
            }
        }

        // Read - show relative file_path
        if toolName == "read" || toolName.hasSuffix(".read") {
            if case .object(let obj) = args,
               case .string(let path)? = obj["file_path"] {
                return "📄 \(makeRelativePath(path))"
            }
        }

        // Write - show relative file_path
        if toolName == "write" || toolName.hasSuffix(".write") {
            if case .object(let obj) = args,
               case .string(let path)? = obj["file_path"] {
                return "✏️ \(makeRelativePath(path))"
            }
        }

        // Edit - show relative file_path
        if toolName == "edit" || toolName.hasSuffix(".edit") {
            if case .object(let obj) = args,
               case .string(let path)? = obj["file_path"] {
                return "✏️ \(makeRelativePath(path))"
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

    /// Convert absolute path to relative (using ~ for home directory)
    private func makeRelativePath(_ absolutePath: String) -> String {
        // Replace home directory with ~
        let homeDir = NSHomeDirectory()
        if absolutePath.hasPrefix(homeDir) {
            let relativePart = String(absolutePath.dropFirst(homeDir.count))
            return "~\(relativePart)"
        }
        return absolutePath
    }
}

private func prettyJSON(_ v: JSONValue) throws -> String {
    let data = try JSONEncoder().encode(v)
    let obj = try JSONSerialization.jsonObject(with: data)
    let pd = try JSONSerialization.data(withJSONObject: obj, options: [.prettyPrinted, .sortedKeys])
    return String(data: pd, encoding: .utf8) ?? String(decoding: pd, as: UTF8.self)
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
