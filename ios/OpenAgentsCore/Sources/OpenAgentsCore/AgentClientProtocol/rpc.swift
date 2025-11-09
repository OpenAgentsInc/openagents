import Foundation

/*!
 RPC method name constants mirroring the ACP Rust SDK `rpc.rs`.
 These strings are used in JSON‑RPC request/notification envelopes.
*/

public enum ACPRPC {
    // Initialization
    public static let initialize = "initialize"

    // Session lifecycle
    public static let sessionNew = "session/new"
    public static let sessionSetMode = "session/set_mode"
    public static let sessionPrompt = "session/prompt"
    public static let sessionCancel = "session/cancel"       // notification
    public static let sessionUpdate = "session/update"       // notification

    // Permissions
    public static let sessionRequestPermission = "session/request_permission"

    // File system (client-handled)
    public static let fsReadTextFile = "fs/read_text_file"
    public static let fsWriteTextFile = "fs/write_text_file"

    // Terminal (client-handled)
    public static let terminalRun = "terminal/run"
    public static let terminalCreate = "terminal/create"
    public static let terminalWrite = "terminal/write"
    public static let terminalOutput = "terminal/output"
    public static let terminalRelease = "terminal/release"
    public static let terminalKill = "terminal/kill"
    public static let terminalWaitForExit = "terminal/wait_for_exit"

    // Orchestration (Phase 2: on-device FM orchestrator)
    public static let orchestrateExploreStart = "orchestrate.explore.start"
    public static let orchestrateExploreStatus = "orchestrate.explore.status"
    public static let orchestrateExploreAbort = "orchestrate.explore.abort"

    // Orchestration Config (Phase 3: configuration & aiming)
    public static let orchestrateConfigGet = "orchestrate/config.get"
    public static let orchestrateConfigSet = "orchestrate/config.set"
    public static let orchestrateConfigList = "orchestrate/config.list"
    public static let orchestrateConfigActivate = "orchestrate/config.activate"

    // Orchestration Scheduler (Phase 3: lightweight stubs)
    public static let orchestrateSchedulerReload = "orchestrate/scheduler.reload"
    public static let orchestrateSchedulerStatus = "orchestrate/scheduler.status"
    public static let orchestrateSchedulerRunNow = "orchestrate/scheduler.run_now"       // trigger immediate run (testing/ops)
    public static let orchestrateSchedulerAdvance = "orchestrate/scheduler.advance"      // test-only alias for run_now

    // Orchestration Setup (conversational config creation)
    public static let orchestrateSetupStart = "orchestrate/setup.start"
    public static let orchestrateSetupStatus = "orchestrate/setup.status"
    public static let orchestrateSetupAbort = "orchestrate/setup.abort"
}
