import Foundation

#if canImport(FoundationModels)
import FoundationModels

@available(iOS 26.0, macOS 26.0, * )
enum FoundationModelSummarizer {
    /// Attempt to summarize the conversation title using Apple's on-device foundation model.
    /// Returns nil if summarization fails so callers can fall back.
    @available(iOS 26.0, macOS 26.0, * )
    static func trySummarizeTitle(messages: [ACPMessage]) async -> String? {
        // Keep the last ~10 user/assistant messages as plain text lines
        var seq = messages.filter { $0.role == .user || $0.role == .assistant }
        seq.sort { $0.ts < $1.ts }
        if seq.count > 10 { seq = Array(seq.suffix(10)) }
        let lines: [String] = seq.compactMap { m in
            let speaker = (m.role == .user) ? "User" : "Assistant"
            let text = m.parts.compactMap { part -> String? in
                if case let .text(t) = part { return t.text } else { return nil }
            }.joined(separator: " ")
            return text.isEmpty ? nil : "\(speaker): \(text)"
        }
        guard !lines.isEmpty else { return nil }

        let model = SystemLanguageModel.default
        switch model.availability {
        case .available:
            break
        case .unavailable(let reason):
            print("[FM] unavailable reason=\(String(describing: reason))")
            return nil
        }

        let instructions = Instructions("""
        You create very short, descriptive conversation titles.
        Output only 3-5 plain words, no punctuation, no quotes.
        """)

        let session = LanguageModelSession(model: model, tools: [], instructions: instructions)
        try? session.prewarm(promptPrefix: nil)

        let prompt = lines.joined(separator: "\n") + "\n\nTitle (3-5 words):"
        do {
            let options = GenerationOptions(temperature: 0.1)
            let resp = try await session.respond(to: prompt, options: options)
            // Response is LanguageModelSession.Response<String>
            #if swift(>=6.0)
            let raw: String = resp.output
            #else
            // Fallback in case property name differs across preview SDKs.
            let raw: String = (resp as? AnyObject)?.value(forKey: "output") as? String ?? String(describing: resp)
            #endif
            let cleaned = cleanup(raw)
            return cleaned.isEmpty ? nil : cleaned
        } catch {
            return nil
        }
    }

    private static func cleanup(_ s: String) -> String {
            let trimmed = s.trimmingCharacters(in: .whitespacesAndNewlines)
            .replacingOccurrences(of: #"["'.,;:()\[\]{}]+"#, with: "", options: .regularExpression)
        // Cap to 5 words
        let words = trimmed.split(whereSeparator: { $0.isWhitespace })
        return words.prefix(5).joined(separator: " ")
    }
}
#endif
