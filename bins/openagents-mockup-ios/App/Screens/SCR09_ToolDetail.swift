import SwiftUI

// SCR-09 Tool detail (later).

struct SCR09ToolDetail: View {
    @Environment(MockApp.self) private var app
    let toolID: String

    private var tool: MockData.Tool { MockData.tool(toolID) }

    var body: some View {
        ScreenScaffold {
            TopBar(back: BackControl(label: "Tools") { app.back() }, title: tool.name)
        } content: {
            Image(systemName: tool.icon).font(.system(size: 44, weight: .semibold))
                .frame(maxWidth: .infinity).padding(.vertical, Theme.Space.s)
            // E01
            Text(tool.line).font(Theme.Fonts.title).fixedSize(horizontal: false, vertical: true)
            // E02
            Card {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Across all trainers:").font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                    let o = MockData.outcome(MockData.outcomeKey(forTool: tool.id))
                    HStack(spacing: Theme.Space.s) {
                        Text("\(o.withoutCount) of \(o.total)")
                        Image(systemName: "arrow.right")
                        Text("\(o.withCount) of \(o.total)")
                    }
                    .font(Theme.Fonts.hugeNumber)
                    Text("tests · \(tool.testedBy) trainers · \(tool.runs) runs · \(tool.checks) checks")
                        .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                }
            }
            // E03
            Label("It only reads the project. It can't change files or go online.", systemImage: "lock.shield")
                .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
        } bottom: {
            // E04, E05, E06
            NextLine(text: "test this tool on Coder.")
            // E05: chat with CARD-01 for this tool.
            PrimaryButton(title: "Test this tool") { app.go(.conversation(.card(.tool(tool.id, .ready)))) }
            SecondaryLink(title: "Ask about this tool") { app.go(.conversation(.answer("tool"))) }
        }
    }
}

#Preview("SCR-09 Tool detail") {
    NavigationStack { SCR09ToolDetail(toolID: "project-map") }.environment(MockApp())
}
