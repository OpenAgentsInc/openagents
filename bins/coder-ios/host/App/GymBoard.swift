// Coder's native Verse packet for the Gym board, shared with the OpenAgents
// app's Verse tab. Rust owns the board's state; these types only decode the
// bounded `gym_board` view it sends when the host asks for it by revision.
import Foundation

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
