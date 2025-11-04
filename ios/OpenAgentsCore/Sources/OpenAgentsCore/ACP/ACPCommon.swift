import Foundation

// MARK: - Core enums and utilities shared by ACP models

public enum ACPRole: String, Codable, CaseIterable, Equatable {
    case system
    case user
    case assistant
    case tool
}

// Generic, strongly-typed JSON value to avoid `Any`.
public enum JSONValue: Equatable, Codable {
    case string(String)
    case number(Double)
    case object([String: JSONValue])
    case array([JSONValue])
    case bool(Bool)
    case null

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() { self = .null; return }
        if let b = try? container.decode(Bool.self) { self = .bool(b); return }
        if let n = try? container.decode(Double.self) { self = .number(n); return }
        if let s = try? container.decode(String.self) { self = .string(s); return }
        if let arr = try? container.decode([JSONValue].self) { self = .array(arr); return }
        if let obj = try? container.decode([String: JSONValue].self) { self = .object(obj); return }
        throw DecodingError.dataCorruptedError(in: container, debugDescription: "Unsupported JSON value")
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .string(let s): try container.encode(s)
        case .number(let n): try container.encode(n)
        case .bool(let b): try container.encode(b)
        case .array(let a): try container.encode(a)
        case .object(let o): try container.encode(o)
        case .null: try container.encodeNil()
        }
    }
}

// Stable identifiers: derive from source + index when not present.
public enum ACPId {
    public static func stableId(namespace: String, seed: String) -> String {
        // A simple, deterministic hash. Not cryptographic.
        var hasher = Hasher()
        hasher.combine(namespace)
        hasher.combine(seed)
        let h = hasher.finalize()
        return String(format: "%08x", h)
    }
}

