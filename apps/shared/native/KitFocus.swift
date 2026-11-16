// Firm reason: 6 (the platform keyboard's lifetime over a GPU surface).
// When a tap ends text entry (GitHub #13645). Every GPU field's platform editor follows one rule:
// a tap on the surface that lands outside the focused field's frame gives up the keyboard. The tap
// still reaches the surface as an ordinary press, so a control it lands on acts as usual. The
// surface hears the focus end through its environment (the editor no longer holds the keyboard),
// and its Coax clears the focused field.
//   apps/shared/native/app-services-check.sh
import Foundation
import CoreGraphics

enum KitFocus {
    /// Whether a tap at `point` ends editing. `field` is the focused field's frame in the same
    /// space, nil when no field holds the keyboard. A tap on the frame's edge stays inside.
    static func tapEndsEditing(at point: CGPoint, field: CGRect?) -> Bool {
        guard let field, !field.isNull, !field.isEmpty else { return false }
        return !field.insetBy(dx: -0.5, dy: -0.5).contains(point)
    }
}
