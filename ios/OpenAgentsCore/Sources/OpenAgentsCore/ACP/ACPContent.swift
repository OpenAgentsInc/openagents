import Foundation

// MARK: - Message content parts

public enum ACPContentPart: Equatable, Codable {
    case text(ACPText)
    case toolCall(ACPToolCall)
    case toolResult(ACPToolResult)

    private enum CodingKeys: String, CodingKey { case type }

    private enum Discriminator: String, Codable {
        case text
        case tool_call
        case tool_result
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let t = try container.decode(Discriminator.self, forKey: .type)
        switch t {
        case .text:
            self = .text(try ACPText(from: decoder))
        case .tool_call:
            self = .toolCall(try ACPToolCall(from: decoder))
        case .tool_result:
            self = .toolResult(try ACPToolResult(from: decoder))
        }
    }

    public func encode(to encoder: Encoder) throws {
        switch self {
        case .text(let v): try v.encode(to: encoder)
        case .toolCall(let v): try v.encode(to: encoder)
        case .toolResult(let v): try v.encode(to: encoder)
        }
    }
}

public struct ACPText: Equatable, Codable {
    public let type: String = "text"
    public var text: String

    public init(text: String) { self.text = text }
}

