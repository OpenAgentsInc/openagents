// WorkspaceScanner.swift — Directory listing and workspace discovery
// Part of Phase 2: orchestrate.explore.start implementation

import Foundation

/// Tool for scanning workspace directories
public struct WorkspaceScanner: Sendable {
    private let workspaceRoot: String
    private let fileManager = FileManager.default

    public init(workspaceRoot: String) {
        self.workspaceRoot = workspaceRoot
    }

    /// List directory contents with optional recursion
    public func listDirectory(
        path: String,
        depth: Int = 0
    ) async throws -> [String] {
        // Resolve path
        let fullPath = try resolvePath(path)

        // List contents
        var results: [String] = []
        try collectEntries(
            at: fullPath,
            relativeTo: workspaceRoot,
            currentDepth: 0,
            maxDepth: depth,
            results: &results
        )

        return results
    }

    /// Scan top-level files of interest (README, manifests, etc.)
    public func scanTopLevel() async throws -> [String] {
        let contents = try fileManager.contentsOfDirectory(atPath: workspaceRoot)

        var topFiles: [String] = []

        for item in contents {
            // Skip hidden files except .gitignore
            if item.hasPrefix(".") && item != ".gitignore" {
                continue
            }

            let itemPath = (workspaceRoot as NSString).appendingPathComponent(item)

            var isDir: ObjCBool = false
            fileManager.fileExists(atPath: itemPath, isDirectory: &isDir)

            if !isDir.boolValue {
                // Check if it's a file of interest
                if isFileOfInterest(item) {
                    topFiles.append(item)
                }
            }
        }

        // Sort by priority
        return topFiles.sorted { priorityOf($0) < priorityOf($1) }
    }

    /// Identify likely entry points (main files, app files, etc.)
    public func identifyEntryPoints() async throws -> [String] {
        var entryPoints: [String] = []

        // Search for common entry point patterns
        let patterns = [
            "main.swift", "main.ts", "main.js", "main.py", "main.rs", "main.go",
            "index.ts", "index.js", "index.html",
            "app.swift", "App.swift", "app.ts", "app.js",
            "server.ts", "server.js",
            "__init__.py"
        ]

        for pattern in patterns {
            if let found = try? findFile(name: pattern, maxDepth: 3) {
                entryPoints.append(contentsOf: found)
            }
        }

        return entryPoints
    }

    // MARK: - Collection

    private func collectEntries(
        at path: String,
        relativeTo root: String,
        currentDepth: Int,
        maxDepth: Int,
        results: inout [String]
    ) throws {
        guard currentDepth <= maxDepth else { return }

        let contents = try fileManager.contentsOfDirectory(atPath: path)

        for item in contents {
            // Skip hidden files
            if item.hasPrefix(".") {
                continue
            }

            let itemPath = (path as NSString).appendingPathComponent(item)
            let relativePath = relativePathFrom(root: root, absolute: itemPath)

            var isDir: ObjCBool = false
            fileManager.fileExists(atPath: itemPath, isDirectory: &isDir)

            if isDir.boolValue {
                results.append("\(relativePath)/")

                // Recurse if within depth limit
                if currentDepth < maxDepth {
                    try collectEntries(
                        at: itemPath,
                        relativeTo: root,
                        currentDepth: currentDepth + 1,
                        maxDepth: maxDepth,
                        results: &results
                    )
                }
            } else {
                results.append(relativePath)
            }
        }
    }

    // MARK: - File Search

    private func findFile(name: String, maxDepth: Int) throws -> [String] {
        var results: [String] = []
        try searchForFile(
            name: name,
            in: workspaceRoot,
            relativeTo: workspaceRoot,
            currentDepth: 0,
            maxDepth: maxDepth,
            results: &results
        )
        return results
    }

    private func searchForFile(
        name: String,
        in dirPath: String,
        relativeTo root: String,
        currentDepth: Int,
        maxDepth: Int,
        results: inout [String]
    ) throws {
        guard currentDepth <= maxDepth else { return }

        let contents = try fileManager.contentsOfDirectory(atPath: dirPath)

        for item in contents {
            if item.hasPrefix(".") {
                continue
            }

            let itemPath = (dirPath as NSString).appendingPathComponent(item)

            var isDir: ObjCBool = false
            fileManager.fileExists(atPath: itemPath, isDirectory: &isDir)

            if isDir.boolValue {
                // Recurse
                try searchForFile(
                    name: name,
                    in: itemPath,
                    relativeTo: root,
                    currentDepth: currentDepth + 1,
                    maxDepth: maxDepth,
                    results: &results
                )
            } else if item == name {
                let relativePath = relativePathFrom(root: root, absolute: itemPath)
                results.append(relativePath)
            }
        }
    }

    // MARK: - File Classification

    private func isFileOfInterest(_ filename: String) -> Bool {
        let interestingFiles = [
            "README.md", "README.txt", "README",
            "CLAUDE.md",
            "package.json", "Cargo.toml", "go.mod", "requirements.txt", "Pipfile",
            "Gemfile", "build.gradle", "pom.xml",
            "Makefile", "CMakeLists.txt",
            ".gitignore", "LICENSE", "LICENSE.md"
        ]

        return interestingFiles.contains(filename)
    }

    private func priorityOf(_ filename: String) -> Int {
        // Lower number = higher priority
        if filename.hasPrefix("README") { return 0 }
        if filename == "CLAUDE.md" { return 1 }
        if filename.hasPrefix("package") { return 2 }
        if filename.hasSuffix(".toml") || filename.hasSuffix(".json") { return 3 }
        if filename == "Makefile" { return 4 }
        return 10
    }

    // MARK: - Path Utilities

    private func resolvePath(_ path: String) throws -> String {
        let cleanPath = path.replacingOccurrences(of: "..", with: "")

        let fullPath: String
        if path == "." || path.isEmpty {
            fullPath = workspaceRoot
        } else if path.hasPrefix("/") {
            fullPath = path
        } else {
            fullPath = (workspaceRoot as NSString).appendingPathComponent(cleanPath)
        }

        let resolvedPath = (fullPath as NSString).standardizingPath

        // Ensure within workspace
        let resolvedWorkspace = (workspaceRoot as NSString).standardizingPath
        guard resolvedPath.hasPrefix(resolvedWorkspace) else {
            throw ToolExecutionError.pathOutsideWorkspace(path)
        }

        // Check exists
        guard fileManager.fileExists(atPath: resolvedPath) else {
            throw ToolExecutionError.fileNotFound(path)
        }

        return resolvedPath
    }

    private func relativePathFrom(root: String, absolute: String) -> String {
        let rootPath = (root as NSString).standardizingPath
        let absPath = (absolute as NSString).standardizingPath

        if absPath.hasPrefix(rootPath) {
            let startIndex = absPath.index(absPath.startIndex, offsetBy: rootPath.count)
            var relative = String(absPath[startIndex...])

            // Remove leading slash
            if relative.hasPrefix("/") {
                relative = String(relative.dropFirst())
            }

            return relative.isEmpty ? "." : relative
        }

        return absolute
    }
}
