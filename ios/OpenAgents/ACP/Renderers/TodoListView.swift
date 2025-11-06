import SwiftUI
import OpenAgentsCore

/// Structured rendering for TodoWrite tool results
struct TodoListView: View {
    let todos: [TodoItem]

    struct TodoItem: Codable {
        let content: String
        let status: String
        let activeForm: String
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Image(systemName: "checklist")
                    .imageScale(.small)
                    .foregroundStyle(OATheme.Colors.textPrimary)
                Text("Task List")
                    .font(OAFonts.ui(.subheadline, 13))
                    .foregroundStyle(OATheme.Colors.textPrimary)
            }

            VStack(alignment: .leading, spacing: 6) {
                ForEach(todos.indices, id: \.self) { idx in
                    HStack(alignment: .top, spacing: 8) {
                        statusIcon(for: todos[idx].status)
                            .font(.system(size: 14))

                        VStack(alignment: .leading, spacing: 2) {
                            Text(todos[idx].content)
                                .font(OAFonts.ui(.footnote, 13))
                                .foregroundStyle(OATheme.Colors.textPrimary)

                            if todos[idx].status == "in_progress" {
                                Text(todos[idx].activeForm)
                                    .font(OAFonts.ui(.caption, 11))
                                    .foregroundStyle(OATheme.Colors.textSecondary)
                                    .italic()
                            }
                        }
                    }
                    .padding(.vertical, 2)
                }
            }
        }
    }

    @ViewBuilder
    private func statusIcon(for status: String) -> some View {
        switch status {
        case "completed":
            Image(systemName: "checkmark.circle.fill")
                .foregroundStyle(OATheme.Colors.success)
        case "in_progress":
            Image(systemName: "arrow.triangle.2.circlepath.circle")
                .foregroundStyle(.yellow)
        case "pending":
            Image(systemName: "circle")
                .foregroundStyle(OATheme.Colors.textTertiary)
        default:
            Image(systemName: "circle")
                .foregroundStyle(OATheme.Colors.textTertiary)
        }
    }
}

/// Helper to parse TodoWrite results from JSONValue
extension TodoListView {
    static func parse(from jsonValue: JSONValue) -> [TodoItem]? {
        guard case .object(let dict) = jsonValue,
              case .array(let todosArray)? = dict["todos"] else {
            return nil
        }

        return todosArray.compactMap { item -> TodoItem? in
            guard case .object(let todoDict) = item,
                  case .string(let content)? = todoDict["content"],
                  case .string(let status)? = todoDict["status"],
                  case .string(let activeForm)? = todoDict["activeForm"] else {
                return nil
            }

            return TodoItem(content: content, status: status, activeForm: activeForm)
        }
    }
}
