// ContentSpanTool.swift — Bounded file reading with content.get_span semantics
// Part of Phase 2: orchestrate.explore.start implementation

import Foundation

/// Tool for reading bounded spans from files
public struct ContentSpanTool: Sendable {
    private let workspaceRoot: String
    private let fileManager = FileManager.default

    public init(workspaceRoot: String) {
        self.workspaceRoot = workspaceRoot
    }

    /// Read a span of lines from a file with optional context
    public func readSpan(
        path: String,
        startLine: Int,
        endLine: Int,
        context: Int? = nil
    ) async throws -> ContentSpanResult {
        // Validate and resolve path
        let fullPath = try resolvePath(path)

        // Read file content
        let content = try String(contentsOfFile: fullPath, encoding: .utf8)
        let allLines = content.components(separatedBy: "\n")

        // Validate line range
        guard startLine > 0 && endLine >= startLine else {
            throw ToolExecutionError.invalidParameters("Invalid line range: \(startLine)-\(endLine)")
        }

        // Convert to 0-indexed
        let startIdx = startLine - 1
        let endIdx = min(endLine - 1, allLines.count - 1)

        // Apply context if specified
        let contextLines = context ?? 0
        let actualStartIdx = max(0, startIdx - contextLines)
        let actualEndIdx = min(allLines.count - 1, endIdx + contextLines)

        // Extract lines
        var extractedLines = Array(allLines[actualStartIdx...actualEndIdx])

        // Check bounds and truncate if needed
        var truncated = false
        let originalSize = extractedLines.reduce(0) { $0 + $1.utf8.count + 1 } // +1 for newline

        // Apply line limit
        if extractedLines.count > MAX_INLINE_LINES {
            extractedLines = Array(extractedLines.prefix(MAX_INLINE_LINES))
            truncated = true
        }

        // Apply byte limit
        var totalBytes = 0
        var linesToKeep = 0
        for line in extractedLines {
            let lineBytes = line.utf8.count + 1 // +1 for newline
            if totalBytes + lineBytes > MAX_INLINE_BYTES {
                truncated = true
                break
            }
            totalBytes += lineBytes
            linesToKeep += 1
        }

        if linesToKeep < extractedLines.count {
            extractedLines = Array(extractedLines.prefix(linesToKeep))
        }

        // Normalize line endings to \n (already done by components(separatedBy:))
        // Normalize encoding to UTF-8 (already done by String init)

        return ContentSpanResult(
            path: path,
            start_line: startLine,
            end_line: endLine,
            lines: extractedLines,
            truncated: truncated,
            original_size: truncated ? originalSize : nil,
            encoding: "utf-8"
        )
    }

    // MARK: - Path Validation

    /// Resolve and validate path within workspace
    private func resolvePath(_ path: String) throws -> String {
        // Remove any path traversal attempts
        let cleanPath = path.replacingOccurrences(of: "..", with: "")

        // Resolve full path
        let fullPath: String
        if path.hasPrefix("/") {
            fullPath = path
        } else {
            fullPath = (workspaceRoot as NSString).appendingPathComponent(cleanPath)
        }

        // Resolve symlinks and canonicalize
        let resolvedPath: String
        do {
            let url = URL(fileURLWithPath: fullPath)
            resolvedPath = try fileManager.destinationOfSymbolicLink(atPath: url.path)
        } catch {
            // Not a symlink, use as-is
            resolvedPath = (fullPath as NSString).standardizingPath
        }

        // Security check: ensure resolved path is within workspace
        let resolvedWorkspace = (workspaceRoot as NSString).standardizingPath
        guard resolvedPath.hasPrefix(resolvedWorkspace) else {
            throw ToolExecutionError.pathOutsideWorkspace(path)
        }

        // Check file exists
        guard fileManager.fileExists(atPath: resolvedPath) else {
            throw ToolExecutionError.fileNotFound(path)
        }

        // Check readable
        guard fileManager.isReadableFile(atPath: resolvedPath) else {
            throw ToolExecutionError.permissionDenied(path)
        }

        // Check not a directory
        var isDir: ObjCBool = false
        fileManager.fileExists(atPath: resolvedPath, isDirectory: &isDir)
        guard !isDir.boolValue else {
            throw ToolExecutionError.invalidParameters("\(path) is a directory")
        }

        // Check file size (skip very large files)
        let attrs = try fileManager.attributesOfItem(atPath: resolvedPath)
        let fileSize = attrs[.size] as? Int ?? 0
        if fileSize > 10 * 1024 * 1024 { // 10MB
            throw ToolExecutionError.executionFailed("File too large: \(fileSize) bytes")
        }

        return resolvedPath
    }
}

// MARK: - Binary Detection

extension ContentSpanTool {
    /// Check if file appears to be binary
    private func isBinaryFile(_ path: String) -> Bool {
        guard let data = fileManager.contents(atPath: path),
              data.count > 0 else {
            return false
        }

        // Check first 8KB for null bytes
        let sampleSize = min(8192, data.count)
        let sample = data.prefix(sampleSize)

        // If contains null bytes, likely binary
        return sample.contains(0x00)
    }
}
