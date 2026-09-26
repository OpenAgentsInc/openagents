import Foundation
import ImageIO
import CryptoKit

let arguments = CommandLine.arguments
precondition(arguments.count == 3, "Expected image and expected payload file")
let imageURL = URL(fileURLWithPath: arguments[1])
let expectedURL = URL(fileURLWithPath: arguments[2])
let imageData = try Data(contentsOf: imageURL)
guard let source = CGImageSourceCreateWithData(imageData as CFData, nil),
      let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
    fatalError("Cannot decode synthetic QR image")
}
let expected = try Data(contentsOf: expectedURL)
let actual = try QRInvitation.decode(image: image)
guard Data(actual.utf8) == expected else {
    fatalError("Decoded invitation does not match the Rust output")
}
let record: [String: Any] = [
    "schema": "coder.synthetic-qr-interop.v1",
    "decoder": "shipped QRInvitation.decode via Apple Vision",
    "image_width": image.width,
    "image_height": image.height,
    "payload_bytes": expected.count,
    "exact_payload_match": true,
    "physical_camera_tested": false,
    "network_used": false,
    "image_sha256": SHA256.hash(data: imageData).map { String(format: "%02x", $0) }.joined()
]
let result = try JSONSerialization.data(withJSONObject: record, options: [.prettyPrinted, .sortedKeys])
FileHandle.standardOutput.write(result)
FileHandle.standardOutput.write(Data("\n".utf8))
