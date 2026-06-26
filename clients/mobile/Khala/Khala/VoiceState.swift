import SwiftUI

/// The push-to-talk lifecycle, descended from Onyx's `VoiceSession` state
/// machine: idle -> recording -> transcribing -> success/error -> idle.
/// Each state maps to a color, reusing Onyx's voice status semantics.
enum VoiceState: Equatable {
    case idle
    case recording
    case transcribing
    case thinking // awaiting Khala API response
    case success
    case error(String)

    /// Onyx voice status colors:
    /// recording = red hsl(0,80%,50%), transcribing = orange hsl(45,90%,50%),
    /// success = green hsl(120,60%,50%). Idle/thinking are dim neutrals.
    var accentColor: Color {
        switch self {
        case .idle:
            return Color(hue: 0.0, saturation: 0.0, brightness: 0.55)
        case .recording:
            return Color(hue: 0.0 / 360.0, saturation: 0.80, brightness: 0.55)
        case .transcribing:
            return Color(hue: 45.0 / 360.0, saturation: 0.90, brightness: 0.55)
        case .thinking:
            return Color(hue: 210.0 / 360.0, saturation: 0.55, brightness: 0.60)
        case .success:
            return Color(hue: 120.0 / 360.0, saturation: 0.60, brightness: 0.55)
        case .error:
            return Color(hue: 0.0 / 360.0, saturation: 0.85, brightness: 0.50)
        }
    }

    var label: String {
        switch self {
        case .idle: return "Hold to talk"
        case .recording: return "Listening…"
        case .transcribing: return "Transcribing…"
        case .thinking: return "Khala is thinking…"
        case .success: return "Done"
        case .error(let message): return message
        }
    }

    var isBusy: Bool {
        switch self {
        case .idle, .success, .error: return false
        case .recording, .transcribing, .thinking: return true
        }
    }
}
