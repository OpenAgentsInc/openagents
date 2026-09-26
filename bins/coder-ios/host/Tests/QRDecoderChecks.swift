// Deterministic QR image decoding, not a physical-camera acceptance test.
import CoreGraphics
import CoreImage
import Foundation

@main
struct QRDecoderChecks {
    static func main() throws {
        let invitation = "coder-pair:" + String(repeating: "a", count: 240)
        let image = try qr(invitation)
        guard try QRInvitation.decode(image: image) == invitation else { throw Failure.fixture }
        for value in ["https://example.invalid/not-an-invitation", "coder-pair:" + String(repeating: "a", count: 640)] {
            do {
                _ = try QRInvitation.bounded(value)
                throw Failure.fixture
            } catch is QRInvitation.Failure {}
        }
        do {
            _ = try QRInvitation.decode(image: qr("https://example.invalid/"))
            throw Failure.fixture
        } catch is QRInvitation.Failure {}
        print("QR image checks passed: exact invitation bytes, foreign QR refusal, oversize refusal. No camera hardware or pairing authority was exercised.")
    }

    private static func qr(_ value: String) throws -> CGImage {
        guard let filter = CIFilter(name: "CIQRCodeGenerator") else { throw Failure.fixture }
        filter.setValue(Data(value.utf8), forKey: "inputMessage")
        filter.setValue("M", forKey: "inputCorrectionLevel")
        guard let output = filter.outputImage?.transformed(by: CGAffineTransform(scaleX: 6, y: 6)),
              let code = CIContext().createCGImage(output, from: output.extent),
              let context = CGContext(data: nil, width: code.width + 64, height: code.height + 64,
                                      bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpaceCreateDeviceRGB(),
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { throw Failure.fixture }
        context.setFillColor(CGColor(gray: 1, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: context.width, height: context.height))
        context.interpolationQuality = .none
        context.draw(code, in: CGRect(x: 32, y: 32, width: code.width, height: code.height))
        guard let image = context.makeImage() else { throw Failure.fixture }
        return image
    }
    enum Failure: Error { case fixture }
}
