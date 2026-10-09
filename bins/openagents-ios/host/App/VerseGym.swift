// The Grid's Gym board in the Verse tab: the same board as Coder's Gym, in
// the OpenAgents app's white-on-black style. Rust owns the board's
// subscriptions, verified snapshots, recipe selection, authority, and launch
// identity; this panel renders what Rust sends and forwards choices. Keychain
// stores the Gym connection's exact bytes and nothing else.
import Charts
import Foundation
import Security
import SwiftUI
import UIKit

/// The host's Gym grant for this phone's world key, kept on this device only.
/// It is separate from the device key's host grants and from the world key.
enum VerseGymConnection {
    private static var query: [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: "com.openagents.app.gym",
         kSecAttrAccount as String: "grant-v1",
         kSecAttrSynchronizable as String: false]
    }

    static func load() throws -> String? {
        var request = query
        request[kSecReturnData as String] = true
        var result: CFTypeRef?
        let status = SecItemCopyMatching(request as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = result as? Data, data.count <= 65_536,
              let code = String(data: data, encoding: .utf8) else {
            throw DeviceKey.Failure.message("The saved Gym connection is unavailable. Unlock the device and try again.")
        }
        return code
    }

    static func save(_ code: String) throws {
        let data = Data(code.utf8)
        guard data.count <= 65_536 else {
            throw DeviceKey.Failure.message("That Gym connection code is too long.")
        }
        let attributes: [String: Any] = [kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly]
        var status = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
        if status == errSecItemNotFound {
            status = SecItemAdd(query.merging(attributes) { _, new in new } as CFDictionary, nil)
        }
        guard status == errSecSuccess else {
            throw DeviceKey.Failure.message("The Gym connection works for this session but could not be saved in Keychain.")
        }
    }
}

/// The Gym board panel. Each part of the board is its own section, so a new
/// data-driven board (a leaderboard, say) is one more section over the same
/// Rust-owned view.
struct VerseGymPanel: View {
    @ObservedObject var world: VerseWorld
    let close: () -> Void
    @State private var configuring = false
    @State private var code = ""
    private var board: GymBoardView? { world.gymBoard }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Label("Gym", systemImage: "chart.xyaxis.line").font(.paper(.headline))
                Spacer()
                Button("Back to world", systemImage: "xmark", action: close)
                    .labelStyle(.iconOnly).frame(width: 44, height: 44)
                    .accessibilityIdentifier("gym-close")
            }
            if let board {
                Text(board.status).font(.paper(.caption)).foregroundStyle(.secondary)
                    .accessibilityIdentifier("gym-status")
                if board.stale { Text("This board is out of date, so new runs can't start.").font(.paper(.caption)) }
                if let error = board.error ?? world.error ?? world.gymStorageError {
                    Text(error).font(.paper(.callout)).textSelection(.enabled).accessibilityIdentifier("gym-error")
                }
                ScrollView {
                    VStack(alignment: .leading, spacing: 14) {
                        if !board.configured || configuring {
                            connection(board)
                        } else if let run = board.selected_run {
                            Button("All runs") { world.send(["action": "gym_close_detail"]) }
                                .accessibilityIdentifier("gym-all-runs")
                            runDetails(run)
                        } else if let recipe = board.selected_recipe {
                            Button("All runs") { world.send(["action": "gym_close_detail"]) }
                            recipeDetails(recipe, board: board)
                        } else {
                            runList(board)
                            recipeList(board)
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
        .foregroundStyle(.white)
        .tint(.white)
        .background(Color(white: 0.04).opacity(0.97), in: RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(.white.opacity(0.55), lineWidth: 1))
    }

    private func connection(_ board: GymBoardView) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Connect a Gym host").font(.paper(.headline))
            Text("Create a Gym connection on your host for this world key, then paste its gym-connect: code.")
            Text(board.public_key).font(.paper(.caption2)).textSelection(.enabled)
                .accessibilityIdentifier("gym-public-key")
            Button("Copy public key", systemImage: "doc.on.doc") { UIPasteboard.general.string = board.public_key }
            TextEditor(text: $code).frame(minHeight: 85, maxHeight: 130)
                .scrollContentBackground(.hidden).background(Color(white: 0.1))
                .textInputAutocapitalization(.never).autocorrectionDisabled()
                .accessibilityLabel("Gym connection code").accessibilityIdentifier("gym-code")
            Button("Connect Gym") {
                if world.configureGym(code) { code = ""; configuring = false }
            }.disabled(code.isEmpty).accessibilityIdentifier("gym-connect")
            Text("This connection is separate from your computers' access. Only runs the host lists can start, and only after you confirm.")
                .font(.paper(.caption)).foregroundStyle(.secondary)
            if board.configured { Button("Back to board") { configuring = false } }
        }
    }

    private func runList(_ board: GymBoardView) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Microcoder and Terminal-Bench runs").font(.paper(.headline))
            if board.runs.isEmpty { Text("No runs are available in this snapshot.") }
            ForEach(board.runs.sorted { rank($0.category) < rank($1.category) }) { run in
                Button {
                    world.send(["action": "gym_select_run", "id": run.id])
                } label: {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(run.title).font(.paper(.headline))
                        Text("\(run.category) · \(run.status)").font(.paper(.caption)).foregroundStyle(.secondary)
                        progress(run)
                        Text(summary(run)).font(.paper(.caption))
                    }.frame(maxWidth: .infinity, alignment: .leading)
                }.accessibilityIdentifier("gym-run-\(run.id)")
                Divider().overlay(.white.opacity(0.3))
            }
        }
    }

    private func recipeList(_ board: GymBoardView) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Supported new runs").font(.paper(.headline))
            if board.recipes.isEmpty { Text("This connection has no supported start recipes.") }
            ForEach(board.recipes) { recipe in
                Button(recipe.title) { world.send(["action": "gym_select_recipe", "id": recipe.id]) }
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
                                .foregroundStyle(.white)
                            PointMark(x: .value("Step", point.step), y: .value(metric.unit, point.value))
                                .foregroundStyle(.white)
                        }
                        .chartXAxis { AxisMarks { AxisGridLine().foregroundStyle(.white.opacity(0.2)); AxisValueLabel().foregroundStyle(.gray) } }
                        .chartYAxis { AxisMarks { AxisGridLine().foregroundStyle(.white.opacity(0.2)); AxisValueLabel().foregroundStyle(.gray) } }
                        .frame(height: 160).accessibilityLabel("\(metric.name), \(metric.points.count) recorded points")
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
            Text(run.provenance).font(.paper(.caption)).foregroundStyle(.secondary).textSelection(.enabled)
            Text("Completed means the run finished, not that it passed.")
                .font(.paper(.caption)).foregroundStyle(.secondary)
        }
    }

    private func recipeDetails(_ recipe: GymRecipe, board: GymBoardView) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(recipe.title).font(.paper(.headline))
            Text(recipe.detail).textSelection(.enabled)
            Text("Time limit: \(recipe.budget.wall_ms / 1000) seconds · Maximum starts: \(recipe.budget.max_starts)")
            Text(recipe.budget.spend_enforced ? "The host enforces this recipe's spending limit." : "No dollar limit is enforced for this recipe.")
            Text("Recipe revision: \(recipe.revision)").font(.paper(.caption2)).textSelection(.enabled)
            Text("Starting sends this recipe to the host. Leaving the Gym does not cancel the run.")
            Button("Start this run") { world.send(["action": "gym_launch"]) }
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
                Text("We couldn't confirm the host got this. Retrying won't start it twice.").font(.paper(.caption))
                Button("Retry the same request") { world.send(["action": "gym_retry"]) }
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
