// Draws the invitation QR modules that Rust rendered on this device. The
// invitation never reaches a QR-generation service.
import SwiftUI

struct InvitationQRCode: View {
    let qr: ComputersQR

    var body: some View {
        Canvas { context, size in
            let count = max(qr.rows.count, 1)
            // Whole points per module keep edges sharp for a scanner.
            let cell = max(floor(min(size.width, size.height) / CGFloat(count)), 1)
            let side = cell * CGFloat(count)
            context.fill(Path(CGRect(x: 0, y: 0, width: side, height: side)), with: .color(.white))
            for (y, row) in qr.rows.enumerated() {
                for (x, module) in row.enumerated() where module == "1" {
                    let square = CGRect(x: CGFloat(x) * cell, y: CGFloat(y) * cell, width: cell, height: cell)
                    context.fill(Path(square), with: .color(.black))
                }
            }
        }
        .frame(width: 240, height: 240)
        .accessibilityElement()
        .accessibilityLabel("Invitation QR code")
        .accessibilityIdentifier("computers-qr")
    }
}
