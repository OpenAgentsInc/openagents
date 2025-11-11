import Foundation

// MARK: - Config Loader Error

public enum ConfigLoaderError: LocalizedError {
    case fileNotFound(String)
    case invalidJSON(String)
    case validationFailed([String])
    case workspaceVariableNotSet

    public var errorDescription: String? {
        switch self {
        case .fileNotFound(let path):
            return "Config file not found: \(path)"
        case .invalidJSON(let message):
            return "Invalid JSON: \(message)"
        case .validationFailed(let errors):
            return "Validation failed:\n" + errors.joined(separator: "\n")
        case .workspaceVariableNotSet:
            return "$WORKSPACE variable not set. Please set WORKSPACE environment variable or replace $WORKSPACE in config."
        }
    }
}

// MARK: - Config Loader

/// Loads orchestration configurations from JSON files
public struct ConfigLoader {

    public init() {}

    /// Load orchestration config from JSON file
    /// - Parameter path: Path to JSON config file
    /// - Returns: Validated OrchestrationConfig
    /// - Throws: ConfigLoaderError if file not found, invalid, or fails validation
    public func load(from path: String) throws -> OrchestrationConfig {
        // Check file exists
        guard FileManager.default.fileExists(atPath: path) else {
            throw ConfigLoaderError.fileNotFound(path)
        }

        // Read file
        let data: Data
        do {
            data = try Data(contentsOf: URL(fileURLWithPath: path))
        } catch {
            throw ConfigLoaderError.fileNotFound(path)
        }

        // Parse JSON
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase

        let config: OrchestrationConfig
        do {
            config = try decoder.decode(OrchestrationConfig.self, from: data)
        } catch {
            throw ConfigLoaderError.invalidJSON(error.localizedDescription)
        }

        // Expand variables
        let expandedConfig = try expandVariables(in: config)

        // Validate config
        let errors = expandedConfig.validate()
        guard errors.isEmpty else {
            throw ConfigLoaderError.validationFailed(errors)
        }

        return expandedConfig
    }

    /// Load orchestration config from JSON string
    /// - Parameter json: JSON string
    /// - Returns: Validated OrchestrationConfig
    /// - Throws: ConfigLoaderError if invalid or fails validation
    public func loadFromString(_ json: String) throws -> OrchestrationConfig {
        guard let data = json.data(using: .utf8) else {
            throw ConfigLoaderError.invalidJSON("Could not convert string to UTF-8 data")
        }

        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase

        let config: OrchestrationConfig
        do {
            config = try decoder.decode(OrchestrationConfig.self, from: data)
        } catch {
            throw ConfigLoaderError.invalidJSON(error.localizedDescription)
        }

        // Expand variables
        let expandedConfig = try expandVariables(in: config)

        // Validate config
        let errors = expandedConfig.validate()
        guard errors.isEmpty else {
            throw ConfigLoaderError.validationFailed(errors)
        }

        return expandedConfig
    }

    /// Save orchestration config to JSON file
    /// - Parameters:
    ///   - config: Config to save
    ///   - path: Destination path
    /// - Throws: Error if save fails
    public func save(_ config: OrchestrationConfig, to path: String) throws {
        let encoder = JSONEncoder()
        encoder.keyEncodingStrategy = .convertToSnakeCase
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]

        let data = try encoder.encode(config)

        try data.write(to: URL(fileURLWithPath: path))
    }

    // MARK: - Private Helpers

    /// Expand environment variables in config
    /// Supports: $WORKSPACE, $HOME, $USER
    private func expandVariables(in config: OrchestrationConfig) throws -> OrchestrationConfig {
        var expandedConfig = config

        // Expand workspace_root
        expandedConfig.workspaceRoot = try expandVariable(config.workspaceRoot)

        return expandedConfig
    }

    private func expandVariable(_ value: String) throws -> String {
        var result = value

        // $WORKSPACE
        if result.contains("$WORKSPACE") {
            guard let workspace = ProcessInfo.processInfo.environment["WORKSPACE"] else {
                throw ConfigLoaderError.workspaceVariableNotSet
            }
            result = result.replacingOccurrences(of: "$WORKSPACE", with: workspace)
        }

        // $HOME
        if result.contains("$HOME") {
            let home = NSHomeDirectory()
            result = result.replacingOccurrences(of: "$HOME", with: home)
        }

        // $USER
        if result.contains("$USER") {
            let user = NSUserName()
            result = result.replacingOccurrences(of: "$USER", with: user)
        }

        return result
    }
}

// MARK: - Convenience Extensions

extension OrchestrationConfig {
    /// Load config from JSON file
    /// - Parameter path: Path to JSON config file
    /// - Returns: Validated OrchestrationConfig
    /// - Throws: ConfigLoaderError if file not found, invalid, or fails validation
    public static func load(from path: String) throws -> OrchestrationConfig {
        let loader = ConfigLoader()
        return try loader.load(from: path)
    }

    /// Load config from JSON string
    /// - Parameter json: JSON string
    /// - Returns: Validated OrchestrationConfig
    /// - Throws: ConfigLoaderError if invalid or fails validation
    public static func loadFromString(_ json: String) throws -> OrchestrationConfig {
        let loader = ConfigLoader()
        return try loader.loadFromString(json)
    }

    /// Save config to JSON file
    /// - Parameter path: Destination path
    /// - Throws: Error if save fails
    public func save(to path: String) throws {
        let loader = ConfigLoader()
        try loader.save(self, to: path)
    }
}
