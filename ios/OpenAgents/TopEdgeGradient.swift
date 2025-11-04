import SwiftUI

/// Subtle black→transparent gradient at the top edge to sit behind a transparent toolbar.
struct TopEdgeGradient: View {
    var body: some View {
        LinearGradient(
            colors: [
                Color.black.opacity(0.55),
                Color.black.opacity(0.28),
                Color.black.opacity(0.0)
            ],
            startPoint: .top,
            endPoint: .bottom
        )
        .frame(height: 84)
        .ignoresSafeArea(edges: .top)
        .allowsHitTesting(false)
    }
}

