import SwiftUI
#if os(iOS)
import UIKit
#endif

/// A small floating toolbar anchored above the bottom-right corner on iPhone.
/// - Uses Liquid Glass on supported OS versions; falls back to thin material.
struct FloatingToolbar: View {
    var body: some View {
        #if os(iOS)
        Group {
            if UIDevice.current.userInterfaceIdiom == .phone {
                content
            } else {
                EmptyView()
            }
        }
        #else
        EmptyView()
        #endif
    }

    private var content: some View {
        // Pencil is disabled; render nothing for now.
        EmptyView()
    }
}

#Preview {
    ZStack(alignment: .bottomTrailing) {
        OATheme.Colors.background.ignoresSafeArea()
        FloatingToolbar()
    }
}
