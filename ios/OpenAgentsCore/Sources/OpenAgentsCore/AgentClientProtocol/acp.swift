import Foundation

/*!
 [Agent Client Protocol](https://agentclientprotocol.com/)

 # Agent Client Protocol (ACP)

 The Agent Client Protocol standardizes communication between code editors
 (IDEs, text-editors, etc.) and coding agents (programs that use generative AI
 to autonomously modify code).

 ACP is transport-agnostic and typically serialized via JSON‑RPC 2.0 over a
 bidirectional stream (stdio, WebSocket, etc.).

 This Swift package mirrors the structure of the ACP Rust SDK (`acp.rs` and
 its submodules) to provide a one-to-one mapping of types and documentation
 comments. Files are organized to match the Rust crate layout where feasible.
*/

public enum ACP {
    // Namespace container for Agent-side types (handled by agents).
    public enum Agent {}
    // Namespace container for Client-side types (handled by clients/apps).
    public enum Client {}
}

