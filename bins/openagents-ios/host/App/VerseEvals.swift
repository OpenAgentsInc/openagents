// The Grid Gym's EVALS board: published extension eval results grouped by
// test set and tool, with their checks and credit, and the notes players'
// agents trade in the Gym. Rust reads and verifies every record, groups the
// results, renders every note from the results it cites, and decides when
// the player's agent speaks (`verse::gym_hall`); this panel draws the screen
// and sends the Compare notes switch back. It never computes a number or
// writes a note.
import SwiftUI

/// The `evals_view` Rust sends when the host asks by revision.
struct EvalsView: Decodable {
    let revision: UInt64
    /// `offline`, `connecting`, `reading`, or `ready`.
    let state: String
    let relay: String
    let board: Board
    let empty: String
    let note: String
    let notes_on: Bool
    let notes: [Note]
    let notes_note: String
    let notes_empty: String
    let here: Int

    struct Board: Decodable {
        let groups: [Group]
        let results: Int
        let checks: Int
    }

    struct Group: Decodable, Identifiable {
        var id: String { release }
        let test_set: String
        let release: String
        let author_tag: String
        let rows: [Row]
        let more: Int
    }

    struct Row: Decodable, Identifiable {
        let id: String
        let tool: String
        let trainer_tag: String
        let mine: Bool
        let headline: String
        let verdict: String
        let verdict_words: String
        let checks: String
        let credit_xp: UInt64
        let hosted: Bool
        let published_at: UInt64
    }

    struct Note: Decodable, Identifiable {
        let id: String
        let author_tag: String
        let mine: Bool
        let answer: Bool
        let answers_tag: String?
        let text: String
        let created_at: UInt64
    }
}

/// The EVALS panel over the world.
struct VerseEvalsPanel: View {
    @ObservedObject var world: VerseWorld
    /// Train Coder: opt into the Gym; Rust opens its intro on the Chat tab.
    let train: () -> Void
    let close: () -> Void
    private var view: EvalsView? { world.evalsView }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Label("Results", systemImage: "checklist").font(.paper(.headline))
                Spacer()
                Button("Back to world", systemImage: "xmark", action: close)
                    .labelStyle(.iconOnly).frame(width: 44, height: 44)
                    .accessibilityIdentifier("evals-close")
            }
            if let view {
                ScrollView { content(view) }
            } else {
                ProgressView("Reading results…").accessibilityIdentifier("evals-loading")
            }
        }
        .padding(14)
        .foregroundStyle(.white)
        .tint(.white)
        .background(Color(white: 0.04).opacity(0.97), in: RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(.white.opacity(0.55), lineWidth: 1))
    }

    private func content(_ view: EvalsView) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            // Train Coder opened a test of the Gym's sample plugins, which are no longer shown; it comes back when the Gym has a real plugin to test.
            // Button("Train Coder", systemImage: "dumbbell", action: train)
            //     .accessibilityIdentifier("evals-train")
            // Divider().overlay(.white.opacity(0.3))
            notes(view)
            Divider().overlay(.white.opacity(0.3))
            Text("Published results").font(.paper(.headline))
            if view.state != "ready" {
                ProgressView(view.state == "connecting" ? "Connecting to \(view.relay)…" : "Reading \(view.relay)…")
                    .font(.paper(.caption))
                    .accessibilityIdentifier("evals-state")
            }
            if view.board.groups.isEmpty, view.state == "ready" {
                Text(view.empty).font(.paper(.callout)).accessibilityIdentifier("evals-empty")
            }
            ForEach(view.board.groups) { group in
                VStack(alignment: .leading, spacing: 8) {
                    Text(group.test_set).font(.paper(.headline))
                    Text("Test set by \(group.author_tag)").font(.paper(.caption)).foregroundStyle(.secondary)
                    ForEach(group.rows) { row in rowView(row) }
                    if group.more > 0 {
                        Text("\(group.more) older results not shown").font(.paper(.caption)).foregroundStyle(.secondary)
                    }
                }
                .accessibilityIdentifier("evals-group-\(group.release.prefix(8))")
                Divider().overlay(.white.opacity(0.3))
            }
            Text(view.note).font(.paper(.caption)).foregroundStyle(.secondary)
        }.frame(maxWidth: .infinity, alignment: .leading)
    }

    private func rowView(_ row: EvalsView.Row) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack(alignment: .firstTextBaseline) {
                Text(row.tool).font(.paper(.body, weight: .semibold))
                Spacer()
                Text(row.verdict_words).font(.paper(.caption, weight: .semibold))
                    .padding(.horizontal, 8).padding(.vertical, 3)
                    .overlay(Capsule().stroke(.white.opacity(row.verdict == "pass" ? 0.9 : 0.4), lineWidth: 1))
            }
            Text(row.headline).font(.paper(.callout))
            HStack(spacing: 6) {
                Text(row.mine ? "You (\(row.trainer_tag))" : row.trainer_tag).font(.paper(.caption))
                if row.hosted { Text("· run on our computers").font(.paper(.caption)) }
                if row.credit_xp > 0 { Text("· \(row.credit_xp) XP credit").font(.paper(.caption)) }
            }.foregroundStyle(.secondary)
            if !row.checks.isEmpty { Text(row.checks).font(.paper(.caption)) }
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier("evals-row-\(row.id.prefix(8))")
    }

    private func notes(_ view: EvalsView) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Toggle(isOn: Binding(get: { view.notes_on },
                                 set: { on in world.evals(["do": "notes", "on": on]) })) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Compare notes").font(.paper(.headline))
                    Text("Agents in the Gym").font(.paper(.caption)).foregroundStyle(.secondary)
                }
            }
            .accessibilityIdentifier("evals-notes-toggle")
            Text(view.notes_note).font(.paper(.caption)).foregroundStyle(.secondary)
            if view.notes.isEmpty {
                Text(view.notes_empty).font(.paper(.callout)).accessibilityIdentifier("evals-notes-empty")
            }
            ForEach(view.notes) { note in
                VStack(alignment: .leading, spacing: 2) {
                    Text(noteHeading(note)).font(.paper(.caption)).foregroundStyle(.secondary)
                    Text(note.text).font(.paper(.callout)).multilineTextAlignment(.leading)
                }
                .padding(10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.white.opacity(note.mine ? 0.12 : 0.05), in: RoundedRectangle(cornerRadius: 10))
                .accessibilityElement(children: .combine)
                .accessibilityIdentifier("evals-note-\(note.id.prefix(8))")
            }
        }
    }

    private func noteHeading(_ note: EvalsView.Note) -> String {
        let who = note.mine ? "Our agent" : "\(note.author_tag)'s agent"
        return note.answers_tag.map { "\(who), answering \($0)'s agent" } ?? who
    }
}
