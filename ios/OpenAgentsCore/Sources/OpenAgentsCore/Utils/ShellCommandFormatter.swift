import Foundation

/// Utility for formatting and parsing shell command tool calls
/// Extracted from UI layer to enable reuse and testing
public struct ShellCommandFormatter {

    /// Format a shell command tool call into a readable string
    /// - Parameter call: The tool call to format
    /// - Returns: Formatted command string, or nil if not a shell command
    public static func format(call: ACPToolCall) -> String? {
        let name = call.tool_name.lowercased()
        guard name == "bash" || name == "shell" || name.hasSuffix(".shell") else { return nil }

        let args = unwrapArgumentsJSON(call.arguments)
        guard let parts = parseCommandArray(from: args) else { return nil }

        // Special case: bash -lc "command" -> just show the command
        if parts.count >= 3 && parts[0] == "bash" && parts[1] == "-lc" {
            return parts[2]
        }

        // Generic: join with quoting whitespace args
        let joined = parts.map { part in
            if part.contains(" ") || part.contains("\t") {
                return "\"\(part)\""
            } else {
                return part
            }
        }.joined(separator: " ")

        return joined
    }

    /// Parse command array from tool call arguments
    /// - Parameter args: JSONValue arguments
    /// - Returns: Array of command parts, or nil if parsing fails
    public static func parseCommandArray(from args: JSONValue) -> [String]? {
        switch args {
        case .object(let obj):
            // Direct command array in object
            if case let .array(arr)? = obj["command"] {
                return arr.compactMap { value in
                    switch value {
                    case .string(let s): return s
                    case .number(let n): return String(n)
                    case .bool(let b): return b ? "true" : "false"
                    default: return nil
                    }
                }
            }

            // Nested JSON string with command array
            if case let .string(jsonStr)? = obj["arguments"],
               let data = jsonStr.data(using: .utf8),
               let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let arr = dict["command"] as? [Any] {
                return arr.compactMap { item in
                    if let s = item as? String { return s }
                    if let n = item as? NSNumber { return n.stringValue }
                    if let b = item as? Bool { return b ? "true" : "false" }
                    return nil
                }
            }

            return nil

        case .string(let jsonStr):
            // Top-level JSON string
            if let data = jsonStr.data(using: .utf8),
               let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
               let arr = dict["command"] as? [Any] {
                return arr.compactMap { item in
                    if let s = item as? String { return s }
                    if let n = item as? NSNumber { return n.stringValue }
                    return nil
                }
            }
            return nil

        default:
            return nil
        }
    }

    /// Unwrap nested JSON arguments if present
    /// Some tool calls have arguments wrapped in a JSON string
    private static func unwrapArgumentsJSON(_ value: JSONValue) -> JSONValue {
        if case .object(let obj) = value,
           let inner = obj["arguments"],
           case .string(let jsonStr) = inner {
            if let data = jsonStr.data(using: .utf8),
               let dict = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
                var result: [String: JSONValue] = [:]
                for (key, val) in dict {
                    if let s = val as? String {
                        result[key] = .string(s)
                    } else if let n = val as? NSNumber {
                        result[key] = .number(n.doubleValue)
                    } else if let b = val as? Bool {
                        result[key] = .bool(b)
                    }
                }
                return .object(result)
            }
        }
        return value
    }
}
