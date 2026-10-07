// Coder's native Gym board. Rust owns subscriptions, verified snapshots,
// recipe selection, authority, and launch identity. Charts render received data.
import Charts
import SwiftUI
import UIKit

struct GymPanel: View {
    @ObservedObject var bridge: VerseBridge
    let close: () -> Void
    @State private var configuring = false
    @State private var code = ""
    private var board: GymBoardView? { bridge.gymBoard }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Label("Gym", systemImage: "chart.xyaxis.line").font(.paper(.headline))
                Spacer()
                Button("Back to world", systemImage: "xmark", action: close)
                    .labelStyle(.iconOnly).accessibilityIdentifier("gym-close")
            }
            if let board {
                Text(board.status).font(.paper(.caption)).accessibilityIdentifier("gym-status")
                if board.stale { Text("Snapshot is stale. New starts are unavailable.").font(.paper(.caption)) }
                if let error = board.error ?? bridge.packet?.error ?? bridge.gymStorageError ?? bridge.nativeError {
                    Text(error).font(.paper(.callout)).textSelection(.enabled).accessibilityIdentifier("gym-error")
                }
                ScrollView {
                    VStack(alignment: .leading, spacing: 14) {
                        if !board.configured || configuring {
                            connection(board)
                        } else if let run = board.selected_run {
                            Button("All runs") { bridge.send(["action": "gym_close_detail"]) }
                                .accessibilityIdentifier("gym-all-runs")
                            runDetails(run)
                        } else if let recipe = board.selected_recipe {
                            Button("All runs") { bridge.send(["action": "gym_close_detail"]) }
                            recipeDetails(recipe, board: board)
                        } else {
                            runList(board)
                            Button("Gym connection") { configuring = true }
                                .accessibilityIdentifier("gym-connection")
                        }
                        if let launch = board.launch { launchStatus(launch, active: board.active) }
                        ForEach(Array(board.notices.enumerated()), id: \.offset) { _, notice in
                            Text(notice).font(.paper(.caption)).textSelection(.enabled)
                        }
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }.scrollDismissesKeyboard(.interactively)
            } else {
                ProgressView("Loading Gym board…").accessibilityIdentifier("gym-loading")
            }
        }
        .padding(14)
        .background(Color(red: 0.025, green: 0.02, blue: 0).opacity(0.97), in: RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(.tint.opacity(0.7), lineWidth: 1))
    }

    private func connection(_ board: GymBoardView) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Connect a Gym host").font(.paper(.headline))
            Text("Create a Gym connection grant on your host for this device public key, then paste its gym-connect: code.")
            Text(board.public_key).font(.paper(.caption2)).textSelection(.enabled)
                .accessibilityIdentifier("gym-public-key")
            Button("Copy public key", systemImage: "doc.on.doc") { UIPasteboard.general.string = board.public_key }
            TextEditor(text: $code).frame(minHeight: 85, maxHeight: 130)
                .textInputAutocapitalization(.never).autocorrectionDisabled()
                .accessibilityLabel("Gym connection code").accessibilityIdentifier("gym-code")
            Button("Connect Gym") {
                if bridge.configureGym(code) { code = ""; configuring = false }
            }.disabled(code.isEmpty).accessibilityIdentifier("gym-connect")
            Text("This grant is separate from saved-chat access. Only the host's listed recipes can start, after you confirm.")
                .font(.paper(.caption))
            if board.configured { Button("Back to board") { configuring = false } }
        }
    }

    private func runList(_ board: GymBoardView) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Microcoder and Terminal-Bench runs").font(.paper(.headline))
            if board.runs.isEmpty { Text("No runs are available in this snapshot.") }
            ForEach(board.runs.sorted { rank($0.category) < rank($1.category) }) { run in
                Button {
                    bridge.send(["action": "gym_select_run", "id": run.id])
                } label: {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(run.title).font(.paper(.headline))
                        Text("\(run.category) · \(run.status)").font(.paper(.caption))
                        progress(run)
                        Text(summary(run)).font(.paper(.caption))
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }.accessibilityIdentifier("gym-run-\(run.id)")
                Divider()
            }
            Text("Supported new runs").font(.paper(.headline))
            if board.recipes.isEmpty { Text("This connection has no supported start recipes.") }
            ForEach(board.recipes) { recipe in
                Button(recipe.title) { bridge.send(["action": "gym_select_recipe", "id": recipe.id]) }
                    .disabled(!board.active || board.stale)
                    .accessibilityIdentifier("gym-recipe-\(recipe.id)")
            }
        }
    }

    private func runDetails(_ run: GymRun) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(run.title).font(.paper(.headline)).accessibilityIdentifier("gym-run-title")
            Text("\(run.category) · \(run.status)")
            progress(run)
            Text(summary(run)).font(.paper(.callout))
            ForEach(Array(run.metrics.enumerated()), id: \.offset) { _, metric in
                VStack(alignment: .leading, spacing: 6) {
                    Text("\(metric.name) (\(metric.unit))").font(.paper(.headline))
                    if metric.points.isEmpty { Text("No recorded points.") }
                    else {
                        Chart(Array(metric.points.enumerated()), id: \.offset) { _, point in
                            LineMark(x: .value("Step", point.step), y: .value(metric.unit, point.value))
                                .foregroundStyle(.tint)
                            PointMark(x: .value("Step", point.step), y: .value(metric.unit, point.value))
                                .foregroundStyle(.tint)
                        }.frame(height: 160).accessibilityLabel("\(metric.name), \(metric.points.count) recorded points")
                        DisclosureGroup("Recorded values") {
                            ForEach(Array(metric.points.enumerated()), id: \.offset) { _, point in
                                Text("Step \(point.step): \(point.value.formatted()) \(metric.unit)")
                                    .font(.paper(.caption)).textSelection(.enabled)
                            }
                        }
                    }
                }
            }
            Text("Source: \(run.source)").font(.paper(.caption)).textSelection(.enabled)
            Text(run.provenance).font(.paper(.caption)).textSelection(.enabled)
            Text("Completed describes the recorded process; it does not by itself establish benchmark success.")
                .font(.paper(.caption))
        }
    }

    private func recipeDetails(_ recipe: GymRecipe, board: GymBoardView) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(recipe.title).font(.paper(.headline))
            Text(recipe.detail).textSelection(.enabled)
            Text("Time limit: \(recipe.budget.wall_ms / 1000) seconds · Maximum starts: \(recipe.budget.max_starts)")
            Text(recipe.budget.spend_enforced ? "The host enforces this recipe's spending limit." : "No dollar limit is enforced for this recipe.")
            Text("Recipe revision: \(recipe.revision)").font(.paper(.caption2)).textSelection(.enabled)
            Text("Starting submits this exact recipe to the host. Leaving the Gym does not cancel the run.")
            Button("Start this run") { bridge.send(["action": "gym_launch"]) }
                .disabled(!board.active || board.stale || ["sending", "unknown"].contains(board.launch?.phase ?? ""))
                .accessibilityIdentifier("gym-confirm-launch")
        }
    }

    private func launchStatus(_ launch: GymLaunch, active: Bool) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Request: \(launch.phase)").font(.paper(.headline)).accessibilityIdentifier("gym-launch-status")
            Text(launch.request_id).font(.paper(.caption2)).textSelection(.enabled)
            if let receipt = launch.receipt {
                Text("Host receipt: \(receipt.status) · Run \(receipt.run_id)").font(.paper(.caption))
            }
            if let error = launch.error { Text(error).font(.paper(.caption)) }
            if launch.phase == "sending" { ProgressView("Waiting for the host receipt…") }
            if launch.phase == "unknown" {
                Text("The host may already have accepted this request. Retry uses the same request identity.").font(.paper(.caption))
                Button("Retry the same request") { bridge.send(["action": "gym_retry"]) }
                    .disabled(!active).accessibilityIdentifier("gym-retry")
            }
        }
    }

    @ViewBuilder private func progress(_ run: GymRun) -> some View {
        if let completed = run.completed, let total = run.total {
            Text("\(completed) / \(total) recorded").font(.paper(.caption))
            if total > 0 { ProgressView(value: Double(completed), total: Double(total)) }
        } else { Text("Progress unavailable").font(.paper(.caption)) }
    }

    private func rank(_ category: String) -> Int {
        switch category { case "agent": return 0; case "evaluation": return 1; default: return 2 }
    }

    private func summary(_ run: GymRun) -> String {
        let cost = run.cost_usd.map { "$\($0.formatted(.number.precision(.fractionLength(4))))" } ?? "Cost unavailable"
        let time = run.elapsed_ms.map { "\(($0 / 1000)) s" } ?? "Time unavailable"
        return "\(cost) · \(time)"
    }
}
