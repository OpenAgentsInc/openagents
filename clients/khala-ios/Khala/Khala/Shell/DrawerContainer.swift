import SwiftUI

/// A ChatGPT-style left slide-over drawer container.
///
/// Hosts `content` (the main chat surface) and a left `drawer` overlay that the
/// hamburger toggles. Tapping the dim scrim or dragging closes it; an edge drag
/// from the left opens it. Reduced-motion safe: when Reduce Motion is on, the
/// open/close is a crossfade with no spring.
///
/// The shell owns ONLY the slide/scrim/gesture mechanics. The drawer CONTENTS
/// (Recents, search, New Chat, settings — issue #6344) and the chat-view
/// internals (issue #6345) are passed in as `drawer` / `content` closures and
/// filled by the feature lanes.
struct DrawerContainer<Content: View, Drawer: View>: View {
    @Binding var isOpen: Bool
    @ViewBuilder var content: () -> Content
    @ViewBuilder var drawer: () -> Drawer

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    /// Live drag translation while the user is dragging the drawer.
    @State private var dragOffset: CGFloat = 0

    /// Fraction of the screen width the open drawer covers.
    private let widthFraction: CGFloat = 0.84
    private let maxWidth: CGFloat = 360

    var body: some View {
        GeometryReader { geo in
            let drawerWidth = min(geo.size.width * widthFraction, maxWidth)
            // Resting x-offset of the drawer's leading edge: 0 when open,
            // -drawerWidth when closed, plus the live drag delta (clamped).
            let baseOffset = isOpen ? 0 : -drawerWidth
            let liveOffset = clamp(baseOffset + dragOffset, lower: -drawerWidth, upper: 0)
            // Scrim opacity tracks how far open the drawer is (0...1).
            let openFraction = (liveOffset + drawerWidth) / drawerWidth

            ZStack(alignment: .leading) {
                content()
                    .frame(width: geo.size.width, height: geo.size.height)
                    // Disable touches on the chat surface while the drawer is open
                    // so taps go to the scrim/drawer.
                    .allowsHitTesting(!isOpen)

                // Dim scrim over the chat surface. Tap to close.
                Color.black
                    .opacity(0.45 * openFraction)
                    .ignoresSafeArea()
                    .allowsHitTesting(openFraction > 0.01)
                    .onTapGesture { setOpen(false) }
                    .accessibilityHidden(openFraction < 0.5)

                drawer()
                    .frame(width: drawerWidth, height: geo.size.height)
                    .background(.regularMaterial)
                    .offset(x: liveOffset)
                    .accessibilityElement(children: .contain)
                    .accessibilityHidden(openFraction < 0.5)
            }
            .frame(width: geo.size.width, height: geo.size.height)
            .contentShape(Rectangle())
            .gesture(dragGesture(drawerWidth: drawerWidth, screenWidth: geo.size.width))
            .animation(motionAnimation, value: isOpen)
        }
    }

    // MARK: - Gesture

    private func dragGesture(drawerWidth: CGFloat, screenWidth: CGFloat) -> some Gesture {
        DragGesture(minimumDistance: 12, coordinateSpace: .global)
            .onChanged { value in
                if isOpen {
                    // Allow dragging the open drawer closed (leftward).
                    dragOffset = min(0, value.translation.width)
                } else {
                    // Only an edge drag from the very left opens the drawer.
                    guard value.startLocation.x < 28 else { return }
                    dragOffset = max(0, value.translation.width)
                }
            }
            .onEnded { value in
                let projected = value.predictedEndTranslation.width
                if isOpen {
                    setOpen(!(projected < -drawerWidth * 0.25))
                } else if value.startLocation.x < 28 {
                    setOpen(projected > drawerWidth * 0.25)
                }
                dragOffset = 0
            }
    }

    private var motionAnimation: Animation? {
        reduceMotion ? .easeInOut(duration: 0.2) : .interactiveSpring(response: 0.32, dampingFraction: 0.86)
    }

    private func setOpen(_ open: Bool) {
        withAnimation(motionAnimation) {
            isOpen = open
            dragOffset = 0
        }
    }

    private func clamp(_ value: CGFloat, lower: CGFloat, upper: CGFloat) -> CGFloat {
        min(max(value, lower), upper)
    }
}
