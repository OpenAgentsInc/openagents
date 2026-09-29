import SwiftUI

// SCR-08 All tools (later). Every row does one thing: open the tool.

enum SCR08State: String, Hashable, CaseIterable {
    case normal, offline
}

struct SCR08AllTools: View {
    @Environment(MockApp.self) private var app
    let state: SCR08State

    var body: some View {
        ScreenScaffold {
            TopBar(back: BackControl(label: "Gym") { app.back() }, title: "All tools")
        } content: {
            // E01
            Text("Pick a tool to see what it does.").font(Theme.Fonts.body)
                .foregroundStyle(Theme.Colors.textSecondary)
            // E02
            VStack(spacing: 0) {
                ForEach(MockData.tools) { tool in
                    ListRow(icon: tool.icon, title: tool.name.uppercased(),
                            trailing: state == .offline ? "Can't check right now"
                                : (tool.status == "Helps" ? "Helps ✓" : tool.status)) {
                        app.go(.toolDetail(tool.id))
                    }
                    Divider().overlay(Theme.Colors.divider)
                }
                // E03
                ListRow(icon: "lock", title: "More tools soon", chevron: false)
                    .opacity(0.5)
            }
        } bottom: {
            EmptyView()
        }
    }
}

#Preview("SCR-08 All tools") {
    NavigationStack { SCR08AllTools(state: .normal) }.environment(MockApp())
}
