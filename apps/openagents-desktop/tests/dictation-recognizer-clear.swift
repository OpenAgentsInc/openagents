// Firm reason: 2 (independent success/failure ordering oracle; injected C ABI).
import Foundation
@main struct RecognizerClearOracle {
  static func main() throws {
    let recognizer = ResidentParakeetDictation(directory: "unused-fixture-model")
    try recognizer.open(); let audio: [Float] = [0.25]
    let first = try recognizer.recognize(audio[...])
    precondition(String(decoding: first, as: UTF8.self) == "kept text")
    precondition(clear_oracle_calls() == 1 && clear_oracle_dirty() == 0)
    clear_oracle_fail(1)
    do { _ = try recognizer.recognize(audio[...]); preconditionFailure("injected refusal accepted") }
    catch DictationEffectError.recognition {}
    precondition(clear_oracle_calls() == 2 && clear_oracle_dirty() == 0)
    precondition(String(decoding: first, as: UTF8.self) == "kept text")
    recognizer.close(); precondition(clear_oracle_closed() == 1)
    print("Dictation recognizer clear: copied success text survives; success and failure both clear resident audio")
  }
}
