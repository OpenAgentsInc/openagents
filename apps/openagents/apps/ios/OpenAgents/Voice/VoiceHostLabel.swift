/// The call's host identity and its last available display name.
struct VoiceHostLabel {
    let deviceId: String
    private(set) var name: String

    mutating func resolve(using nameForDevice: (String) -> String?) -> String {
        if let current = nameForDevice(deviceId), !current.isEmpty {
            name = current
        }
        return name
    }
}
