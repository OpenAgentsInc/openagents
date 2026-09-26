// Coder's native Gym board. Rust owns subscriptions, verified snapshots,
// recipe selection, authority, and launch identity. Charts render received data.
import Charts
import SwiftUI
import UIKit

struct GymBoardView: Decodable {
    let revision: UInt64
    let active: Bool
    let configured: Bool
    let public_key: String
    let status: String
    let error: String?
    let stale: Bool
    let observed_at: UInt64?
    let runs: [GymRun]
    let recipes: [GymRecipe]
    let selected_run: GymRun?
    let selected_recipe: GymRecipe?
    let launch: GymLaunch?
    let notices: [String]

    var valid: Bool {
        runs.count <= 64 && recipes.count <= 16 && notices.count <= 16 &&
        runs.allSatisfy(\.valid) && (selected_run?.valid ?? true)
    }
}

struct GymRun: Decodable, Identifiable {
    let id: String
    let title: String
    let category: String
    let status: String
    let completed: UInt64?
    let total: UInt64?
    let cost_usd: Double?
    let elapsed_ms: UInt64?
    let metrics: [GymMetric]
    let source: String
    let provenance: String
    var valid: Bool {
        metrics.count <= 4 && metrics.allSatisfy { metric in
            metric.points.count <= 64 && metric.points.allSatisfy { $0.value.isFinite }
        } && (cost_usd.map { $0.isFinite && $0 >= 0 } ?? true)
    }
}
struct GymMetric: Decodable { let name: String; let unit: String; let points: [GymPoint] }
struct GymPoint: Decodable { let step: UInt64; let value: Double }
struct GymRecipe: Decodable, Identifiable {
    let id: String
    let title: String
    let revision: String
    let budget: GymBudget
    let detail: String
}
struct GymBudget: Decodable {
    let wall_ms: UInt64
    let max_starts: UInt32
    let spend_limit_usd: Double?
    let spend_enforced: Bool
}
struct GymLaunch: Decodable {
    let request_id: String
    let phase: String
    let error: String?
    let receipt: GymLaunchReceipt?
}
struct GymLaunchReceipt: Decodable {
    let request_id: String
    let run_id: String
    let recipe_id: String
    let revision: String
    let status: String
    let submitted_at: UInt64
    let finished_at: UInt64?
    let exit_code: Int32?
}

struct GymPanel: View {
    @ObservedObject var bridge: VerseBridge
    let close: () -> Void
    @State private var configuring = false
    @State private var code = ""
    private var board: GymBoardView? { bridge.gymBoard }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Label("Gym", systemImage: "chart.xyaxis.line").font(.headline)
                Spacer()
                Button("Back to world", systemImage: "xmark", action: close)
                    .labelStyle(.iconOnly).accessibilityIdentifier("gym-close")
            }
            if let board {
                Text(board.status).font(.caption).accessibilityIdentifier("gym-status")
                if board.stale { Text("Snapshot is stale. New starts are unavailable.").font(.caption) }
                if let error = board.error ?? bridge.packet?.error ?? bridge.gymStorageError ?? bridge.nativeError {
                    Text(error).font(.callout).textSelection(.enabled).accessibilityIdentifier("gym-error")
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
                            Text(notice).font(.caption).textSelection(.enabled)
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
            Text("Connect a Gym host").font(.headline)
            Text("Create a Gym connection grant on your host for this device public key, then paste its gym-connect: code.")
            Text(board.public_key).font(.system(.caption2, design: .monospaced)).textSelection(.enabled)
                .accessibilityIdentifier("gym-public-key")
            Button("Copy public key", systemImage: "doc.on.doc") { UIPasteboard.general.string = board.public_key }
            TextEditor(text: $code).frame(minHeight: 85, maxHeight: 130)
                .textInputAutocapitalization(.never).autocorrectionDisabled()
                .accessibilityLabel("Gym connection code").accessibilityIdentifier("gym-code")
            Button("Connect Gym") {
                if bridge.configureGym(code) { code = ""; configuring = false }
            }.disabled(code.isEmpty).accessibilityIdentifier("gym-connect")
            Text("This grant is separate from saved-chat access. Only the host's listed recipes can start, after you confirm.")
                .font(.caption)
            if board.configured { Button("Back to board") { configuring = false } }
        }
    }

    private func runList(_ board: GymBoardView) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Microcoder and Terminal-Bench runs").font(.headline)
            if board.runs.isEmpty { Text("No runs are available in this snapshot.") }
            ForEach(board.runs.sorted { rank($0.category) < rank($1.category) }) { run in
                Button {
                    bridge.send(["action": "gym_select_run", "id": run.id])
                } label: {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(run.title).font(.headline)
                        Text("\(run.category) · \(run.status)").font(.caption)
                        progress(run)
                        Text(summary(run)).font(.caption)
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }.accessibilityIdentifier("gym-run-\(run.id)")
                Divider()
            }
            Text("Supported new runs").font(.headline)
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
            Text(run.title).font(.headline).accessibilityIdentifier("gym-run-title")
            Text("\(run.category) · \(run.status)")
            progress(run)
            Text(summary(run)).font(.callout)
            ForEach(Array(run.metrics.enumerated()), id: \.offset) { _, metric in
                VStack(alignment: .leading, spacing: 6) {
                    Text("\(metric.name) (\(metric.unit))").font(.headline)
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
                                    .font(.caption).textSelection(.enabled)
                            }
                        }
                    }
                }
            }
            Text("Source: \(run.source)").font(.caption).textSelection(.enabled)
            Text(run.provenance).font(.caption).textSelection(.enabled)
            Text("Completed describes the recorded process; it does not by itself establish benchmark success.")
                .font(.caption)
        }
    }

    private func recipeDetails(_ recipe: GymRecipe, board: GymBoardView) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(recipe.title).font(.headline)
            Text(recipe.detail).textSelection(.enabled)
            Text("Time limit: \(recipe.budget.wall_ms / 1000) seconds · Maximum starts: \(recipe.budget.max_starts)")
            Text(recipe.budget.spend_enforced ? "The host enforces this recipe's spending limit." : "No dollar limit is enforced for this recipe.")
            Text("Recipe revision: \(recipe.revision)").font(.caption2.monospaced()).textSelection(.enabled)
            Text("Starting submits this exact recipe to the host. Leaving the Gym does not cancel the run.")
            Button("Start this run") { bridge.send(["action": "gym_launch"]) }
                .disabled(!board.active || board.stale || ["sending", "unknown"].contains(board.launch?.phase ?? ""))
                .accessibilityIdentifier("gym-confirm-launch")
        }
    }

    private func launchStatus(_ launch: GymLaunch, active: Bool) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Request: \(launch.phase)").font(.headline).accessibilityIdentifier("gym-launch-status")
            Text(launch.request_id).font(.caption2.monospaced()).textSelection(.enabled)
            if let receipt = launch.receipt {
                Text("Host receipt: \(receipt.status) · Run \(receipt.run_id)").font(.caption)
            }
            if let error = launch.error { Text(error).font(.caption) }
            if launch.phase == "sending" { ProgressView("Waiting for the host receipt…") }
            if launch.phase == "unknown" {
                Text("The host may already have accepted this request. Retry uses the same request identity.").font(.caption)
                Button("Retry the same request") { bridge.send(["action": "gym_retry"]) }
                    .disabled(!active).accessibilityIdentifier("gym-retry")
            }
        }
    }

    @ViewBuilder private func progress(_ run: GymRun) -> some View {
        if let completed = run.completed, let total = run.total {
            Text("\(completed) / \(total) recorded").font(.caption)
            if total > 0 { ProgressView(value: Double(completed), total: Double(total)) }
        } else { Text("Progress unavailable").font(.caption) }
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
