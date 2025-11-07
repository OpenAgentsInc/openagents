// PathUtils.swift — Utilities for normalizing model-provided paths to workspace-relative
// Public so tests and multiple orchestrators can reuse consistently.

import Foundation

public enum PathUtils {
    /// Normalize a user/model-provided path into a workspace-relative path string.
    /// Returns "." for the root. Never returns a path that escapes the workspace.
    public static func normalizeToWorkspaceRelative(workspaceRoot: String, inputPath: String) -> String {
        var path = inputPath.trimmingCharacters(in: .whitespacesAndNewlines)
        let name = (workspaceRoot as NSString).lastPathComponent
        let rootStd = (workspaceRoot as NSString).standardizingPath

        if path == "." || path == "/" || path.lowercased() == "workspace" || path == "/workspace" { return "." }

        // Handle common placeholder prefixes emitted by models, e.g. "/path/to", "path/to/..."
        if path == "/path/to" || path == "path/to" { return "." }
        if path.hasPrefix("/path/to/") { path.removeFirst("/path/to/".count) }
        if path.hasPrefix("path/to/") { path.removeFirst("path/to/".count) }

        if path.hasPrefix("/workspace/") { path.removeFirst("/workspace/".count) }

        if path.hasPrefix("/") {
            let std = (path as NSString).standardizingPath
            if std.hasPrefix(rootStd) {
                var rel = String(std.dropFirst(rootStd.count))
                if rel.hasPrefix("/") { rel.removeFirst() }
                path = rel
            }
        }

        if path == name { return "." }
        if path.hasPrefix(name + "/") { path.removeFirst(name.count + 1) }

        if path.hasPrefix("./") { path.removeFirst(2) }
        if path.hasSuffix("/") { path.removeLast() }
        return path.isEmpty ? "." : path
    }
}
