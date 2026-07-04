import AVFoundation
import ExpoModulesCore
import Speech

public final class KhalaPushToTalkSttModule: Module {
  public func definition() -> ModuleDefinition {
    Name("KhalaPushToTalkStt")

    AsyncFunction("getAvailabilityAsync") { () -> [String: String] in
      let speechStatus = SFSpeechRecognizer.authorizationStatus()
      let microphoneStatus = AVAudioSession.sharedInstance().recordPermission

      if speechStatus == .denied || speechStatus == .restricted || microphoneStatus == .denied {
        return [
          "status": "denied",
          "reason": "speech_or_microphone_permission_denied"
        ]
      }

      let recognizer = SFSpeechRecognizer()
      guard recognizer?.isAvailable == true else {
        return [
          "status": "unavailable",
          "reason": "speech_recognizer_unavailable"
        ]
      }

      return ["status": "available"]
    }

    AsyncFunction("startRecognitionAsync") { (locale: String?) -> [String: Any] in
      throw SpeechRuntimeUnavailableException()
    }

    AsyncFunction("stopRecognitionAsync") { () -> [String: Any] in
      [
        "transcript": "",
        "isFinal": true,
        "locale": Locale.current.identifier
      ]
    }
  }
}

final class SpeechRuntimeUnavailableException: Exception {
  override var reason: String {
    "The TS-8 module shell is linked, but streaming SFSpeechRecognizer capture still needs the owner device proof pass."
  }
}
