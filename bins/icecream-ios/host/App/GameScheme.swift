import Foundation
import WebKit

/// Serves the bundled game at icecream://game/... from the app's
/// `game` resource folder. WKWebView will not load ES modules or fetch
/// wasm from file:// reliably, so the page gets a real origin with the
/// right MIME types instead.
final class GameScheme: NSObject, WKURLSchemeHandler {
    static let scheme = "icecream"
    static let start = URL(string: "icecream://game/index.html")!

    private let root: URL

    override init() {
        root = Bundle.main.resourceURL!.appendingPathComponent("game", isDirectory: true).standardizedFileURL
        super.init()
    }

    func webView(_ webView: WKWebView, start task: WKURLSchemeTask) {
        guard let url = task.request.url, let file = resolve(url),
              var data = try? Data(contentsOf: file) else {
            let response = HTTPURLResponse(url: task.request.url ?? GameScheme.start, statusCode: 404,
                                           httpVersion: "HTTP/1.1", headerFields: ["Content-Length": "0"])!
            task.didReceive(response)
            task.didReceive(Data())
            task.didFinish()
            return
        }
        if file.lastPathComponent == "index.html" { data = GameScheme.tagVersion(data) }
        let headers = [
            "Content-Type": GameScheme.mime(file.pathExtension),
            "Content-Length": String(data.count),
            "Cache-Control": "no-cache",
        ]
        let response = HTTPURLResponse(url: url, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: headers)!
        task.didReceive(response)
        task.didReceive(data)
        task.didFinish()
    }

    func webView(_ webView: WKWebView, stop task: WKURLSchemeTask) {}

    /// The file for a URL: its path under the game folder, or, when the
    /// page asks for an asset under a site path (/games/the-ice-cream-man/
    /// or a versioned folder), the file of that name at the top.
    private func resolve(_ url: URL) -> URL? {
        var path = url.path
        if path.isEmpty || path == "/" { path = "/index.html" }
        let direct = root.appendingPathComponent(String(path.dropFirst())).standardizedFileURL
        if direct.path.hasPrefix(root.path + "/"), exists(direct) { return direct }
        let flat = root.appendingPathComponent(url.lastPathComponent).standardizedFileURL
        if flat.path.hasPrefix(root.path + "/"), exists(flat) { return flat }
        return nil
    }

    private func exists(_ url: URL) -> Bool {
        var directory: ObjCBool = false
        return FileManager.default.fileExists(atPath: url.path, isDirectory: &directory) && !directory.boolValue
    }

    /// Puts the app's version on the game canvas (data-version), which the
    /// game shows small in its pause menu, unless the page set one.
    static func tagVersion(_ data: Data) -> Data {
        guard var page = String(data: data, encoding: .utf8),
              let range = page.range(of: "id=\"icecream-canvas\""),
              !page.contains("data-version=") else { return data }
        let info = Bundle.main.infoDictionary ?? [:]
        let version = info["CFBundleShortVersionString"] as? String ?? "1.0"
        let build = info["CFBundleVersion"] as? String ?? "1"
        page.replaceSubrange(range, with: "id=\"icecream-canvas\" data-version=\"ios \(version) (\(build))\"")
        return Data(page.utf8)
    }

    static func mime(_ ext: String) -> String {
        switch ext.lowercased() {
        case "html", "htm": return "text/html; charset=utf-8"
        case "js", "mjs": return "text/javascript; charset=utf-8"
        case "wasm": return "application/wasm"
        case "css": return "text/css; charset=utf-8"
        case "json", "webmanifest": return "application/json"
        case "png": return "image/png"
        case "jpg", "jpeg": return "image/jpeg"
        case "svg": return "image/svg+xml"
        case "webp": return "image/webp"
        case "ico": return "image/x-icon"
        case "mp3": return "audio/mpeg"
        case "ogg": return "audio/ogg"
        case "wav": return "audio/wav"
        case "woff2": return "font/woff2"
        case "woff": return "font/woff"
        case "txt": return "text/plain; charset=utf-8"
        default: return "application/octet-stream"
        }
    }
}
