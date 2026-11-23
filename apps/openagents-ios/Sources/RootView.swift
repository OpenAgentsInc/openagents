// Firm reason: 6 (iOS SwiftUI root navigation container with pure-black OLED drawer and top bar).
import SwiftUI

struct RootView: View {
    @ObservedObject var store: AppStore

    var body: some View {
        ZStack(alignment: .leading) {
            OATheme.background
                .ignoresSafeArea()

            VStack(spacing: 0) {
                topBar
                Divider().overlay(OATheme.hairline)
                ChatView(store: store)
            }
            .background(OATheme.background)

            if store.isSidebarOpen {
                Color.black.opacity(0.62)
                    .ignoresSafeArea()
                    .onTapGesture {
                        withAnimation(.snappy(duration: 0.22)) {
                            store.isSidebarOpen = false
                        }
                    }
                    .transition(.opacity)

                SidebarView(store: store)
                    .frame(width: 300)
                    .background(OATheme.background)
                    .overlay(alignment: .trailing) {
                        Rectangle()
                            .fill(OATheme.hairline)
                            .frame(width: 0.5)
                            .ignoresSafeArea()
                    }
                    .transition(.move(edge: .leading))
            }
        }
        .animation(.snappy(duration: 0.22), value: store.isSidebarOpen)
        .sheet(isPresented: $store.isComputersPresented) {
            ComputersView(store: store)
                .preferredColorScheme(.dark)
        }
    }

    private var topBar: some View {
        HStack(spacing: 12) {
            Button {
                withAnimation(.snappy(duration: 0.22)) {
                    store.isSidebarOpen.toggle()
                }
            } label: {
                Image(systemName: store.isSidebarOpen ? "xmark" : "sidebar.left")
                    .font(.system(size: 16, weight: .medium))
                    .foregroundStyle(OATheme.textPrimary)
                    .frame(width: 36, height: 36)
                    .contentShape(Rectangle())
            }
            .accessibilityIdentifier("nav.sidebar")
            .accessibilityLabel(store.isSidebarOpen ? "Close sidebar" : "Open sidebar")

            Spacer(minLength: 4)

            Button {
                store.isComputersPresented = true
            } label: {
                HStack(spacing: 6) {
                    Circle()
                        .fill(statusColor)
                        .frame(width: 7, height: 7)
                    Text(headerTitle)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(OATheme.textPrimary)
                        .lineLimit(1)
                    Image(systemName: "chevron.down")
                        .font(.system(size: 10, weight: .semibold))
                        .foregroundStyle(OATheme.textMuted)
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .background(OATheme.surface, in: Capsule())
                .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
            }
            .accessibilityIdentifier("nav.computerPill")

            Spacer(minLength: 4)

            Button {
                store.startNewChat()
            } label: {
                Image(systemName: "square.and.pencil")
                    .font(.system(size: 16, weight: .medium))
                    .foregroundStyle(OATheme.textPrimary)
                    .frame(width: 36, height: 36)
                    .contentShape(Rectangle())
            }
            .accessibilityIdentifier("nav.newChat")
            .accessibilityLabel("New chat")
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(OATheme.background)
    }

    private var headerTitle: String {
        if let session = store.activeSession {
            return session.displayTitle
        }
        if let computer = store.activeComputer {
            return computer.name
        }
        return "Antigravity"
    }

    private var statusColor: Color {
        guard let computer = store.activeComputer else { return OATheme.textMuted }
        switch computer.reachability {
        case .reachable: return OATheme.statusReady
        case .checking: return OATheme.statusWarn
        case .refused, .unreachable: return OATheme.statusError
        case .unknown: return computer.canSendRPC ? OATheme.statusWarn : OATheme.textMuted
        }
    }
}
