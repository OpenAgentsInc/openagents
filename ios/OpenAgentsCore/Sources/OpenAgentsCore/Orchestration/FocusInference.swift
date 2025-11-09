import Foundation

// MARK: - Focus Inference

/// Infers glob patterns from high-level user selections (folders, languages)
///
/// Avoids exposing raw glob syntax to users. Instead, users pick:
/// - Top-level folders ("ios", "docs", "packages/core")
/// - Languages ("Swift", "Rust", "TypeScript")
///
/// This utility converts those selections into include/exclude glob patterns
/// and normalizes them with PathUtils.
public struct FocusInference {
    // MARK: - Language Mappings

    private static let languageGlobs: [String: [String]] = [
        "Swift": ["**/*.swift"],
        "Rust": ["**/*.rs", "**/Cargo.toml"],
        "TypeScript": ["**/*.ts", "**/*.tsx"],
        "JavaScript": ["**/*.js", "**/*.jsx"],
        "Python": ["**/*.py"],
        "Go": ["**/*.go"],
        "Java": ["**/*.java"],
        "Kotlin": ["**/*.kt"],
        "C": ["**/*.c", "**/*.h"],
        "C++": ["**/*.cpp", "**/*.hpp", "**/*.cc"],
        "C#": ["**/*.cs"],
        "Ruby": ["**/*.rb"],
        "PHP": ["**/*.php"],
        "Shell": ["**/*.sh", "**/*.bash", "**/*.zsh"]
    ]

    // MARK: - Common Excludes

    private static let defaultExcludes = [
        "**/node_modules/**",
        "**/.git/**",
        "**/build/**",
        "**/dist/**",
        "**/.next/**",
        "**/target/**",
        "**/.DS_Store",
        "**/Package.resolved",
        "**/yarn.lock",
        "**/package-lock.json"
    ]

    // MARK: - Inference

    /// Infer include/exclude patterns from user selections
    ///
    /// - Parameters:
    ///   - folders: Top-level folders to include (e.g., ["ios", "packages/core"])
    ///   - languages: Programming languages to focus on (e.g., ["Swift", "Rust"])
    ///   - workspaceRoot: Workspace root for normalization
    ///
    /// - Returns: Include patterns, exclude patterns, and human-readable summary
    public static func infer(
        folders: [String]?,
        languages: [String]?,
        workspaceRoot: String
    ) -> (include: [String], exclude: [String], summary: String) {
        var include: [String] = []
        var exclude = defaultExcludes
        var summaryParts: [String] = []

        // Add folder patterns
        if let folders = folders, !folders.isEmpty {
            for folder in folders {
                // Normalize to workspace-relative path
                let normalized = PathUtils.normalizeToWorkspaceRelative(
                    workspaceRoot: workspaceRoot,
                    inputPath: folder
                )
                include.append("\(normalized)/**")
            }
            summaryParts.append("Folders: \(folders.joined(separator: ", "))")
        }

        // Add language patterns
        if let languages = languages, !languages.isEmpty {
            for language in languages {
                if let globs = languageGlobs[language] {
                    // If folders specified, scope language patterns to those folders
                    if let folders = folders, !folders.isEmpty {
                        for folder in folders {
                            let normalized = PathUtils.normalizeToWorkspaceRelative(
                                workspaceRoot: workspaceRoot,
                                inputPath: folder
                            )
                            for glob in globs {
                                // Combine folder + language pattern
                                // e.g., "ios/**/*.swift"
                                let combined = "\(normalized)/\(glob)"
                                include.append(combined)
                            }
                        }
                    } else {
                        // No folder restrictions: use language globs globally
                        include.append(contentsOf: globs)
                    }
                }
            }
            summaryParts.append("Languages: \(languages.joined(separator: ", "))")
        }

        // If nothing specified, default to entire workspace
        if include.isEmpty {
            include = ["."]
            summaryParts.append("Entire workspace")
        }

        let summary = summaryParts.isEmpty ? "Entire workspace" : summaryParts.joined(separator: " | ")

        return (include, exclude, summary)
    }

    /// Parse natural language focus description into selections
    ///
    /// Example: "Swift files in ios folder" → folders: ["ios"], languages: ["Swift"]
    ///
    /// - Parameter description: Natural language focus description
    /// - Returns: Inferred folders and languages
    public static func parseNaturalLanguage(_ description: String) -> (folders: [String], languages: [String]) {
        let lowercased = description.lowercased()

        // Extract languages
        var languages: [String] = []
        for (language, _) in languageGlobs {
            if lowercased.contains(language.lowercased()) {
                languages.append(language)
            }
        }

        // Extract folder hints
        var folders: [String] = []
        let commonFolders = ["ios", "android", "web", "packages", "src", "lib", "docs", "tests"]
        for folder in commonFolders {
            if lowercased.contains(folder) {
                folders.append(folder)
            }
        }

        return (folders, languages)
    }
}
