const { AndroidConfig, withAndroidManifest, withInfoPlist } = require("@expo/config-plugins")

module.exports = function withKhalaPushToTalkStt(config) {
  config = withInfoPlist(config, mod => {
    mod.modResults.NSMicrophoneUsageDescription =
      mod.modResults.NSMicrophoneUsageDescription ||
      "Khala uses the microphone for push-to-talk transcription."
    mod.modResults.NSSpeechRecognitionUsageDescription =
      mod.modResults.NSSpeechRecognitionUsageDescription ||
      "Khala uses on-device speech recognition to transcribe push-to-talk input."
    return mod
  })

  return withAndroidManifest(config, mod => {
    AndroidConfig.Permissions.ensurePermissions(mod.modResults, ["android.permission.RECORD_AUDIO"])
    return mod
  })
}
