import Foundation

/*!
 Methods and notifications the client handles/receives.

 This file mirrors the ACP Rust SDK `client.rs` where client-side streamed
 updates (`session/update`) and client-handled requests (fs/terminal/permission)
 are defined. Below is a subset sufficient to scaffold streaming and UI
 rendering; the surface can be extended to one-to-one parity.
*/

public extension ACP.Client {
    /// A streamed item of content (text/tool/etc.) used by update chunks.
    /// Mirrors `ContentChunk` in Rust `client.rs`.
    struct ContentChunk: Codable {
        var content: ContentBlock
        var _meta: [String: AnyEncodable]? = nil
        public init(content: ContentBlock, _meta: [String: AnyEncodable]? = nil) {
            self.content = content; self._meta = _meta
        }
    }

    /// Content block variants (subset: text only for now).
    /// Mirrors `ContentBlock` (full surface TBD).
    enum ContentBlock: Codable {
        case text(String)
        case resource_link(ResourceLink)
        case image(Image)

        private enum CodingKeys: String, CodingKey { case type, text, url, title, alt }
        private enum Discriminator: String, Codable { case text, resource_link, image }

        public init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            let t = try c.decode(Discriminator.self, forKey: .type)
            switch t {
            case .text:
                self = .text(try c.decode(String.self, forKey: .text))
            case .resource_link:
                let url = try c.decode(String.self, forKey: .url)
                let title = try? c.decode(String.self, forKey: .title)
                self = .resource_link(ResourceLink(url: url, title: title))
            case .image:
                let url = try c.decode(String.self, forKey: .url)
                let alt = try? c.decode(String.self, forKey: .alt)
                self = .image(Image(url: url, alt: alt))
            }
        }
        public func encode(to encoder: Encoder) throws {
            var c = encoder.container(keyedBy: CodingKeys.self)
            switch self {
            case .text(let s):
                try c.encode(Discriminator.text, forKey: .type)
                try c.encode(s, forKey: .text)
            case .resource_link(let link):
                try c.encode(Discriminator.resource_link, forKey: .type)
                try c.encode(link.url, forKey: .url)
                if let t = link.title { try c.encode(t, forKey: .title) }
            case .image(let img):
                try c.encode(Discriminator.image, forKey: .type)
                try c.encode(img.url, forKey: .url)
                if let a = img.alt { try c.encode(a, forKey: .alt) }
            }
        }

        public struct ResourceLink: Codable, Equatable {
            public var url: String
            public var title: String?
            public init(url: String, title: String? = nil) { self.url = url; self.title = title }
        }
        public struct Image: Codable, Equatable {
            public var url: String
            public var alt: String?
            public init(url: String, alt: String? = nil) { self.url = url; self.alt = alt }
        }
    }

    /// Different types of updates that can be sent during session processing.
    /// Mirrors `SessionUpdate` in Rust `client.rs` (subset variants for now).
    enum SessionUpdate: Codable {
        case userMessageChunk(ContentChunk)
        case agentMessageChunk(ContentChunk)
        case agentThoughtChunk(ContentChunk)
        case plan(Plan)
        // The rest (ToolCall, ToolCallUpdate, AvailableCommandsUpdate, CurrentModeUpdate) can be added next.

        private enum CodingKeys: String, CodingKey { case sessionUpdate }
        private enum Discriminator: String, Codable {
            case user_message_chunk
            case agent_message_chunk
            case agent_thought_chunk
            case plan
        }
        private enum PayloadKeys: String, CodingKey { case content, plan }

        public init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            let kind = try c.decode(Discriminator.self, forKey: .sessionUpdate)
            let p = try decoder.container(keyedBy: PayloadKeys.self)
            switch kind {
            case .user_message_chunk:
                self = .userMessageChunk(try p.decode(ContentChunk.self, forKey: .content))
            case .agent_message_chunk:
                self = .agentMessageChunk(try p.decode(ContentChunk.self, forKey: .content))
            case .agent_thought_chunk:
                self = .agentThoughtChunk(try p.decode(ContentChunk.self, forKey: .content))
            case .plan:
                self = .plan(try p.decode(Plan.self, forKey: .plan))
            }
        }
        public func encode(to encoder: Encoder) throws {
            var c = encoder.container(keyedBy: CodingKeys.self)
            var p = encoder.container(keyedBy: PayloadKeys.self)
            switch self {
            case .userMessageChunk(let chunk):
                try c.encode(Discriminator.user_message_chunk, forKey: .sessionUpdate)
                try p.encode(chunk, forKey: .content)
            case .agentMessageChunk(let chunk):
                try c.encode(Discriminator.agent_message_chunk, forKey: .sessionUpdate)
                try p.encode(chunk, forKey: .content)
            case .agentThoughtChunk(let chunk):
                try c.encode(Discriminator.agent_thought_chunk, forKey: .sessionUpdate)
                try p.encode(chunk, forKey: .content)
            case .plan(let plan):
                try c.encode(Discriminator.plan, forKey: .sessionUpdate)
                try p.encode(plan, forKey: .plan)
            }
        }
    }

    /// Minimal `Plan` placeholder to support plan updates and UI.
    struct Plan: Codable, Equatable {
        var steps: [String]
        public init(steps: [String]) { self.steps = steps }
    }
}
