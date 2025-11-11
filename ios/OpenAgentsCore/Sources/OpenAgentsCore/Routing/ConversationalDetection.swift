import Foundation

/// Utility for detecting conversational questions vs coding tasks
/// Used for intelligent routing between orchestrator and specialized agents
public enum ConversationalDetection {
    /// Detect if a prompt is a conversational question rather than a coding task
    public static func isConversational(_ text: String) -> Bool {
        let lower = text.lowercased().trimmingCharacters(in: .whitespacesAndNewlines)

        // Greetings
        let greetings = ["hi", "hello", "hey", "greetings"]
        if greetings.contains(lower) { return true }

        // Identity/capability questions
        let identityPatterns = [
            "who are you", "what are you", "tell me about yourself",
            "what can you do", "what do you do", "what are your capabilities",
            "how do you work", "how can you help", "can you help me",
            "what is openagents", "what's openagents"
        ]
        for pattern in identityPatterns {
            if lower.contains(pattern) { return true }
        }

        // Exclude coding/file-related prompts (these should go to coding agents)
        let codingIndicators = [
            "file", "code", "function", "class", "bug", "error",
            "implement", "refactor", "fix", "debug", "test",
            "build", "run", "compile", "install", ".swift", ".py",
            ".js", ".ts", "git", "package", "dependency"
        ]
        for indicator in codingIndicators {
            if lower.contains(indicator) { return false }
        }

        // Simple questions (ending with ?)
        if lower.hasSuffix("?") && lower.count < 100 {
            // Short questions without coding keywords are likely conversational
            return true
        }

        return false
    }
}
