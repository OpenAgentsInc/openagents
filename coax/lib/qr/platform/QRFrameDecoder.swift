import CoreVideo
import Vision

/// Platform byte observation only. This does not open a camera, authenticate
/// a URL, normalize payloads, retain frames or approve any scanned request.
enum CoaxQRFrameDecoder {
    static func decode(_ frame: CVPixelBuffer) throws -> [String] {
        let request = VNDetectBarcodesRequest()
        // QR-only baseline decoder is available on Simulator as well as the
        // phone; newer learned detectors can require an unavailable context.
        request.revision = VNDetectBarcodesRequestRevision1
        request.symbologies = [.qr]
        try VNImageRequestHandler(cvPixelBuffer: frame, orientation: .up).perform([request])
        return request.results?.compactMap(\.payloadStringValue) ?? []
    }
}
