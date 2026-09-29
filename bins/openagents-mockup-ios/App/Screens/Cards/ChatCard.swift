import SwiftUI

// Chat cards (spec: "Chat cards and sheets"). A card is drawn by the app
// inside a reply, with the app's own labels and one button; a newer card of
// the same thing (a run finishing) replaces the older one in place.
// One file per card ID: CARD01_Tool.swift … CARD07_Credit.swift. This file
// holds what they share: the card a chat message carries, and its frame.

/// The card a chat message carries, with its live state.
enum ChatCard: Hashable {
    case tool(String, CARD01State)            // a tool id
    case draft(CARD02Step)
    case run(RunKind, CARD03State, Date)      // what runs, its state, when it started
    case result(String, added: Bool)          // an outcome key (MockData.outcomes)
    case news(CARD05State)
    case check(CARD06State)
    case credit(CARD07State)

    /// The card an answer asks for, in its first state.
    init(_ card: MockData.Card, runsLeft: Int) {
        switch card {
        case .tool(let id): self = .tool(id, runsLeft > 0 ? .ready : .noRunsLeft)
        case .draft: self = .draft(.tests)
        case .result(let key): self = .result(key, added: false)
        case .news(let empty): self = .news(empty ? .empty : .items)
        case .check: self = .check(.ready)
        case .credit(let empty): self = .credit(empty ? .empty : .rows)
        }
    }

    /// The spec ID, for comments and the Screen index.
    var specID: String {
        switch self {
        case .tool: "CARD-01"
        case .draft: "CARD-02"
        case .run: "CARD-03"
        case .result: "CARD-04"
        case .news: "CARD-05"
        case .check: "CARD-06"
        case .credit: "CARD-07"
        }
    }
}

/// What a CARD-03 run is running.
enum RunKind: Hashable {
    /// A starter tool's test set, with and without the tool.
    case tool(String)
    /// Another trainer's result, rerun (CARD-06).
    case check
    /// The player's draft, one run (FLOW-07 "Try it once").
    case tryOnce
    /// The player's draft, the full test set.
    case fullDraft

    var title: String {
        switch self {
        case .tool(let id): "Testing \(MockData.tool(id).name)"
        case .check: "Checking \(MockData.checkTrainer)'s result"
        case .tryOnce: "Trying \(MockData.madeToolName) once"
        case .fullDraft: "Testing \(MockData.madeToolName)"
        }
    }

    var toolName: String {
        switch self {
        case .tool(let id): MockData.tool(id).name
        case .check: MockData.checkTool
        case .tryOnce, .fullDraft: MockData.madeToolName
        }
    }

    var testSet: String {
        switch self {
        case .tool(let id): MockData.tool(id).testSet
        case .check: MockData.checkTestSet
        case .tryOnce, .fullDraft: "changelog"
        }
    }

    var total: Int { MockData.testSet(testSet).tests.count }

    /// A one-run try is quicker than three runs each way.
    var seconds: Double { self == .tryOnce ? MockData.fakeRunSeconds / 2 : MockData.fakeRunSeconds }

    var minutes: Int { self == .tryOnce ? 2 : MockData.runMinutes }

    var outcomeKey: String {
        switch self {
        case .tool(let id): MockData.outcomeKey(forTool: id)
        case .check: "confirmed"
        case .tryOnce: "firstTry"
        case .fullDraft: "madeBetter"
        }
    }

    /// Checks and first tries don't use one of today's runs.
    var spendsRun: Bool {
        switch self {
        case .tool, .fullDraft: true
        case .check, .tryOnce: false
        }
    }
}

/// The frame every chat card sits in: a raised surface, a hairline, and
/// room for the card's one primary button.
struct ChatCardFrame<Content: View>: View {
    var highlighted = false
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: Theme.Space.s) {
            content
        }
        .padding(Theme.Space.m)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: Theme.Radius.card).fill(Theme.Colors.chatCard))
        .overlay(
            RoundedRectangle(cornerRadius: Theme.Radius.card)
                .stroke(highlighted ? Theme.Colors.strokeStrong : Theme.Colors.stroke,
                        lineWidth: highlighted ? Theme.Stroke.selected : Theme.Stroke.hairline)
        )
    }
}

/// A card's small gray line (sources, how tests are checked, time and cost).
struct CardNote: View {
    let text: String
    var tertiary = false

    var body: some View {
        Text(text)
            .font(tertiary ? Theme.Fonts.caption : Theme.Fonts.body)
            .foregroundStyle(tertiary ? Theme.Colors.textTertiary : Theme.Colors.textSecondary)
            .fixedSize(horizontal: false, vertical: true)
    }
}

/// ✓ or ✗ for one test on one side (SCR-05.E12, CARD-02 marks).
struct TestMark: View {
    let passed: Bool

    var body: some View {
        Image(systemName: passed ? "checkmark" : "xmark")
            .font(.system(size: 14, weight: .heavy))
            .foregroundStyle(passed ? Theme.Colors.markPass : Theme.Colors.markFail)
            .frame(width: 22)
            .accessibilityLabel(passed ? "passed" : "not passed")
    }
}
