import SwiftUI
import OpenAgentsCore

struct ToolResultView: View {
    let result: ACPToolResult

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            // Only show Result/Error header for non-structured data
            if !isStructuredData {
                HStack(spacing: 6) {
                    Image(systemName: result.ok ? "checkmark.seal" : "xmark.seal")
                        .imageScale(.small)
                        .foregroundStyle(result.ok ? OATheme.Colors.success : OATheme.Colors.danger)
                    Text(result.ok ? "Result" : "Error")
                        .font(OAFonts.ui(.subheadline, 13))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                }
            }

            if let err = result.error, !err.isEmpty {
                Text(err)
                    .font(OAFonts.ui(.footnote, 12))
                    .foregroundStyle(OATheme.Colors.danger)
            }

            if let v = result.result {
                // Try to parse and render structured data
                if let todos = TodoListView.parse(from: v) {
                    TodoListView(todos: todos)
                } else if let planState = parsePlanState(from: v) {
                    PlanStateView(state: planState)
                } else if let pretty = try? prettyJSON(v) {
                    // Fallback to raw JSON for unstructured data
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

    /// Check if the result contains structured data that should be rendered as a component
    private var isStructuredData: Bool {
        guard let v = result.result else { return false }
        // Check for known structured formats
        if TodoListView.parse(from: v) != nil { return true }
        if parsePlanState(from: v) != nil { return true }
        return false
    }

    /// Try to parse a plan_state from the result
    private func parsePlanState(from jsonValue: JSONValue) -> ACPPlanState? {
        guard case .object(let dict) = jsonValue else { return nil }
        guard case .string(let type)? = dict["type"], type == "plan_state" else { return nil }

        // Parse status
        guard case .string(let statusStr)? = dict["status"],
              let status = ACPPlanStatus(rawValue: statusStr) else {
            return nil
        }

        // Parse optional fields
        var summary: String? = nil
        if case .string(let s)? = dict["summary"] {
            summary = s
        }

        var steps: [String]? = nil
        if case .array(let stepsArray)? = dict["steps"] {
            steps = stepsArray.compactMap { item in
                if case .string(let step) = item { return step }
                return nil
            }
        }

        var ts: Int64? = nil
        if case .number(let n)? = dict["ts"] {
            ts = Int64(n)
        }

        return ACPPlanState(status: status, summary: summary, steps: steps, ts: ts)
    }
}

private func prettyJSON(_ v: JSONValue) throws -> String {
    let data = try JSONEncoder().encode(v)
    let obj = try JSONSerialization.jsonObject(with: data)
    let pd = try JSONSerialization.data(withJSONObject: obj, options: [.prettyPrinted, .sortedKeys])
    return String(data: pd, encoding: .utf8) ?? String(decoding: pd, as: UTF8.self)
}
