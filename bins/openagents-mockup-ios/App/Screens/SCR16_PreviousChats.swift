import SwiftUI

// SCR-16 Chat: previous chats. Behind ☰, newest first.

enum SCR16State: String, Hashable, CaseIterable {
    case normal, empty, computerOffline
}

struct SCR16PreviousChats: View {
    @Environment(MockApp.self) private var app
    let state: SCR16State

    var body: some View {
        VStack(spacing: 0) {
            // E01, E02, E03
            ChatHeader(back: BackControl(label: "OpenAgents") { app.back() }, onMenu: nil, title: "Chats") {
                Button { app.go(.newChat(.returning)) } label: {
                    Image(systemName: "square.and.pencil").font(.system(size: 19, weight: .semibold))
                        .frame(width: 44, height: 44)
                }
            }
            if state == .empty {
                VStack(spacing: Theme.Space.m) {
                    Spacer()
                    Text("No chats yet.").font(Theme.Fonts.title)
                    PrimaryButton(title: "New chat") { app.go(.newChat(.firstTime)) }
                    Spacer()
                }
                .padding(.horizontal, Theme.Space.page)
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 0) {
                        // E04
                        if state == .computerOffline {
                            StatusPill(text: "\(MockData.computerName) is offline.", live: false)
                                .padding(.vertical, Theme.Space.s)
                        }
                        // E05
                        ForEach(Array(MockData.previousChats.enumerated()), id: \.element.id) { i, chat in
                            ListRow(icon: chat.onComputer ? "desktopcomputer" : "bubble.left",
                                    title: chat.title, trailing: chat.when) {
                                switch i {
                                case 0: app.go(.conversation(.aboutResult))
                                case 1: app.go(.coderChat(.done))
                                default: app.go(.conversation(.answer("cost")))
                                }
                            }
                            Divider().overlay(Theme.Colors.divider)
                        }
                    }
                    .padding(.horizontal, Theme.Space.page)
                }
            }
        }
        .foregroundStyle(Theme.Colors.textPrimary)
        .background(Theme.Colors.background.ignoresSafeArea())
    }
}

#Preview("SCR-16 Previous chats") {
    NavigationStack { SCR16PreviousChats(state: .normal) }.environment(MockApp())
}

#Preview("SCR-16 Empty") {
    NavigationStack { SCR16PreviousChats(state: .empty) }.environment(MockApp())
}
