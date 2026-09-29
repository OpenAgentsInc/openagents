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
                    HStack(spacing: Theme.Space.s) {
                        Text("\(tool.pooledBefore) of 10")
                        Image(systemName: "arrow.right")
                        Text("\(tool.pooledAfter) of 10")
                    }
                    .font(Theme.Fonts.hugeNumber)
                    Text("\(tool.triedBy) trainers · \(tool.runs) runs · \(tool.status == "Helps" ? "confirmed" : "being tested")")
                        .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                }
            }
            // E03
            Label("It only reads the project. It can't change files or go online.", systemImage: "lock.shield")
                .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
        } bottom: {
            // E04, E05, E06
            NextLine(text: "train Coder with this tool.")
            PrimaryButton(title: "Train with this tool") {
                app.selectedToolID = tool.id
                app.go(.gym(.returning))
            }
            SecondaryLink(title: "Ask about this tool") { app.go(.conversation(.aboutTool)) }
        }
    }
}

#Preview("SCR-09 Tool detail") {
    NavigationStack { SCR09ToolDetail(toolID: "project-map") }.environment(MockApp())
}
