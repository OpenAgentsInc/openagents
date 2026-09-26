// Camera and image decoding return bounded text. Only Rust can validate an
// invitation or admit a connection; QR contents are never opened as URLs.
import Foundation
import Vision
import CoreGraphics

enum QRInvitation {
    static let maximumBytes = 640

    static func bounded(_ text: String) throws -> String {
        guard text.hasPrefix("coder-pair:"), text.utf8.count <= maximumBytes else {
            throw Failure.message("This QR code is not a Coder invitation. Scan the code displayed by your computer.")
        }
        return text
    }

    static func decode(image: CGImage) throws -> String {
        guard image.width <= 4096, image.height <= 4096 else {
            throw Failure.message("The QR image is too large.")
        }
        let request = VNDetectBarcodesRequest()
        request.symbologies = [.qr]
        try VNImageRequestHandler(cgImage: image, options: [:]).perform([request])
        let codes = request.results?.compactMap(\.payloadStringValue) ?? []
        guard codes.count == 1, let code = codes.first else {
            throw Failure.message("Show one Coder invitation QR code at a time.")
        }
        return try bounded(code)
    }

    enum Failure: LocalizedError {
        case message(String)
        var errorDescription: String? {
            switch self { case let .message(message): return message }
        }
    }
}
