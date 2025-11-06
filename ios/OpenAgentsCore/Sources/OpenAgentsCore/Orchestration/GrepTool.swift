// GrepTool.swift — Pattern searching with bounded results
// Part of Phase 2: orchestrate.explore.start implementation

import Foundation

/// Tool for searching patterns in workspace files
public struct GrepTool: Sendable {
    private let workspaceRoot: String
    private let fileManager = FileManager.default

    public init(workspaceRoot: String) {
        self.workspaceRoot = workspaceRoot
    }

    /// Search for pattern in workspace files
    public func grep(
        pattern: String,
        pathPrefix: String? = nil,
        caseInsensitive: Bool = false,
        maxResults: Int = 200
    ) async throws -> GrepResult {
        // Resolve search root
        let searchRoot: String
        if let prefix = pathPrefix {
            searchRoot = try resolveSearchPath(prefix)
        } else {
            searchRoot = workspaceRoot
        }

        // Compile regex
        var regexOptions: NSRegularExpression.Options = []
        if caseInsensitive {
            regexOptions.insert(.caseInsensitive)
        }

        let regex: NSRegularExpression
        do {
            regex = try NSRegularExpression(pattern: pattern, options: regexOptions)
        } catch {
            throw ToolExecutionError.invalidParameters("Invalid regex pattern: \(error.localizedDescription)")
        }

        // Search files
        var matches: [GrepMatch] = []
        var totalMatches = 0
        var truncated = false

        try await searchDirectory(
            searchRoot,
            regex: regex,
            matches: &matches,
            totalMatches: &totalMatches,
            maxResults: maxResults,
            truncated: &truncated
        )

        return GrepResult(
            pattern: pattern,
            matches: matches,
            truncated: truncated,
            total_matches: totalMatches
        )
    }

    // MARK: - Directory Search

    private func searchDirectory(
        _ dirPath: String,
        regex: NSRegularExpression,
        matches: inout [GrepMatch],
        totalMatches: inout Int,
        maxResults: Int,
        truncated: inout Bool
    ) async throws {
        // Get directory contents
        let contents = try fileManager.contentsOfDirectory(atPath: dirPath)

        for item in contents {
            // Stop if we've hit max results
            if matches.count >= maxResults {
                truncated = true
                return
            }

            let itemPath = (dirPath as NSString).appendingPathComponent(item)

            // Skip hidden files and common ignore patterns
            if shouldSkip(item) {
                continue
            }

            // Check if directory
            var isDir: ObjCBool = false
            fileManager.fileExists(atPath: itemPath, isDirectory: &isDir)

            if isDir.boolValue {
                // Recurse into directory
                try await searchDirectory(
                    itemPath,
                    regex: regex,
                    matches: &matches,
                    totalMatches: &totalMatches,
                    maxResults: maxResults,
                    truncated: &truncated
                )
            } else {
                // Search file
                try await searchFile(
                    itemPath,
                    regex: regex,
                    matches: &matches,
                    totalMatches: &totalMatches,
                    maxResults: maxResults
                )
            }
        }
    }

    // MARK: - File Search

    private func searchFile(
        _ filePath: String,
        regex: NSRegularExpression,
        matches: inout [GrepMatch],
        totalMatches: inout Int,
        maxResults: Int
    ) async throws {
        // Skip binary files
        if isBinaryFile(filePath) {
            return
        }

        // Read file
        let content: String
        do {
            content = try String(contentsOfFile: filePath, encoding: .utf8)
        } catch {
            // Skip files that can't be read as UTF-8
            return
        }

        // Search lines
        let lines = content.components(separatedBy: "\n")
        for (index, line) in lines.enumerated() {
            let range = NSRange(line.startIndex..<line.endIndex, in: line)
            if regex.firstMatch(in: line, options: [], range: range) != nil {
                totalMatches += 1

                // Add match if under limit
                if matches.count < maxResults {
                    let relativePath = relativePathFromWorkspace(filePath)
                    matches.append(GrepMatch(
                        path: relativePath,
                        line_number: index + 1, // 1-indexed
                        line: line,
                        context_before: nil, // Could add context in future
                        context_after: nil
                    ))
                }
            }
        }
    }

    // MARK: - Path Utilities

    private func resolveSearchPath(_ pathPrefix: String) throws -> String {
        let cleanPath = pathPrefix.replacingOccurrences(of: "..", with: "")

        let fullPath: String
        if pathPrefix.hasPrefix("/") {
            fullPath = pathPrefix
        } else {
            fullPath = (workspaceRoot as NSString).appendingPathComponent(cleanPath)
        }

        let resolvedPath = (fullPath as NSString).standardizingPath

        // Ensure within workspace
        let resolvedWorkspace = (workspaceRoot as NSString).standardizingPath
        guard resolvedPath.hasPrefix(resolvedWorkspace) else {
            throw ToolExecutionError.pathOutsideWorkspace(pathPrefix)
        }

        // Check exists
        guard fileManager.fileExists(atPath: resolvedPath) else {
            throw ToolExecutionError.fileNotFound(pathPrefix)
        }

        return resolvedPath
    }

    private func relativePathFromWorkspace(_ absolutePath: String) -> String {
        let workspacePrefix = (workspaceRoot as NSString).standardizingPath
        let absPath = (absolutePath as NSString).standardizingPath

        if absPath.hasPrefix(workspacePrefix) {
            let startIndex = absPath.index(absPath.startIndex, offsetBy: workspacePrefix.count)
            var relative = String(absPath[startIndex...])

            // Remove leading slash
            if relative.hasPrefix("/") {
                relative = String(relative.dropFirst())
            }

            return relative.isEmpty ? "." : relative
        }

        return absolutePath
    }

    // MARK: - Skip Patterns

    private func shouldSkip(_ filename: String) -> Bool {
        // Skip hidden files
        if filename.hasPrefix(".") && filename != "." {
            return true
        }

        // Skip common build/dependency directories
        let skipDirs = [
            "node_modules",
            ".git",
            ".svn",
            "build",
            "dist",
            "target",
            ".build",
            "DerivedData",
            "__pycache__",
            ".pytest_cache",
            ".tox",
            "venv",
            ".venv"
        ]

        if skipDirs.contains(filename) {
            return true
        }

        // Skip common binary/build artifacts
        let skipExtensions = [
            ".o", ".a", ".so", ".dylib",
            ".exe", ".dll",
            ".pyc", ".pyo",
            ".class", ".jar",
            ".png", ".jpg", ".gif", ".ico",
            ".pdf", ".zip", ".tar", ".gz"
        ]

        let ext = (filename as NSString).pathExtension.lowercased()
        if skipExtensions.contains(".\(ext)") {
            return true
        }

        return false
    }

    private func isBinaryFile(_ path: String) -> Bool {
        guard let data = fileManager.contents(atPath: path),
              data.count > 0 else {
            return false
        }

        // Check first 512 bytes for null bytes
        let sampleSize = min(512, data.count)
        let sample = data.prefix(sampleSize)

        return sample.contains(0x00)
    }
}
