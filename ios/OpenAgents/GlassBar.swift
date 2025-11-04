import SwiftUI

struct GlassBar<Content: View>: View {
    var content: () -> Content

    init(@ViewBuilder content: @escaping () -> Content) {
        self.content = content
    }

    var body: some View {
        Group {
            #if canImport(SwiftUI)
            if #available(iOS 26, macOS 15, *) {
                GlassEffectContainer {
                    ZStack {
                        Rectangle().fill(Color.clear)
                            .glassEffect(.regular, in: Rectangle())
                        HStack { content() }.padding(.horizontal, 12)
                    }
                }
            } else {
                fallback
            }
            #else
            fallback
            #endif
        }
        .frame(height: 52)
        .overlay(Divider().opacity(0.25), alignment: .top)
    }

    private var fallback: some View {
        ZStack {
            Rectangle().fill(.ultraThinMaterial)
            HStack { content() }.padding(.horizontal, 12)
        }
    }
}

