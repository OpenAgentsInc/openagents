// Firm reason: 6 (native actions to clipboard, persistence and the voice SDK bridge).
import AppKit
import WebKit

@MainActor
extension ForgeApp {
    func metalAction(_ code: Int32, _ text: String) {
        switch code {
        case 7, 8, 9:
            let method = code == 7 ? "toggle":code == 8 ? "face":"say"
            Task {
                _ = try? await web.callAsyncJavaScript("return coaxial.voice[method](method==='toggle' ? (text==='end' ? 'end':text==='hold' ? 'hold':text==='on') : text);", arguments: ["method":method,"text":text], in: nil, contentWorld: .page)
            }
        case 5:
            if let data = text.data(using: .utf8), let p = try? JSONSerialization.jsonObject(with: data) as? [String: Any], let key = p["key"] as? String {
                metalWorld?.network?.playerKey = key
                try? data.write(to: dataURL.appendingPathComponent("metal-player.json"), options: .atomic)
            }
        case 13: try? Data(text.utf8).write(to: dataURL.appendingPathComponent("metal-sound.txt"), options: .atomic)
        case 1: if let url = URL(string: text), url.scheme == "https" || url.scheme == "lightning" { NSWorkspace.shared.open(url) }
        default: break
        }
    }
}
