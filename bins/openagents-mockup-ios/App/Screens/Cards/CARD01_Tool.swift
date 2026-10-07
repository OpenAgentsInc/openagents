import SwiftUI

// CARD-01 Tool card: a tool, what it does, its latest result, and the one
// button that tests it. Replaces the retired SCR-03 page.

enum CARD01State: String, Hashable, CaseIterable {
    case ready, noRunsLeft, offline
}

struct CARD01Tool: View {
    @Environment(MockApp.self) private var app
    let toolID: String
    var state: CARD01State = .ready
    /// Step 2 of 3 in the first-run chat.
    var firstRun = false
    var onStart: () -> Void = {}
    var onRetry: () -> Void = {}
    var onSeeTests: () -> Void = {}
    var onAsk: (String) -> Void = { _ in }

    private var tool: MockData.Tool { MockData.tool(toolID) }
    private var others: [MockData.Tool] { MockData.tools.filter { $0.id != toolID } }

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.Space.s) {
            if state == .offline {
                // PAT-01 inside the card.
                PAT01Inline(icon: "wifi.slash", what: "You're offline.", means: "Tests need the internet.",
                            reassure: "Nothing is lost.", code: "NET-01", onRetry: onRetry)
            } else {
                ChatCardFrame(highlighted: state == .ready) {
                    // E01, E02
                    HStack(spacing: 10) {
                        Image(systemName: tool.icon).font(.paper(20, weight: .semibold))
                        Text(tool.name).condensedTitle(Theme.Fonts.cardTitle)
                    }
                    CardNote(text: tool.line)
                    // E08
                    Text(tool.latest.map { "Latest: \($0)" } ?? "Not tested yet. Be the first.")
                        .font(Theme.Fonts.bodyBold)
                        .fixedSize(horizontal: false, vertical: true)
                    // E03, E04
                    CardNote(text: "\(MockData.testSet(tool.testSet).tests.count) tests, with and without the tool.")
                    CardNote(text: state == .noRunsLeft
                             ? "No runs left today. New runs at \(MockData.newRunsAt) tomorrow."
                             : "About \(MockData.runMinutes) minutes. Free. \(app.runsLeft) runs left today.")
                    // E05
                    PrimaryButton(title: state == .noRunsLeft ? "Start tomorrow at \(MockData.newRunsAt)" : "Start the test",
                                  enabled: state == .ready) { onStart() }
                    // E06, E09 (later)
                    HStack(spacing: 8) {
                        Chip(icon: "list.bullet", text: "See the tests") { onSeeTests() }
                        if MockData.showLaterFeatures && !firstRun {
                            Chip(icon: "info.circle", text: "More about it") { app.go(.toolDetail(toolID)) }
                        }
                    }
                }
            }
            // E07: the other tools, as chips; a check doesn't use a run.
            if !firstRun {
                FlowLayout {
                    if state == .noRunsLeft {
                        Chip(icon: "checkmark.circle", text: "Check a result") { onAsk("checkAResult") }
                    }
                    ForEach(others) { t in
                        Chip(icon: t.icon, text: t.name) { onAsk(testAnswer(t.id)) }
                    }
                }
            }
        }
    }

    private func testAnswer(_ id: String) -> String {
        switch id {
        case "code-finder": "testCodeFinder"
        case "test-reader": "testTestReader"
        default: "testProjectMap"
        }
    }
}

#Preview("CARD-01 Tool") {
    ScrollView { CARD01Tool(toolID: "project-map").padding() }
        .background(Theme.Colors.background).environment(MockApp())
}

#Preview("CARD-01 Not tested yet") {
    ScrollView { CARD01Tool(toolID: "test-reader").padding() }
        .background(Theme.Colors.background).environment(MockApp())
}

#Preview("CARD-01 No runs left") {
    ScrollView { CARD01Tool(toolID: "project-map", state: .noRunsLeft).padding() }
        .background(Theme.Colors.background).environment(MockApp())
}

#Preview("CARD-01 First run") {
    ScrollView { CARD01Tool(toolID: "project-map", firstRun: true).padding() }
        .background(Theme.Colors.background).environment(MockApp())
}

#Preview("CARD-01 Offline") {
    ScrollView { CARD01Tool(toolID: "project-map", state: .offline).padding() }
        .background(Theme.Colors.background).environment(MockApp())
}
