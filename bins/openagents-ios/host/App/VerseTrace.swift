// The Grid Gym's trace viewer for a published attempt: a scrubber over the
// bundle's clock with play, pause, and step, and tabs for Jev, the
// briefing, the agent's steps, and the verifier. Rust loads and verifies the
// bundle, owns the playhead and paging, and formats every row
// (`gym_leaderboard::view::TracePage`); this view draws one page and sends
// choices back. Expanded output comes from Rust only for the row asked for.
import SwiftUI

struct ResultsTracePage: Decodable {
    let attempt: String
    let header: ResultsHeader
    let clock: Clock
    let tab: String
    let tabs: [TabChip]
    let jev: Jev?
    let briefing: TextBlock?
    let agent: Agent?
    let verifier: Verifier?

    struct Clock: Decodable {
        let duration_ms: UInt64
        let playhead_ms: UInt64
        let fraction: Double
        let step: Int
        let steps: Int
        let playing: Bool
        let text: String
        let marks: [Double]
    }
    struct TabChip: Decodable, Hashable { let tab: String; let text: String; let selected: Bool; let available: Bool }
    struct TextBlock: Decodable { let text: String; let cut: String? }
    struct Jev: Decodable {
        let summary: String
        let question_set: String
        let questions: [String]
        let keep_threshold: Double
        let flag_threshold: Double
        let candidates: [Candidate]
        let requirements: [Requirement]
    }
    struct Candidate: Decodable, Identifiable {
        var id: String { "\(rank)-\(key)" }
        let rank: UInt32
        let key: String
        let title: String?
        let p: Double
        let kept: Bool
        let own: Bool
        let fate: String
        let accessibility: String
        private enum CodingKeys: String, CodingKey { case rank, key = "id", title, p, kept, own, fate, accessibility }
    }
    struct Requirement: Decodable, Hashable {
        let text: String
        let p: Double
        let flagged: Bool
        let accessibility: String
    }
    struct Agent: Decodable {
        let page: Int
        let pages: Int
        let tokens: String
        let rows: [Step]
    }
    struct Step: Decodable, Identifiable {
        var id: Int { index }
        let index: Int
        let at: String
        let kind: String
        let text: String
        let exit_code: Int64?
        let expandable: Bool
        let output: TextBlock?
        let cut: String?
        let reached: Bool
        let current: Bool
        let accessibility: String
    }
    struct Verifier: Decodable {
        let summary: String
        let tests: [Test]
        let output_tail: TextBlock
    }
    struct Test: Decodable, Hashable { let name: String; let status: String; let passed: Bool }
}

struct VerseTraceViewer: View {
    @ObservedObject var world: VerseWorld
    let page: ResultsTracePage
    /// Where the replay's ghost stands in the Gym, from Rust.
    let replay: String?
    @State private var scrub: Double?
    /// Only the timeline shows, so the replay in the Gym is in view.
    @State private var watching = Self.watchAtLaunch

    /// Simulator and debug builds take `--trace-watch` to start with only
    /// the timeline, for captures of the replay.
    private static var watchAtLaunch: Bool {
        #if DEBUG || targetEnvironment(simulator)
        ProcessInfo.processInfo.arguments.contains("--trace-watch")
        #else
        false
        #endif
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if watching {
                Text("\(page.header.task) · \(page.header.result) · \(page.header.beat)").font(.paper(.subheadline, weight: .bold))
            } else {
                ResultsHeaderView(header: page.header)
            }
            timeline
            HStack {
                if let replay { Text(replay).font(.paper(.caption)).accessibilityIdentifier("trace-replay") }
                Spacer()
                Button(watching ? "Show the trace" : "Watch in the Gym",
                       systemImage: watching ? "rectangle.expand.vertical" : "figure.walk") {
                    watching.toggle()
                }
                .font(.paper(.caption))
                .accessibilityIdentifier("trace-watch")
            }
            if !watching { tabs }
        }
    }

    @ViewBuilder private var tabs: some View {
        VStack(alignment: .leading, spacing: 8) {
            Picker("Tab", selection: Binding(get: { page.tab }, set: { world.results(["do": "tab", "tab": $0]) })) {
                ForEach(page.tabs, id: \.self) { tab in
                    Text(tab.text).tag(tab.tab)
                }
            }
            .pickerStyle(.segmented)
            .accessibilityIdentifier("trace-tabs")
            if let agent = page.agent {
                // The running token counter at the playhead stays in view.
                Text(agent.tokens).font(.paper(.caption)).accessibilityIdentifier("trace-tokens")
            }
            ScrollViewReader { reader in
                ScrollView {
                    VStack(alignment: .leading, spacing: 8) {
                        switch page.tab {
                        case "jev": jev
                        case "briefing": briefing
                        case "agent": agent
                        default: verifier
                        }
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }
                // The Agent tab follows the playhead's row.
                .onChange(of: page.clock.step, initial: true) { _, step in
                    guard page.tab == "agent" else { return }
                    withAnimation(.easeOut(duration: 0.2)) { reader.scrollTo(step, anchor: .center) }
                }
                .onChange(of: page.tab) { _, tab in
                    if tab == "agent" { reader.scrollTo(page.clock.step, anchor: .center) }
                }
            }
        }
    }

    private var timeline: some View {
        VStack(alignment: .leading, spacing: 4) {
            ZStack(alignment: .leading) {
                GeometryReader { geometry in
                    ForEach(Array(page.clock.marks.enumerated()), id: \.offset) { _, mark in
                        Rectangle().fill(.white.opacity(0.35)).frame(width: 1, height: 6)
                            .offset(x: mark * geometry.size.width, y: 0)
                    }
                }.frame(height: 6).allowsHitTesting(false)
                Slider(value: Binding(get: { scrub ?? page.clock.fraction }, set: { scrub = $0 }), in: 0...1) { editing in
                    if !editing, let value = scrub {
                        world.results(["do": "seek", "fraction": value])
                        scrub = nil
                    }
                }
                .padding(.top, 6)
                .accessibilityLabel("Trace timeline")
                .accessibilityValue("\(page.clock.text), step \(page.clock.step + 1) of \(page.clock.steps)")
                .accessibilityIdentifier("trace-scrubber")
            }
            HStack(spacing: 18) {
                Button("Step back", systemImage: "backward.frame") { world.results(["do": "step", "forward": false]) }
                    .labelStyle(.iconOnly)
                Button(page.clock.playing ? "Pause" : "Play", systemImage: page.clock.playing ? "pause.fill" : "play.fill") {
                    world.results(["do": "play", "playing": !page.clock.playing])
                }
                .labelStyle(.iconOnly)
                .accessibilityIdentifier("trace-play")
                Button("Step forward", systemImage: "forward.frame") { world.results(["do": "step", "forward": true]) }
                    .labelStyle(.iconOnly)
                Spacer()
                Text("\(page.clock.text) · step \(page.clock.step + 1) of \(page.clock.steps)")
                    .font(.paper(.caption))
            }
        }
    }

    @ViewBuilder private var jev: some View {
        if let jev = page.jev {
            Text(jev.summary).font(.paper(.caption))
            Text("Candidates").font(.paper(.subheadline, weight: .bold))
            ForEach(jev.candidates) { candidate in
                VStack(alignment: .leading, spacing: 3) {
                    HStack {
                        Text("\(candidate.rank). \(candidate.title ?? candidate.key)").font(.paper(.caption, weight: candidate.kept ? .bold : .regular))
                        Spacer()
                        if candidate.own { Text("own").font(.paper(.caption2)).padding(.horizontal, 5).overlay(Capsule().stroke(.white.opacity(0.8))) }
                        Text(candidate.kept ? "kept" : candidate.fate).font(.paper(.caption2)).foregroundStyle(.secondary)
                    }
                    ProbabilityBar(p: candidate.p, threshold: jev.keep_threshold, strong: candidate.kept)
                }
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(candidate.accessibility)
            }
            Text("Requirements").font(.paper(.subheadline, weight: .bold)).padding(.top, 6)
            ForEach(jev.requirements, id: \.self) { requirement in
                VStack(alignment: .leading, spacing: 3) {
                    Text(requirement.text).font(.paper(.caption, weight: requirement.flagged ? .bold : .regular))
                    ProbabilityBar(p: requirement.p, threshold: jev.flag_threshold, strong: requirement.flagged)
                }
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(requirement.accessibility)
            }
        } else {
            Text("No Jev decision in this trace.").font(.paper(.caption))
        }
    }

    @ViewBuilder private var briefing: some View {
        if let briefing = page.briefing {
            if let cut = briefing.cut { Text(cut).font(.paper(.caption2)).foregroundStyle(.secondary) }
            Text(briefing.text).font(.paper(.caption)).textSelection(.enabled)
        } else {
            Text("No briefing in this trace.").font(.paper(.caption))
        }
    }

    @ViewBuilder private var agent: some View {
        if let agent = page.agent {
            if agent.pages > 1 {
                HStack {
                    Button("Earlier") { world.results(["do": "page", "page": agent.page - 1]) }.disabled(agent.page == 0)
                    Spacer()
                    Text("Page \(agent.page + 1) of \(agent.pages)").font(.paper(.caption))
                    Spacer()
                    Button("Later") { world.results(["do": "page", "page": agent.page + 1]) }.disabled(agent.page + 1 >= agent.pages)
                }.font(.paper(.caption))
            }
            ForEach(agent.rows) { step in
                VStack(alignment: .leading, spacing: 3) {
                    HStack(alignment: .firstTextBaseline, spacing: 6) {
                        Text(step.at).font(.paper(.caption2)).foregroundStyle(.secondary)
                        Text(step.kind.replacingOccurrences(of: "_", with: " ")).font(.paper(.caption2, weight: .bold))
                        if let code = step.exit_code { Text("exit \(code)").font(.paper(.caption2)) }
                        Spacer()
                    }
                    Text(step.text).font(step.kind == "command" ? .paper(.caption) : .paper(.caption))
                        .lineLimit(step.kind == "say" ? 6 : 4)
                    if let cut = step.cut { Text(cut).font(.paper(.caption2)).foregroundStyle(.secondary) }
                    if let output = step.output {
                        Text(output.text).font(.paper(.caption2)).textSelection(.enabled)
                            .padding(6).background(Color(white: 0.1), in: RoundedRectangle(cornerRadius: 6))
                    }
                }
                .padding(6)
                .id(step.index)
                .opacity(step.reached ? 1 : 0.45)
                .overlay(RoundedRectangle(cornerRadius: 6).stroke(.white.opacity(step.current ? 0.9 : 0)))
                .contentShape(Rectangle())
                .onTapGesture {
                    if step.expandable {
                        world.results(["do": "expand", "index": step.output == nil ? step.index : NSNull()])
                    }
                }
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(step.accessibility)
                .accessibilityAddTraits(step.expandable ? .isButton : [])
                .accessibilityHint(step.expandable ? (step.output == nil ? "Shows the output" : "Hides the output") : "")
            }
        }
    }

    @ViewBuilder private var verifier: some View {
        if let verifier = page.verifier {
            Text(verifier.summary).font(.paper(.callout, weight: .bold))
            ForEach(verifier.tests, id: \.self) { test in
                HStack {
                    Image(systemName: test.passed ? "checkmark" : "xmark").font(.paper(.caption))
                    Text(test.name).font(.paper(.caption))
                    Spacer()
                    Text(test.status).font(.paper(.caption2)).foregroundStyle(.secondary)
                }
                .accessibilityElement(children: .combine)
            }
            Text("Output tail").font(.paper(.subheadline, weight: .bold)).padding(.top, 6)
            if let cut = verifier.output_tail.cut { Text(cut).font(.paper(.caption2)).foregroundStyle(.secondary) }
            Text(verifier.output_tail.text).font(.paper(.caption2)).textSelection(.enabled)
        } else {
            Text("No verifier result in this trace.").font(.paper(.caption))
        }
    }
}

/// A probability as a bar, with the keep or flag threshold as a line.
private struct ProbabilityBar: View {
    let p: Double
    let threshold: Double
    let strong: Bool

    var body: some View {
        GeometryReader { geometry in
            ZStack(alignment: .leading) {
                Rectangle().fill(.white.opacity(0.12))
                Rectangle().fill(.white.opacity(strong ? 0.85 : 0.4))
                    .frame(width: max(0, min(1, p)) * geometry.size.width)
                Rectangle().fill(.white).frame(width: 2)
                    .offset(x: max(0, min(1, threshold)) * geometry.size.width - 1)
            }
        }
        .frame(height: 6)
        .overlay(alignment: .trailing) {
            Text(String(format: "%.2f", p)).font(.paper(.caption2)).offset(y: -10)
        }
    }
}
