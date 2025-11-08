# macOS: Capability Advertising (NIP-89)

**Phase:** 1 - MVP
**Component:** macOS App
**Priority:** P1 (High - Enables provider discovery)
**Estimated Effort:** 1 week

## Summary

Implement NIP-89 capability advertising for the macOS worker, publishing provider capabilities to Nostr relays so buyers can discover available services, pricing, and provider specifications.

## Motivation

Marketplace providers must advertise their capabilities so buyers can:
- **Discover providers**: Find compute providers offering specific job kinds
- **Compare pricing**: See cost estimates before submitting jobs
- **Check limits**: Understand max input size, timeout, token limits
- **Verify availability**: Know which providers are active

Without capability advertising, the marketplace is invisible - buyers don't know providers exist.

## Acceptance Criteria

### Capability Advertisement
- [ ] Publish NIP-89 events (kind:31990) for each supported job kind
- [ ] Include pricing model (base price, per-unit price)
- [ ] Include limits (max input size, max tokens, timeout)
- [ ] Include features (model, version, options)
- [ ] Include relay list (where to find this provider)
- [ ] Republish periodically (refresh every 24 hours)

### Configuration
- [ ] Enable/disable capability advertising
- [ ] Configure pricing per job kind
- [ ] Configure limits per job kind
- [ ] Configure advertising relays (where to publish ads)
- [ ] Configure features/metadata (custom fields)

### Lifecycle Management
- [ ] Publish capabilities on worker start
- [ ] Update capabilities when config changes
- [ ] Delete capabilities on worker stop (NIP-09 deletion event)
- [ ] Automatic refresh (keep ads fresh)

### UI (macOS)
- [ ] Worker status view (is advertising?)
- [ ] Preview capability ads (before publishing)
- [ ] Edit pricing/limits per job kind
- [ ] Test publish (verify ads appear on relay)

## Technical Design

### Module Structure

```swift
// ios/OpenAgentsCore/Sources/OpenAgentsCore/Worker/

CapabilityAdvertiser.swift   // NIP-89 capability advertising
CapabilityConfig.swift       // Capability configuration
```

### Core Types

```swift
// CapabilityAdvertiser.swift

import Foundation
import OpenAgentsCore

/// NIP-89 capability advertiser
public class CapabilityAdvertiser {
    private let config: CapabilityConfig
    private let nostrClient: NostrRelayManager
    private let workerPrivateKey: String
    private var advertisedEventIds: [JobKind: String] = [:]  // Track published events
    private var refreshTask: Task<Void, Never>?

    public init(
        config: CapabilityConfig,
        nostrClient: NostrRelayManager,
        workerPrivateKey: String
    ) {
        self.config = config
        self.nostrClient = nostrClient
        self.workerPrivateKey = workerPrivateKey
    }

    // MARK: - Lifecycle

    /// Start advertising capabilities
    public func start() async throws {
        guard config.enabled else { return }

        // Publish capabilities for all supported job kinds
        for jobKind in config.supportedJobKinds {
            try await publishCapability(for: jobKind)
        }

        // Schedule periodic refresh (every 24 hours)
        refreshTask = Task {
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 24 * 60 * 60 * 1_000_000_000) // 24h
                for jobKind in config.supportedJobKinds {
                    try? await publishCapability(for: jobKind)
                }
            }
        }

        logger.info("Capability advertising started for \(config.supportedJobKinds.count) job kinds")
    }

    /// Stop advertising (delete capability events)
    public func stop() async {
        refreshTask?.cancel()
        refreshTask = nil

        // Publish deletion events (NIP-09) for all advertised capabilities
        for (jobKind, eventId) in advertisedEventIds {
            await deleteCap ability(eventId: eventId)
        }

        advertisedEventIds.removeAll()
        logger.info("Capability advertising stopped")
    }

    /// Update capabilities (when config changes)
    public func updateCapabilities() async throws {
        for jobKind in config.supportedJobKinds {
            try await publishCapability(for: jobKind)
        }
    }

    // MARK: - Publishing

    private func publishCapability(for jobKind: JobKind) async throws {
        // Get capability config for this job kind
        guard let capability = config.capabilitiesPerKind[jobKind] else {
            logger.warning("No capability config for job kind: \(jobKind.name)")
            return
        }

        // Build NIP-89 event
        let event = try buildCapabilityEvent(jobKind: jobKind, capability: capability)

        // Publish to relays
        let results = await nostrClient.broadcast(event)

        // Track published event ID
        advertisedEventIds[jobKind] = event.id

        // Log results
        let successCount = results.values.filter { try? $0.get() != nil }.count
        logger.info("Published capability for \(jobKind.name) to \(successCount)/\(results.count) relays")
    }

    private func buildCapabilityEvent(
        jobKind: JobKind,
        capability: JobCapability
    ) throws -> NostrEvent {
        // NIP-89: kind:31990 (Replaceable event with 'd' tag)
        let dTag = "\(jobKind.rawValue)"  // Unique identifier for this capability

        var tags: [[String]] = [
            ["d", dTag],                         // Replaceable event identifier
            ["k", "\(jobKind.rawValue)"],        // Kind this handler supports
            ["relays"] + config.advertisingRelays
        ]

        // Encode capability as JSON content
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let capabilityData = try encoder.encode(capability)
        let content = String(data: capabilityData, encoding: .utf8)!

        // Sign and return event
        return try NostrEvent.sign(
            privateKey: workerPrivateKey,
            created_at: Int64(Date().timeIntervalSince1970),
            kind: 31990,  // NIP-89 handler information
            tags: tags,
            content: content
        )
    }

    private func deleteCapability(eventId: String) async {
        // NIP-09: kind:5 deletion event
        let deleteEvent = try! NostrEvent.sign(
            privateKey: workerPrivateKey,
            created_at: Int64(Date().timeIntervalSince1970),
            kind: 5,
            tags: [["e", eventId]],
            content: "Capability no longer available"
        )

        await nostrClient.broadcast(deleteEvent)
        logger.info("Published deletion event for capability: \(eventId)")
    }

    // MARK: - Preview

    /// Preview capability events (for UI)
    public func previewCapabilities() throws -> [NostrEvent] {
        var events: [NostrEvent] = []

        for jobKind in config.supportedJobKinds {
            if let capability = config.capabilitiesPerKind[jobKind] {
                let event = try buildCapabilityEvent(jobKind: jobKind, capability: capability)
                events.append(event)
            }
        }

        return events
    }
}
```

```swift
// CapabilityConfig.swift

import Foundation

/// Capability advertising configuration
public struct CapabilityConfig: Codable {
    public var enabled: Bool = true
    public var supportedJobKinds: [JobKind] = [
        .textSummarization,
        .qaRag,
        .codeReview,
        .dataExtraction,
        .sentimentAnalysis
    ]
    public var advertisingRelays: [String] = [
        "wss://relay.damus.io",
        "wss://nos.lol"
    ]
    public var capabilitiesPerKind: [JobKind: JobCapability] = [:]

    public init() {
        // Initialize default capabilities
        for kind in supportedJobKinds {
            capabilitiesPerKind[kind] = JobCapability.defaultFor(kind)
        }
    }
}

/// Job capability (NIP-89 extension)
public struct JobCapability: Codable {
    public var jobKind: JobKind
    public var version: String = "0.1.0"
    public var pricing: PricingModel
    public var limits: Limits?
    public var features: [String: String]?

    public struct PricingModel: Codable {
        public var basePrice: Int64        // msats
        public var perUnitPrice: Int64?    // msats per unit (e.g., per token)
        public var unit: String?           // "token", "word", "character"

        public static let free = PricingModel(basePrice: 0, perUnitPrice: nil, unit: nil)

        public static func perJob(_ msats: Int64) -> PricingModel {
            PricingModel(basePrice: msats, perUnitPrice: nil, unit: nil)
        }

        public static func perToken(_ msats: Int64) -> PricingModel {
            PricingModel(basePrice: 0, perUnitPrice: msats, unit: "token")
        }
    }

    public struct Limits: Codable {
        public var maxInputSize: Int?      // bytes
        public var maxOutputSize: Int?     // bytes
        public var maxTokens: Int?
        public var timeout: Int?           // seconds

        public static let `default` = Limits(
            maxInputSize: 100_000,       // 100KB
            maxOutputSize: 50_000,       // 50KB
            maxTokens: 2000,
            timeout: 300                 // 5 minutes
        )
    }

    /// Default capability for job kind
    public static func defaultFor(_ kind: JobKind) -> JobCapability {
        JobCapability(
            jobKind: kind,
            version: "0.1.0",
            pricing: .free,  // MVP: Free compute
            limits: .default,
            features: [
                "model": "foundation-models",
                "platform": "macOS"
            ]
        )
    }
}
```

### macOS UI

```swift
// ios/OpenAgents/Views/Worker/ (macOS target)

WorkerSettingsView.swift     // Worker configuration UI
CapabilityEditorView.swift   // Edit capabilities per job kind
```

```swift
// WorkerSettingsView.swift

import SwiftUI

struct WorkerSettingsView: View {
    @StateObject private var advertiser: CapabilityAdvertiser
    @State private var showingCapabilityEditor = false

    var body: some View {
        Form {
            Section("Capability Advertising") {
                Toggle("Advertise Capabilities", isOn: $advertiser.config.enabled)

                if advertiser.config.enabled {
                    LabeledContent("Job Kinds", value: "\(advertiser.config.supportedJobKinds.count)")

                    Button("Edit Capabilities") {
                        showingCapabilityEditor = true
                    }

                    Button("Preview Ads") {
                        previewCapabilities()
                    }

                    Button("Publish Now") {
                        Task {
                            try? await advertiser.updateCapabilities()
                        }
                    }
                }
            }

            Section("Advertising Relays") {
                ForEach(advertiser.config.advertisingRelays, id: \.self) { relay in
                    Text(relay)
                }

                Button("Add Relay") {
                    // Show add relay sheet
                }
            }
        }
        .sheet(isPresented: $showingCapabilityEditor) {
            CapabilityEditorView(config: $advertiser.config)
        }
    }

    private func previewCapabilities() {
        guard let events = try? advertiser.previewCapabilities() else { return }

        for event in events {
            print("=== Capability Ad ===")
            print("Kind: \(event.kind)")
            print("Tags: \(event.tags)")
            print("Content:\n\(event.content)")
            print()
        }
    }
}
```

```swift
// CapabilityEditorView.swift

struct CapabilityEditorView: View {
    @Binding var config: CapabilityConfig
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            List {
                ForEach(config.supportedJobKinds) { jobKind in
                    NavigationLink(destination: CapabilityDetailEditor(
                        jobKind: jobKind,
                        capability: binding(for: jobKind)
                    )) {
                        VStack(alignment: .leading) {
                            Text(jobKind.displayName)
                                .font(.headline)

                            if let cap = config.capabilitiesPerKind[jobKind] {
                                Text("Base: \(cap.pricing.basePrice) msats")
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                            }
                        }
                    }
                }
            }
            .navigationTitle("Edit Capabilities")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") {
                        dismiss()
                    }
                }
            }
        }
    }

    private func binding(for jobKind: JobKind) -> Binding<JobCapability> {
        Binding(
            get: {
                config.capabilitiesPerKind[jobKind] ?? JobCapability.defaultFor(jobKind)
            },
            set: { newValue in
                config.capabilitiesPerKind[jobKind] = newValue
            }
        )
    }
}

struct CapabilityDetailEditor: View {
    let jobKind: JobKind
    @Binding var capability: JobCapability

    var body: some View {
        Form {
            Section("Pricing") {
                TextField("Base Price (msats)", value: $capability.pricing.basePrice, format: .number)

                Toggle("Per-Unit Pricing", isOn: Binding(
                    get: { capability.pricing.perUnitPrice != nil },
                    set: { enabled in
                        capability.pricing.perUnitPrice = enabled ? 1 : nil
                        capability.pricing.unit = enabled ? "token" : nil
                    }
                ))

                if capability.pricing.perUnitPrice != nil {
                    TextField("Per-Unit Price (msats)", value: Binding(
                        get: { capability.pricing.perUnitPrice ?? 0 },
                        set: { capability.pricing.perUnitPrice = $0 }
                    ), format: .number)

                    Picker("Unit", selection: Binding(
                        get: { capability.pricing.unit ?? "token" },
                        set: { capability.pricing.unit = $0 }
                    )) {
                        Text("Token").tag("token")
                        Text("Word").tag("word")
                        Text("Character").tag("character")
                    }
                }
            }

            Section("Limits") {
                if capability.limits == nil {
                    Button("Add Limits") {
                        capability.limits = .default
                    }
                } else {
                    TextField("Max Input Size (bytes)", value: Binding(
                        get: { capability.limits?.maxInputSize ?? 0 },
                        set: { capability.limits?.maxInputSize = $0 }
                    ), format: .number)

                    TextField("Max Tokens", value: Binding(
                        get: { capability.limits?.maxTokens ?? 0 },
                        set: { capability.limits?.maxTokens = $0 }
                    ), format: .number)

                    TextField("Timeout (seconds)", value: Binding(
                        get: { capability.limits?.timeout ?? 0 },
                        set: { capability.limits?.timeout = $0 }
                    ), format: .number)

                    Button("Remove Limits", role: .destructive) {
                        capability.limits = nil
                    }
                }
            }

            Section("Features") {
                LabeledContent("Model", value: capability.features?["model"] ?? "foundation-models")
                LabeledContent("Platform", value: capability.features?["platform"] ?? "macOS")
            }
        }
        .navigationTitle(jobKind.displayName)
    }
}
```

## Dependencies

### OpenAgents Dependencies
- **Issue #001**: Nostr Client Library (event publishing, broadcast)
- **Issue #004**: Job Schema Registry (JobKind, job definitions)
- **Issue #007**: macOS Foundation Models Worker (worker config)

### System Frameworks
- **Foundation**: Core types
- **SwiftUI**: macOS UI

## Testing Requirements

### Unit Tests
- [ ] Build NIP-89 capability event
- [ ] Encode capability as JSON
- [ ] Multiple job kinds → multiple events
- [ ] Deletion event creation (NIP-09)
- [ ] Preview capabilities (without publishing)

### Integration Tests
- [ ] Publish capability to real relay
- [ ] Verify capability event on relay
- [ ] Update capability (replaceable event)
- [ ] Delete capability (NIP-09 deletion)

### UI Tests
- [ ] Edit pricing in UI
- [ ] Edit limits in UI
- [ ] Preview ads before publishing
- [ ] Publish button triggers broadcast

## Apple Compliance Considerations

### App Store Review Guidelines

**No specific compliance issues** for capability advertising:
- ✅ Publishing Nostr events (data, not code)
- ✅ No payments involved (just advertising pricing)

**ASRG 5.1.2 (Data Use)**
- ✅ Capability ads are public by design (Nostr protocol)
- ✅ User explicitly enables advertising (opt-in)

## Reference Links

### Specifications
- **NIP-89** (Recommended Application Handlers): https://github.com/nostr-protocol/nips/blob/master/89.md
- **NIP-09** (Event Deletion): https://github.com/nostr-protocol/nips/blob/master/09.md

### OpenAgents
- **Issue #001**: Nostr Client Library
- **Issue #004**: Job Schema Registry
- **Issue #007**: macOS Foundation Models Worker

## Success Metrics

- [ ] Publish capabilities for 5+ job kinds
- [ ] Capability ads visible on public relays
- [ ] Update capability when config changes
- [ ] Delete capability on worker stop
- [ ] UI for editing capabilities works

## Notes

- **Replaceable Events**: NIP-89 uses kind:31990 (replaceable) - updates overwrite previous ads
- **Deletion**: NIP-09 deletion events remove capabilities when worker stops
- **Refresh**: Republish every 24h to keep ads fresh
- **Pricing**: MVP can use free pricing; real pricing in Phase 2

## Future Enhancements (Post-MVP)

- Dynamic pricing based on demand/supply
- Capability negotiation (buyer requests specific features)
- Reputation scores in capability ads (from issue #022)
- Multi-language capabilities (i18n job kind descriptions)
- Geographic hints (latency optimization)
