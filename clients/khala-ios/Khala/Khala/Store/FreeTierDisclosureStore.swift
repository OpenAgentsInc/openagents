import Foundation

/// Stores the non-secret first-use acknowledgement for free Khala keys.
enum FreeTierDisclosureStore {
    private static let key = "com.openagents.khala.freeTierDisclosureAccepted"

    static var hasAccepted: Bool {
        UserDefaults.standard.bool(forKey: key)
    }

    static func accept() {
        UserDefaults.standard.set(true, forKey: key)
    }

    static func requiresDisclosure(for apiKey: String) -> Bool {
        apiKey.trimmingCharacters(in: .whitespacesAndNewlines).hasPrefix("oa_agent_")
    }

    static func canUse(apiKey: String) -> Bool {
        hasAccepted || !requiresDisclosure(for: apiKey)
    }
}
