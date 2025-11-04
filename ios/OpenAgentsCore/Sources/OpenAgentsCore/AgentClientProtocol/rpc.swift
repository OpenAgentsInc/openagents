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
    public static let sessionLoad = "session/load"          // optional
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
}

