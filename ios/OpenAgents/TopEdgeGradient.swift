import SwiftUI

/// Subtle black→transparent gradient at the top edge to sit behind a transparent toolbar.
struct TopEdgeGradient: View {
    var body: some View {
        LinearGradient(
            colors: [
                Color.black.opacity(0.78),
                Color.black.opacity(0.48),
                Color.black.opacity(0.0)
            ],
            startPoint: .top,
            endPoint: .bottom
        )
        .frame(height: 120)
        .ignoresSafeArea(edges: .top)
        .allowsHitTesting(false)
    }
}
