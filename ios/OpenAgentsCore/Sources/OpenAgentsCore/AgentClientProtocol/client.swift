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
    public struct ContentChunk: Codable {
        public var content: ContentBlock
        public var _meta: [String: AnyEncodable]? = nil
        public init(content: ContentBlock, _meta: [String: AnyEncodable]? = nil) {
            self.content = content; self._meta = _meta
        }
    }

    /// Content block variants mirroring Rust `content.rs`.
    public enum ContentBlock: Codable {
        case text(TextContent)
        case image(ImageContent)
        case audio(AudioContent)
        case resource_link(ResourceLink)
        case resource(EmbeddedResource)

        private enum CodingKeys: String, CodingKey { case type }
        private enum Discriminator: String, Codable { case text, image, audio, resource_link, resource }

        public init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            let t = try c.decode(Discriminator.self, forKey: .type)
            switch t {
            case .text:
                self = .text(try TextContent(from: decoder))
            case .image:
                self = .image(try ImageContent(from: decoder))
            case .audio:
                self = .audio(try AudioContent(from: decoder))
            case .resource_link:
                self = .resource_link(try ResourceLink(from: decoder))
            case .resource:
                self = .resource(try EmbeddedResource(from: decoder))
            }
        }
        public func encode(to encoder: Encoder) throws {
            switch self {
            case .text(let v): try v.encode(to: encoder)
            case .image(let v): try v.encode(to: encoder)
            case .audio(let v): try v.encode(to: encoder)
            case .resource_link(let v): try v.encode(to: encoder)
            case .resource(let v): try v.encode(to: encoder)
            }
        }

        // MARK: - Nested types mirroring Rust
        public struct TextContent: Codable {
            public var annotations: Annotations?
            public var text: String
            public var _meta: [String: AnyEncodable]?
            private enum CodingKeys: String, CodingKey { case type, annotations, text, _meta }
            public init(annotations: Annotations? = nil, text: String, _meta: [String: AnyEncodable]? = nil) {
                self.annotations = annotations; self.text = text; self._meta = _meta
            }
            public init(from decoder: Decoder) throws {
                let c = try decoder.container(keyedBy: CodingKeys.self)
                _ = try? c.decode(String.self, forKey: .type) // "text"
                self.annotations = try? c.decode(Annotations.self, forKey: .annotations)
                self.text = try c.decode(String.self, forKey: .text)
                self._meta = try? c.decode([String: AnyEncodable].self, forKey: ._meta)
            }
            public func encode(to encoder: Encoder) throws {
                var c = encoder.container(keyedBy: CodingKeys.self)
                try c.encode("text", forKey: .type)
                if let a = annotations { try c.encode(a, forKey: .annotations) }
                try c.encode(text, forKey: .text)
                if let m = _meta { try c.encode(m, forKey: ._meta) }
            }
        }
        public struct ImageContent: Codable {
            public var annotations: Annotations?
            public var data: String
            public var mimeType: String
            public var uri: String?
            public var _meta: [String: AnyEncodable]?
            private enum CodingKeys: String, CodingKey { case type, annotations, data, mimeType, uri, _meta }
            public init(annotations: Annotations? = nil, data: String, mimeType: String, uri: String? = nil, _meta: [String: AnyEncodable]? = nil) {
                self.annotations = annotations; self.data = data; self.mimeType = mimeType; self.uri = uri; self._meta = _meta
            }
            public init(from decoder: Decoder) throws {
                let c = try decoder.container(keyedBy: CodingKeys.self)
                _ = try? c.decode(String.self, forKey: .type)
                self.annotations = try? c.decode(Annotations.self, forKey: .annotations)
                self.data = try c.decode(String.self, forKey: .data)
                self.mimeType = try c.decode(String.self, forKey: .mimeType)
                self.uri = try? c.decode(String.self, forKey: .uri)
                self._meta = try? c.decode([String: AnyEncodable].self, forKey: ._meta)
            }
            public func encode(to encoder: Encoder) throws {
                var c = encoder.container(keyedBy: CodingKeys.self)
                try c.encode("image", forKey: .type)
                if let a = annotations { try c.encode(a, forKey: .annotations) }
                try c.encode(data, forKey: .data)
                try c.encode(mimeType, forKey: .mimeType)
                if let u = uri { try c.encode(u, forKey: .uri) }
                if let m = _meta { try c.encode(m, forKey: ._meta) }
            }
        }
        public struct AudioContent: Codable {
            public var annotations: Annotations?
            public var data: String
            public var mimeType: String
            public var _meta: [String: AnyEncodable]?
            private enum CodingKeys: String, CodingKey { case type, annotations, data, mimeType, _meta }
            public init(annotations: Annotations? = nil, data: String, mimeType: String, _meta: [String: AnyEncodable]? = nil) {
                self.annotations = annotations; self.data = data; self.mimeType = mimeType; self._meta = _meta
            }
            public init(from decoder: Decoder) throws {
                let c = try decoder.container(keyedBy: CodingKeys.self)
                _ = try? c.decode(String.self, forKey: .type)
                self.annotations = try? c.decode(Annotations.self, forKey: .annotations)
                self.data = try c.decode(String.self, forKey: .data)
                self.mimeType = try c.decode(String.self, forKey: .mimeType)
                self._meta = try? c.decode([String: AnyEncodable].self, forKey: ._meta)
            }
            public func encode(to encoder: Encoder) throws {
                var c = encoder.container(keyedBy: CodingKeys.self)
                try c.encode("audio", forKey: .type)
                if let a = annotations { try c.encode(a, forKey: .annotations) }
                try c.encode(data, forKey: .data)
                try c.encode(mimeType, forKey: .mimeType)
                if let m = _meta { try c.encode(m, forKey: ._meta) }
            }
        }
        public struct EmbeddedResource: Codable {
            public var annotations: Annotations?
            public var resource: EmbeddedResourceResource
            public var _meta: [String: AnyEncodable]?
            private enum CodingKeys: String, CodingKey { case type, annotations, resource, _meta }
            public init(annotations: Annotations? = nil, resource: EmbeddedResourceResource, _meta: [String: AnyEncodable]? = nil) {
                self.annotations = annotations; self.resource = resource; self._meta = _meta
            }
            public init(from decoder: Decoder) throws {
                let c = try decoder.container(keyedBy: CodingKeys.self)
                _ = try? c.decode(String.self, forKey: .type)
                self.annotations = try? c.decode(Annotations.self, forKey: .annotations)
                self.resource = try c.decode(EmbeddedResourceResource.self, forKey: .resource)
                self._meta = try? c.decode([String: AnyEncodable].self, forKey: ._meta)
            }
            public func encode(to encoder: Encoder) throws {
                var c = encoder.container(keyedBy: CodingKeys.self)
                try c.encode("resource", forKey: .type)
                if let a = annotations { try c.encode(a, forKey: .annotations) }
                try c.encode(resource, forKey: .resource)
                if let m = _meta { try c.encode(m, forKey: ._meta) }
            }
        }
        public enum EmbeddedResourceResource: Codable {
            case text(TextResourceContents)
            case blob(BlobResourceContents)
            private enum CodingKeys: String, CodingKey { case mimeType, text, uri, blob }
            public init(from decoder: Decoder) throws {
                let c = try decoder.container(keyedBy: CodingKeys.self)
                if let _ = try? c.decode(String.self, forKey: .text) {
                    self = .text(try TextResourceContents(from: decoder))
                } else {
                    self = .blob(try BlobResourceContents(from: decoder))
                }
            }
            public func encode(to encoder: Encoder) throws {
                switch self {
                case .text(let v): try v.encode(to: encoder)
                case .blob(let v): try v.encode(to: encoder)
                }
            }
        }
        public struct TextResourceContents: Codable {
            public var mimeType: String?
            public var text: String
            public var uri: String
            public var _meta: [String: AnyEncodable]?
            private enum CodingKeys: String, CodingKey { case mimeType, text, uri, _meta }
            public init(mimeType: String? = nil, text: String, uri: String, _meta: [String: AnyEncodable]? = nil) {
                self.mimeType = mimeType; self.text = text; self.uri = uri; self._meta = _meta
            }
        }
        public struct BlobResourceContents: Codable {
            public var blob: String
            public var mimeType: String?
            public var uri: String
            public var _meta: [String: AnyEncodable]?
            private enum CodingKeys: String, CodingKey { case blob, mimeType, uri, _meta }
            public init(blob: String, mimeType: String? = nil, uri: String, _meta: [String: AnyEncodable]? = nil) {
                self.blob = blob; self.mimeType = mimeType; self.uri = uri; self._meta = _meta
            }
        }
        public struct ResourceLink: Codable {
            public var annotations: Annotations?
            public var description: String?
            public var mimeType: String?
            public var name: String
            public var size: Int64?
            public var title: String?
            public var uri: String
            public var _meta: [String: AnyEncodable]?
            private enum CodingKeys: String, CodingKey { case type, annotations, description, mimeType, name, size, title, uri, _meta }
            public init(annotations: Annotations? = nil, description: String? = nil, mimeType: String? = nil, name: String, size: Int64? = nil, title: String? = nil, uri: String, _meta: [String: AnyEncodable]? = nil) {
                self.annotations = annotations; self.description = description; self.mimeType = mimeType; self.name = name; self.size = size; self.title = title; self.uri = uri; self._meta = _meta
            }
            public init(from decoder: Decoder) throws {
                let c = try decoder.container(keyedBy: CodingKeys.self)
                _ = try? c.decode(String.self, forKey: .type)
                self.annotations = try? c.decode(Annotations.self, forKey: .annotations)
                self.description = try? c.decode(String.self, forKey: .description)
                self.mimeType = try? c.decode(String.self, forKey: .mimeType)
                self.name = try c.decode(String.self, forKey: .name)
                self.size = try? c.decode(Int64.self, forKey: .size)
                self.title = try? c.decode(String.self, forKey: .title)
                self.uri = try c.decode(String.self, forKey: .uri)
                self._meta = try? c.decode([String: AnyEncodable].self, forKey: ._meta)
            }
            public func encode(to encoder: Encoder) throws {
                var c = encoder.container(keyedBy: CodingKeys.self)
                try c.encode("resource_link", forKey: .type)
                if let a = annotations { try c.encode(a, forKey: .annotations) }
                if let d = description { try c.encode(d, forKey: .description) }
                if let m = mimeType { try c.encode(m, forKey: .mimeType) }
                try c.encode(name, forKey: .name)
                if let s = size { try c.encode(s, forKey: .size) }
                if let t = title { try c.encode(t, forKey: .title) }
                try c.encode(uri, forKey: .uri)
                if let meta = _meta { try c.encode(meta, forKey: ._meta) }
            }
        }
        public struct Annotations: Codable {
            public var audience: [Role]?
            public var lastModified: String?
            public var priority: Double?
            public var _meta: [String: AnyEncodable]?
            private enum CodingKeys: String, CodingKey { case audience, lastModified, priority, _meta }
            public init(audience: [Role]? = nil, lastModified: String? = nil, priority: Double? = nil, _meta: [String: AnyEncodable]? = nil) {
                self.audience = audience; self.lastModified = lastModified; self.priority = priority; self._meta = _meta
            }
        }
        public enum Role: String, Codable, Equatable { case assistant, user }
    }

    /// Different types of updates that can be sent during session processing.
    /// Mirrors `SessionUpdate` in Rust `client.rs` (subset variants for now).
    public enum SessionUpdate: Codable {
        case userMessageChunk(ContentChunk)
        case agentMessageChunk(ContentChunk)
        case agentThoughtChunk(ContentChunk)
        case plan(Plan)
        case availableCommandsUpdate(AvailableCommandsUpdate)
        case currentModeUpdate(CurrentModeUpdate)
        case toolCall(ACPToolCallWire)
        case toolCallUpdate(ACPToolCallUpdateWire)
        // The rest (ToolCall, ToolCallUpdate, AvailableCommandsUpdate, CurrentModeUpdate) can be added next.

        private enum CodingKeys: String, CodingKey { case sessionUpdate }
        private enum Discriminator: String, Codable {
            case user_message_chunk
            case agent_message_chunk
            case agent_thought_chunk
            case plan
            case available_commands_update
            case current_mode_update
            case tool_call
            case tool_call_update
        }
        private enum PayloadKeys: String, CodingKey { case content, plan, available_commands, current_mode_id, tool_call, tool_call_update }

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
            case .available_commands_update:
                let ac = try p.decode([AvailableCommand].self, forKey: .available_commands)
                self = .availableCommandsUpdate(AvailableCommandsUpdate(available_commands: ac))
            case .current_mode_update:
                let mode = try p.decode(ACPSessionModeId.self, forKey: .current_mode_id)
                self = .currentModeUpdate(CurrentModeUpdate(current_mode_id: mode))
            case .tool_call:
                self = .toolCall(try p.decode(ACPToolCallWire.self, forKey: .tool_call))
            case .tool_call_update:
                self = .toolCallUpdate(try p.decode(ACPToolCallUpdateWire.self, forKey: .tool_call_update))
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
            case .availableCommandsUpdate(let ac):
                try c.encode(Discriminator.available_commands_update, forKey: .sessionUpdate)
                try p.encode(ac.available_commands, forKey: .available_commands)
            case .currentModeUpdate(let cur):
                try c.encode(Discriminator.current_mode_update, forKey: .sessionUpdate)
                try p.encode(cur.current_mode_id, forKey: .current_mode_id)
            case .toolCall(let call):
                try c.encode(Discriminator.tool_call, forKey: .sessionUpdate)
                try p.encode(call, forKey: .tool_call)
            case .toolCallUpdate(let upd):
                try c.encode(Discriminator.tool_call_update, forKey: .sessionUpdate)
                try p.encode(upd, forKey: .tool_call_update)
            }
        }
    }

    // Plan type now mirrors ACP Rust Plan via ACPPlan
    public typealias Plan = ACPPlan

    public struct AvailableCommandsUpdate: Codable, Equatable {
        public var available_commands: [AvailableCommand]
        public init(available_commands: [AvailableCommand]) { self.available_commands = available_commands }
    }
    public struct CurrentModeUpdate: Codable, Equatable {
        public var current_mode_id: ACPSessionModeId
        public init(current_mode_id: ACPSessionModeId) { self.current_mode_id = current_mode_id }
    }
    public struct AvailableCommand: Codable, Equatable {
        public var name: String
        public var description: String
        public var input: AvailableCommandInput?
        public init(name: String, description: String, input: AvailableCommandInput? = nil) {
            self.name = name; self.description = description; self.input = input
        }
    }
    public enum AvailableCommandInput: Codable, Equatable {
        case unstructured(hint: String)

        private enum CodingKeys: String, CodingKey { case kind = "type", hint }
        private enum Discriminator: String, Codable { case unstructured }
        public init(from decoder: Decoder) throws {
            let c = try decoder.container(keyedBy: CodingKeys.self)
            let kind = try c.decode(Discriminator.self, forKey: .kind)
            switch kind {
            case .unstructured:
                self = .unstructured(hint: (try? c.decode(String.self, forKey: .hint)) ?? "")
            }
        }
        public func encode(to encoder: Encoder) throws {
            var c = encoder.container(keyedBy: CodingKeys.self)
            switch self {
            case .unstructured(let hint):
                try c.encode(Discriminator.unstructured, forKey: .kind)
                try c.encode(hint, forKey: .hint)
            }
        }
    }
}
