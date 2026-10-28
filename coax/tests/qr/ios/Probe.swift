import UIKit
import CoreVideo

@main
@MainActor
final class QRProbe: UIResponder, UIApplicationDelegate {
    var window: UIWindow?
    func application(_ application: UIApplication, didFinishLaunchingWithOptions options: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {
        window = UIWindow(frame: UIScreen.main.bounds)
        window?.rootViewController = UIViewController()
        window?.makeKeyAndVisible()
        DispatchQueue.main.async { self.scan() }
        return true
    }
    private func scan() {
        var rows: [[String: Any]] = []
        var failure: String?
        do {
            let fixtures = try JSONDecoder().decode([Fixture].self, from: Data(contentsOf: Bundle.main.url(forResource: "fixtures", withExtension: "json")!))
            for fixture in fixtures {
                guard let file = Bundle.main.url(forResource: fixture.file, withExtension: nil), let image = UIImage(contentsOfFile: file.path)?.cgImage else { throw Refusal.image }
                let frame = try raster(image)
                let observed = try CoaxQRFrameDecoder.decode(frame)
                rows.append(["name": fixture.file, "inputBytes": fixture.text.utf8.count, "observed": observed, "matches": observed == [fixture.text], "width": image.width, "height": image.height])
            }
        } catch { failure = String(describing: error) }
        var report: [String: Any] = ["platform": "iOS Simulator", "systemVersion": UIDevice.current.systemVersion, "decoder": "Vision.VNDetectBarcodesRequest QR over CVPixelBuffer", "rows": rows, "physicalCamera": false]
        if let failure { report["failure"] = failure }
        do {
            let url = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0].appendingPathComponent("qr-scanner.json")
            try JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys]).write(to: url, options: .atomic)
        } catch { NSLog("Public QR probe could not retain result") }
    }
    private func raster(_ image: CGImage) throws -> CVPixelBuffer {
        var buffer: CVPixelBuffer?
        let attributes: [String: Any] = [kCVPixelBufferCGImageCompatibilityKey as String: true, kCVPixelBufferCGBitmapContextCompatibilityKey as String: true]
        guard CVPixelBufferCreate(kCFAllocatorDefault, image.width, image.height, kCVPixelFormatType_32BGRA, attributes as CFDictionary, &buffer) == kCVReturnSuccess, let buffer else { throw Refusal.buffer }
        CVPixelBufferLockBaseAddress(buffer, [])
        defer { CVPixelBufferUnlockBaseAddress(buffer, []) }
        guard let context = CGContext(data: CVPixelBufferGetBaseAddress(buffer), width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: CVPixelBufferGetBytesPerRow(buffer), space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue) else { throw Refusal.context }
        context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        return buffer
    }
    private struct Fixture: Decodable { let file: String; let text: String }
    private enum Refusal: Error { case image, buffer, context }
}
