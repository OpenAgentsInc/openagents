// The Grid Gym's RESULTS board: the published Terminal-Bench results in the
// OpenAgents app's white-on-black style. Rust loads and verifies the
// publication, owns where the player is in the panel, and formats every
// figure with its labels (`gym_leaderboard::view`); this panel draws one
// screen's rows and sends the player's choices back. It never computes,
// sums, or relabels a number.
import SwiftUI

/// The `results_view` Rust sends when the host asks by revision.
struct ResultsView: Decodable {
    let revision: UInt64
    let active: Bool
    let loading: Bool
    let status: String
    let error: String?
    /// Where the open trace's replay ghost stands in the Gym.
    let replay: String?
    let can_back: Bool
    let page: ResultsPage?
}

enum ResultsPage: Decodable {
    case boards(ResultsBoardsPage)
    case board(ResultsBoardPage)
    case attempt(ResultsAttemptPage)
    case trace(ResultsTracePage)
    case unknown

    private enum Key: String, CodingKey { case screen }

    init(from decoder: Decoder) throws {
        let screen = try decoder.container(keyedBy: Key.self).decode(String.self, forKey: .screen)
        switch screen {
        case "boards": self = .boards(try ResultsBoardsPage(from: decoder))
        case "board": self = .board(try ResultsBoardPage(from: decoder))
        case "attempt": self = .attempt(try ResultsAttemptPage(from: decoder))
        case "trace": self = .trace(try ResultsTracePage(from: decoder))
        default: self = .unknown
        }
    }
}

struct ResultsChip: Decodable, Hashable { let code: String; let text: String }
struct ResultsCaveat: Decodable, Hashable { let code: String; let text: String }
struct ResultsReference: Decodable { let name: String; let rule: String; let conditions: String }

struct ResultsBoardsPage: Decodable {
    /// One board's plain-language sentence, built in Rust; nil when no
    /// board has a beat.
    let summary: Summary?
    let rows: [Row]
    struct Summary: Decodable { let board: String; let text: String; let source: String }
    let footer: String?
    struct Row: Decodable, Identifiable {
        let id: String
        let title: String
        let benchmark: String
        let headline: String
        let headline_note: String?
        let labels: [ResultsChip]
        let accessibility: String
    }
}

struct ResultsBoardPage: Decodable {
    let id: String
    let title: String
    let benchmark: String
    let question: String
    let summary: String
    let headline: String
    let headline_note: String?
    let labels: [ResultsChip]
    let tallies: [Tally]
    let spend: [String]
    let reference: ResultsReference
    let caveat_count: Int
    let caveats: [ResultsCaveat]
    let caveats_open: Bool
    let filter: String
    let filters: [Filter]
    let tasks: [Task]

    struct Tally: Decodable, Hashable { let name: String; let text: String }
    struct Filter: Decodable, Hashable { let filter: String; let text: String; let count: Int; let selected: Bool }
    struct Task: Decodable, Identifiable {
        var id: String { task }
        let task: String
        let bar: String
        let knowledge: String
        let status: String
        let attempts: [Cell]
        let accessibility: String
    }
    struct Cell: Decodable, Identifiable {
        let id: String
        let series: String
        let passed: Bool
        let beat: Bool
        let text: String
        let labels: [ResultsChip]
        let caveats: [ResultsCaveat]
        let accessibility: String
    }
}

struct ResultsHeader: Decodable {
    let task: String
    let result: String
    let beat: String
    let cost: String
    let time: String
    let labels: [ResultsChip]
    let accessibility: String
}

struct ResultsAttemptPage: Decodable {
    let id: String
    let board_title: String
    let header: ResultsHeader
    let series: String
    let trial: String
    let numbers: [String]
    let misses: String
    let reference: ResultsReference
    let phases: [String]
    let how_it_ended: String?
    let jev: String?
    let verifier: String?
    let failed_tests: [String]
    let caveats: [ResultsCaveat]
    let trace: String?
    let accessibility: String
}

/// The results panel. Each screen is its own section over the same
/// Rust-owned view.
struct VerseResultsPanel: View {
    @ObservedObject var world: VerseWorld
    let close: () -> Void
    private var view: ResultsView? { world.resultsView }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                if view?.can_back == true {
                    Button("Back", systemImage: "chevron.left") { world.results(["do": "back"]) }
                        .labelStyle(.iconOnly).frame(width: 44, height: 44)
                        .accessibilityIdentifier("results-back")
                }
                Label("Results", systemImage: "list.number").font(.paper(.headline))
                Spacer()
                Button("Back to world", systemImage: "xmark", action: close)
                    .labelStyle(.iconOnly).frame(width: 44, height: 44)
                    .accessibilityIdentifier("results-close")
            }
            if let view {
                if let error = view.error {
                    Text(error).font(.paper(.callout)).textSelection(.enabled)
                        .accessibilityIdentifier("results-error")
                }
                switch view.page {
                case .boards(let page)?: ScrollView { boards(page) }
                case .board(let page)?: ScrollView { board(page) }
                case .attempt(let page)?: ScrollView { attempt(page) }
                case .trace(let page)?: VerseTraceViewer(world: world, page: page, replay: view.replay)
                case .unknown?: Text("This screen needs a newer app.")
                case nil:
                    if view.loading { ProgressView(view.status).accessibilityIdentifier("results-loading") }
                    else {
                        Text(view.status).font(.paper(.callout))
                        if view.active { Button("Try again") { world.results(["do": "retry"]) } }
                    }
                }
            } else {
                ProgressView("Loading results…").accessibilityIdentifier("results-loading")
            }
        }
        .padding(14)
        .foregroundStyle(.white)
        .tint(.white)
        .background(Color(white: 0.04).opacity(0.97), in: RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(.white.opacity(0.55), lineWidth: 1))
    }

    // Screen 1: every board, in the publication's order.
    private func boards(_ page: ResultsBoardsPage) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            if let summary = page.summary {
                Button { world.results(["do": "board", "id": summary.board]) } label: {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(summary.text).font(.paper(.body, weight: .semibold)).multilineTextAlignment(.leading)
                        Text(summary.source).font(.paper(.caption)).foregroundStyle(.secondary)
                            .multilineTextAlignment(.leading)
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }
                .accessibilityElement(children: .ignore)
                .accessibilityLabel("\(summary.text) \(summary.source)")
                .accessibilityAddTraits(.isButton)
                .accessibilityIdentifier("results-summary")
                Divider().overlay(.white.opacity(0.3))
            }
            Text("Published Terminal-Bench results").font(.paper(.headline))
            ForEach(page.rows) { row in
                Button { world.results(["do": "board", "id": row.id]) } label: {
                    VStack(alignment: .leading, spacing: 6) {
                        Text(row.title).font(.paper(.headline)).multilineTextAlignment(.leading)
                        Text(row.benchmark).font(.paper(.caption)).foregroundStyle(.secondary)
                        Text(row.headline).font(.paper(.callout)).multilineTextAlignment(.leading)
                        if let note = row.headline_note { Text(note).font(.paper(.caption)).foregroundStyle(.secondary) }
                        ResultsChips(chips: row.labels)
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(row.accessibility)
                .accessibilityAddTraits(.isButton)
                .accessibilityIdentifier("results-board-\(row.id)")
                Divider().overlay(.white.opacity(0.3))
            }
            if let footer = page.footer {
                Text(footer).font(.paper(.caption2)).foregroundStyle(.secondary)
                    .accessibilityIdentifier("results-footer")
            }
            Text("Each board is separate, in the order it was published. Scores aren't added up or ranked across boards.")
                .font(.paper(.caption)).foregroundStyle(.secondary)
        }.frame(maxWidth: .infinity, alignment: .leading)
    }

    // Screen 2: one board.
    private func board(_ page: ResultsBoardPage) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(page.summary).font(.paper(.body, weight: .semibold))
                .accessibilityIdentifier("results-board-summary")
            Text(page.title).font(.paper(.headline))
            Text(page.benchmark).font(.paper(.caption)).foregroundStyle(.secondary)
            Text(page.headline).font(.paper(.callout)).accessibilityIdentifier("results-headline")
            if let note = page.headline_note { Text(note).font(.paper(.caption)).foregroundStyle(.secondary) }
            ResultsChips(chips: page.labels)
            caveats(page)
            VStack(alignment: .leading, spacing: 4) {
                ForEach(page.tallies, id: \.self) { tally in
                    Text(tally.text).font(.paper(.caption))
                }
            }
            VStack(alignment: .leading, spacing: 2) {
                ForEach(page.spend, id: \.self) { Text($0).font(.paper(.caption)) }
            }
            reference(page.reference)
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 8) {
                    ForEach(page.filters, id: \.self) { filter in
                        Button("\(filter.text) (\(filter.count))") {
                            world.results(["do": "filter", "filter": filter.filter])
                        }
                        .font(.paper(.caption))
                        .padding(.horizontal, 10).padding(.vertical, 6)
                        .background(filter.selected ? Color.white.opacity(0.22) : Color.clear, in: Capsule())
                        .overlay(Capsule().stroke(.white.opacity(0.5), lineWidth: 1))
                        .accessibilityAddTraits(filter.selected ? .isSelected : [])
                        .accessibilityIdentifier("results-filter-\(filter.filter)")
                    }
                }
            }
            ForEach(page.tasks) { task in
                VStack(alignment: .leading, spacing: 6) {
                    HStack(alignment: .firstTextBaseline) {
                        Text(task.task).font(.paper(.subheadline, weight: .bold))
                        Spacer()
                        Text(task.status).font(.paper(.caption)).foregroundStyle(.secondary)
                    }
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel(task.accessibility)
                    Text(task.bar).font(.paper(.caption2)).foregroundStyle(.secondary)
                    Text(task.knowledge).font(.paper(.caption2)).foregroundStyle(.secondary)
                    ForEach(task.attempts) { cell in
                        Button { world.results(["do": "attempt", "id": cell.id]) } label: {
                            VStack(alignment: .leading, spacing: 3) {
                                HStack(spacing: 6) {
                                    Image(systemName: cell.beat ? "star.fill" : (cell.passed ? "checkmark" : "xmark"))
                                        .font(.paper(.caption))
                                    Text("\(cell.series): \(cell.text)").font(.paper(.caption))
                                        .multilineTextAlignment(.leading)
                                }
                                ResultsChips(chips: cell.labels, small: true)
                                ForEach(cell.caveats, id: \.self) { caveat in
                                    Text(caveat.text).font(.paper(.caption2)).foregroundStyle(.secondary)
                                        .multilineTextAlignment(.leading)
                                }
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(8)
                            .overlay(RoundedRectangle(cornerRadius: 8).stroke(.white.opacity(cell.beat ? 0.8 : 0.25)))
                        }
                        .accessibilityElement(children: .ignore)
                        .accessibilityLabel(cell.accessibility)
                        .accessibilityAddTraits(.isButton)
                        .accessibilityIdentifier("results-attempt-\(cell.id)")
                    }
                }
                Divider().overlay(.white.opacity(0.3))
            }
        }.frame(maxWidth: .infinity, alignment: .leading)
    }

    private func caveats(_ page: ResultsBoardPage) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Caveats (\(page.caveat_count))").font(.paper(.subheadline, weight: .bold))
            ForEach(page.caveats, id: \.self) { caveat in
                Text(caveat.text).font(.paper(.caption)).textSelection(.enabled)
            }
            if page.caveat_count > 1 {
                Button(page.caveats_open ? "Show the first caveat only" : "Show all \(page.caveat_count) caveats") {
                    world.results(["do": "caveats", "open": !page.caveats_open])
                }
                .font(.paper(.caption))
                .accessibilityIdentifier("results-caveats")
            }
        }
        .padding(10)
        .overlay(RoundedRectangle(cornerRadius: 10).stroke(.white.opacity(0.35)))
    }

    private func reference(_ reference: ResultsReference) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Bar: \(reference.name)").font(.paper(.caption, weight: .bold))
            Text(reference.rule).font(.paper(.caption2)).foregroundStyle(.secondary)
            DisclosureGroup("Reference conditions") {
                Text(reference.conditions).font(.paper(.caption2)).textSelection(.enabled)
            }
            .font(.paper(.caption))
            .accessibilityIdentifier("results-conditions")
        }
    }

    // Screen 3: one attempt.
    private func attempt(_ page: ResultsAttemptPage) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            ResultsHeaderView(header: page.header)
            Text("\(page.board_title) · \(page.series)").font(.paper(.caption)).foregroundStyle(.secondary)
            VStack(alignment: .leading, spacing: 4) {
                ForEach(page.numbers, id: \.self) { Text($0).font(.paper(.callout)) }
            }
            Text(page.misses).font(.paper(.callout, weight: .bold)).accessibilityIdentifier("results-misses")
            ForEach(page.caveats, id: \.self) { caveat in
                Text(caveat.text).font(.paper(.caption)).foregroundStyle(.secondary)
            }
            reference(page.reference)
            if !page.phases.isEmpty {
                Text("Phases").font(.paper(.subheadline, weight: .bold))
                ForEach(page.phases, id: \.self) { Text($0).font(.paper(.caption)) }
            }
            if let ended = page.how_it_ended { Text(ended).font(.paper(.caption)) }
            if let jev = page.jev { Text(jev).font(.paper(.caption)) }
            if let verifier = page.verifier { Text("Verifier: \(verifier)").font(.paper(.caption)) }
            ForEach(page.failed_tests, id: \.self) { Text("Failed: \($0)").font(.paper(.caption2)) }
            Text(page.trial).font(.paper(.caption2)).foregroundStyle(.secondary).textSelection(.enabled)
            if let trace = page.trace {
                Button(trace, systemImage: "play.rectangle") { world.results(["do": "trace"]) }
                    .buttonStyle(.bordered)
                    .accessibilityIdentifier("results-open-trace")
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .contain)
    }
}

/// The header every attempt and trace screen shows.
struct ResultsHeaderView: View {
    let header: ResultsHeader

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(header.task).font(.paper(.headline))
            Text("\(header.result) · \(header.beat)").font(.paper(.callout, weight: .bold))
            Text("Cost \(header.cost)").font(.paper(.caption))
            Text("Time \(header.time)").font(.paper(.caption))
            ResultsChips(chips: header.labels)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(header.accessibility)
        .accessibilityIdentifier("results-header")
    }
}

/// Labels as outlined chips that wrap.
struct ResultsChips: View {
    let chips: [ResultsChip]
    var small = false

    var body: some View {
        ResultsFlow(spacing: 4) {
            ForEach(chips, id: \.self) { chip in
                Text(chip.text)
                    .font(small ? .paper(.caption2) : .paper(.caption))
                    .padding(.horizontal, small ? 5 : 7).padding(.vertical, 2)
                    .overlay(Capsule().stroke(.white.opacity(chip.code == "thin_margin" || chip.code == "in_sample" ? 0.9 : 0.4)))
            }
        }
        .accessibilityElement(children: .combine)
    }
}

/// Lays out its children left to right, wrapping to new rows.
struct ResultsFlow: Layout {
    var spacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? .infinity
        var x: CGFloat = 0, y: CGFloat = 0, row: CGFloat = 0, widest: CGFloat = 0
        for view in subviews {
            let size = view.sizeThatFits(.unspecified)
            if x > 0, x + size.width > width { x = 0; y += row + spacing; row = 0 }
            x += size.width + spacing
            row = max(row, size.height)
            widest = max(widest, x - spacing)
        }
        return CGSize(width: min(widest, width), height: y + row)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX, y = bounds.minY, row: CGFloat = 0
        for view in subviews {
            let size = view.sizeThatFits(.unspecified)
            if x > bounds.minX, x + size.width > bounds.maxX { x = bounds.minX; y += row + spacing; row = 0 }
            view.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
            x += size.width + spacing
            row = max(row, size.height)
        }
    }
}
