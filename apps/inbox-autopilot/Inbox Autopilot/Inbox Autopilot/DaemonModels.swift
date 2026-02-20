import Foundation

struct SessionCreateRequest: Codable {
    let clientName: String?

    enum CodingKeys: String, CodingKey {
        case clientName = "client_name"
    }
}

struct SessionCreateResponse: Codable {
    let sessionToken: String
    let expiresAt: Date

    enum CodingKeys: String, CodingKey {
        case sessionToken = "session_token"
        case expiresAt = "expires_at"
    }
}

struct HealthResponse: Codable {
    let status: String
    let connectedGmail: Bool
    let connectedChatgpt: Bool

    enum CodingKeys: String, CodingKey {
        case status
        case connectedGmail = "connected_gmail"
        case connectedChatgpt = "connected_chatgpt"
    }
}

struct AuthStatusResponse: Codable {
    let connected: Bool
    let updatedAt: Date?

    enum CodingKeys: String, CodingKey {
        case connected
        case updatedAt = "updated_at"
    }
}

struct GmailAuthURLResponse: Codable {
    let url: String
    let state: String
}

struct GmailAuthRequest: Codable {
    let code: String
    let redirectURI: String
    let codeVerifier: String?

    enum CodingKeys: String, CodingKey {
        case code
        case redirectURI = "redirect_uri"
        case codeVerifier = "code_verifier"
    }
}

struct ChatGPTAuthRequest: Codable {
    let apiKey: String

    enum CodingKeys: String, CodingKey {
        case apiKey = "api_key"
    }
}

struct BackfillRequest: Codable {
    let days: Int?
}

struct BackfillResponse: Codable {
    let importedThreads: Int
    let importedMessages: Int

    enum CodingKeys: String, CodingKey {
        case importedThreads = "imported_threads"
        case importedMessages = "imported_messages"
    }
}

struct ThreadListResponse: Codable {
    let threads: [ThreadSummary]
}

enum ThreadCategory: String, Codable, CaseIterable {
    case scheduling
    case reportDelivery = "report_delivery"
    case findingsClarification = "findings_clarification"
    case pricing
    case complaintDispute = "complaint_dispute"
    case legalInsurance = "legal_insurance"
    case other

    var title: String {
        switch self {
        case .scheduling:
            return "Scheduling"
        case .reportDelivery:
            return "Report"
        case .findingsClarification:
            return "Clarification"
        case .pricing:
            return "Pricing"
        case .complaintDispute:
            return "Complaint"
        case .legalInsurance:
            return "Legal"
        case .other:
            return "Other"
        }
    }
}

enum RiskTier: String, Codable {
    case low
    case medium
    case high

    var title: String {
        rawValue.capitalized
    }
}

enum PolicyDecision: String, Codable {
    case draftOnly = "draft_only"
    case sendWithApproval = "send_with_approval"
    case blocked

    var title: String {
        switch self {
        case .draftOnly:
            return "Draft only"
        case .sendWithApproval:
            return "Send with approval"
        case .blocked:
            return "Blocked"
        }
    }
}

enum DraftStatus: String, Codable {
    case pending
    case approved
    case rejected
    case needsHuman = "needs_human"
    case sent

    var title: String {
        switch self {
        case .pending:
            return "Pending"
        case .approved:
            return "Approved"
        case .rejected:
            return "Rejected"
        case .needsHuman:
            return "Needs human"
        case .sent:
            return "Sent"
        }
    }
}

struct ThreadSummary: Codable, Identifiable {
    let id: String
    let subject: String
    let snippet: String
    let fromAddress: String
    let category: ThreadCategory?
    let risk: RiskTier?
    let policy: PolicyDecision?
    let lastMessageAt: Date
    let hasPendingDraft: Bool

    enum CodingKeys: String, CodingKey {
        case id
        case subject
        case snippet
        case fromAddress = "from_address"
        case category
        case risk
        case policy
        case lastMessageAt = "last_message_at"
        case hasPendingDraft = "has_pending_draft"
    }
}

struct MessageRecord: Codable, Identifiable {
    let id: String
    let threadID: String
    let sender: String
    let recipient: String
    let body: String
    let snippet: String
    let inbound: Bool
    let sentAt: Date

    enum CodingKeys: String, CodingKey {
        case id
        case threadID = "thread_id"
        case sender
        case recipient
        case body
        case snippet
        case inbound
        case sentAt = "sent_at"
    }
}

struct DraftRecord: Codable, Identifiable {
    let id: String
    let threadID: String
    let body: String
    let status: DraftStatus
    let sourceSummary: String
    let modelUsed: String?
    let createdAt: Date
    let updatedAt: Date

    enum CodingKeys: String, CodingKey {
        case id
        case threadID = "thread_id"
        case body
        case status
        case sourceSummary = "source_summary"
        case modelUsed = "model_used"
        case createdAt = "created_at"
        case updatedAt = "updated_at"
    }
}

struct ThreadDetailResponse: Codable {
    let thread: ThreadSummary
    let messages: [MessageRecord]
    let draft: DraftRecord?
}

struct DraftListResponse: Codable {
    let drafts: [DraftRecord]
}

struct GenerateDraftResponse: Codable {
    let draft: DraftRecord
    let category: ThreadCategory
    let risk: RiskTier
    let policy: PolicyDecision
}

struct ApproveSendResponse: Codable {
    let draftID: String
    let gmailMessageID: String

    enum CodingKeys: String, CodingKey {
        case draftID = "draft_id"
        case gmailMessageID = "gmail_message_id"
    }
}

struct EventRecord: Codable, Identifiable {
    let id: String
    let threadID: String?
    let eventType: String
    let payload: [String: StringValue]
    let createdAt: Date

    enum CodingKeys: String, CodingKey {
        case id
        case threadID = "thread_id"
        case eventType = "event_type"
        case payload
        case createdAt = "created_at"
    }
}

struct EventListResponse: Codable {
    let events: [EventRecord]
}

struct AuditResponse: Codable {
    let category: ThreadCategory?
    let risk: RiskTier?
    let policy: PolicyDecision?
    let similarThreadIDs: [String]
    let externalModelUsed: Bool
    let events: [EventRecord]

    enum CodingKeys: String, CodingKey {
        case category
        case risk
        case policy
        case similarThreadIDs = "similar_thread_ids"
        case externalModelUsed = "external_model_used"
        case events
    }
}

enum PrivacyMode: String, Codable, CaseIterable {
    case localOnly = "local_only"
    case hybrid
    case cloud

    var title: String {
        switch self {
        case .localOnly:
            return "Local-only"
        case .hybrid:
            return "Hybrid"
        case .cloud:
            return "Cloud"
        }
    }
}

enum AttachmentStorageMode: String, Codable, CaseIterable {
    case none
    case metadata
    case full

    var title: String {
        switch self {
        case .none:
            return "None"
        case .metadata:
            return "Metadata only"
        case .full:
            return "Full"
        }
    }
}

struct SettingsResponse: Codable {
    var privacyMode: PrivacyMode
    var backfillDays: Int
    var allowedRecipientDomains: [String]
    var attachmentStorageMode: AttachmentStorageMode
    var signature: String?
    var templateScheduling: String?
    var templateReportDelivery: String?
    var syncIntervalSeconds: Int

    enum CodingKeys: String, CodingKey {
        case privacyMode = "privacy_mode"
        case backfillDays = "backfill_days"
        case allowedRecipientDomains = "allowed_recipient_domains"
        case attachmentStorageMode = "attachment_storage_mode"
        case signature
        case templateScheduling = "template_scheduling"
        case templateReportDelivery = "template_report_delivery"
        case syncIntervalSeconds = "sync_interval_seconds"
    }
}

struct UpdateSettingsRequest: Codable {
    let privacyMode: PrivacyMode
    let backfillDays: Int
    let allowedRecipientDomains: [String]
    let attachmentStorageMode: AttachmentStorageMode
    let signature: String?
    let templateScheduling: String?
    let templateReportDelivery: String?
    let syncIntervalSeconds: Int?

    enum CodingKeys: String, CodingKey {
        case privacyMode = "privacy_mode"
        case backfillDays = "backfill_days"
        case allowedRecipientDomains = "allowed_recipient_domains"
        case attachmentStorageMode = "attachment_storage_mode"
        case signature
        case templateScheduling = "template_scheduling"
        case templateReportDelivery = "template_report_delivery"
        case syncIntervalSeconds = "sync_interval_seconds"
    }
}

struct ExportAuditResponse: Codable {
    let path: String
    let exportedEvents: Int

    enum CodingKeys: String, CodingKey {
        case path
        case exportedEvents = "exported_events"
    }
}

struct TemplateSuggestion: Codable, Identifiable {
    let id: String
    let category: ThreadCategory
    let templateText: String
    let occurrences: Int

    enum CodingKeys: String, CodingKey {
        case id
        case category
        case templateText = "template_text"
        case occurrences
    }
}

struct TemplateMineResponse: Codable {
    let suggestions: [TemplateSuggestion]
}

struct DraftQualitySampleResult: Codable, Identifiable {
    let threadID: String
    let category: ThreadCategory
    let editRatio: Double
    let minimalEdit: Bool
    let draftWordCount: Int
    let sentWordCount: Int

    var id: String { threadID }

    enum CodingKeys: String, CodingKey {
        case threadID = "thread_id"
        case category
        case editRatio = "edit_ratio"
        case minimalEdit = "minimal_edit"
        case draftWordCount = "draft_word_count"
        case sentWordCount = "sent_word_count"
    }
}

struct DraftQualityCategorySummary: Codable, Identifiable {
    let category: ThreadCategory
    let samples: Int
    let minimalEditCount: Int
    let minimalEditRate: Double
    let averageEditRatio: Double

    var id: String { category.rawValue }

    enum CodingKeys: String, CodingKey {
        case category
        case samples
        case minimalEditCount = "minimal_edit_count"
        case minimalEditRate = "minimal_edit_rate"
        case averageEditRatio = "average_edit_ratio"
    }
}

struct DraftQualityReport: Codable {
    let generatedAt: Date
    let threshold: Double
    let targetRate: Double
    let totalSamples: Int
    let totalMinimalEdit: Int
    let totalMinimalEditRate: Double
    let targetMet: Bool
    let categories: [DraftQualityCategorySummary]
    let samples: [DraftQualitySampleResult]

    enum CodingKeys: String, CodingKey {
        case generatedAt = "generated_at"
        case threshold
        case targetRate = "target_rate"
        case totalSamples = "total_samples"
        case totalMinimalEdit = "total_minimal_edit"
        case totalMinimalEditRate = "total_minimal_edit_rate"
        case targetMet = "target_met"
        case categories
        case samples
    }
}

struct StringValue: Codable {
    let value: String

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let string = try? container.decode(String.self) {
            value = string
        } else if let int = try? container.decode(Int.self) {
            value = String(int)
        } else if let bool = try? container.decode(Bool.self) {
            value = String(bool)
        } else if let double = try? container.decode(Double.self) {
            value = String(double)
        } else if let array = try? container.decode([String].self) {
            value = array.joined(separator: ",")
        } else {
            value = ""
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(value)
    }
}
