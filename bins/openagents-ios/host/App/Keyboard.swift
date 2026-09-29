// Keyboard dismissal shared by every screen.
import SwiftUI
import UIKit

/// Dismiss the keyboard from a Done bar just above it and from a tap
/// anywhere outside a text field. The tap does not stop the touch, so a
/// button tapped while the keyboard is up still acts.
struct DismissesKeyboard: ViewModifier {
    @State private var keyboard = false

    func body(content: Content) -> some View {
        content
            .background(OutsideTap().frame(width: 0, height: 0))
            .safeAreaInset(edge: .bottom, spacing: 0) {
                if keyboard {
                    HStack {
                        Spacer()
                        Button("Done") { OutsideTap.dismiss() }
                            .fontWeight(.semibold).tint(.white)
                            .accessibilityIdentifier("keyboard-done")
                    }
                    .padding(.horizontal, 20).padding(.vertical, 10)
                    .background(Color(white: 0.1))
                }
            }
            .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillShowNotification)) { _ in
                keyboard = true
            }
            .onReceive(NotificationCenter.default.publisher(for: UIResponder.keyboardWillHideNotification)) { _ in
                keyboard = false
            }
    }
}

extension View {
    func dismissesKeyboard() -> some View { modifier(DismissesKeyboard()) }

    /// End editing when a tap lands anywhere outside a text field, on every
    /// screen: applied once at the app's root. The tap still reaches what
    /// it hit, so a tab or button tapped with the keyboard up still acts.
    func dismissesKeyboardOnOutsideTap() -> some View {
        background(OutsideTap().frame(width: 0, height: 0))
    }
}

/// Watches taps on its window while it is on screen and ends editing when one
/// lands outside a text field.
struct OutsideTap: UIViewRepresentable {
    static func dismiss() {
        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
    }

    func makeUIView(context: Context) -> Watcher { Watcher() }
    func updateUIView(_ view: Watcher, context: Context) {}

    final class Watcher: UIView, UIGestureRecognizerDelegate {
        private lazy var tap: UITapGestureRecognizer = {
            let tap = UITapGestureRecognizer(target: self, action: #selector(tapped))
            tap.cancelsTouchesInView = false
            tap.delegate = self
            return tap
        }()

        override func didMoveToWindow() {
            super.didMoveToWindow()
            tap.view?.removeGestureRecognizer(tap)
            window?.addGestureRecognizer(tap)
        }

        @objc private func tapped() { OutsideTap.dismiss() }

        func gestureRecognizer(_ recognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
            var view = touch.view
            while let current = view {
                if current is UITextField || current is UITextView { return false }
                view = current.superview
            }
            return true
        }

        func gestureRecognizer(_ recognizer: UIGestureRecognizer,
                               shouldRecognizeSimultaneouslyWith other: UIGestureRecognizer) -> Bool { true }
    }
}
