import SwiftUI

// SCR-12 Updates (later). Opened from the bell on SCR-01.

enum SCR12State: String, Hashable, CaseIterable {
    case normal, empty
}

struct SCR12Updates: View {
    @Environment(MockApp.self) private var app
    let state: SCR12State

    var body: some View {
        ScreenScaffold {
            TopBar(back: BackControl(label: "Menu") { app.backToMenu() }, title: "Updates")
        } content: {
            if state == .empty {
                Text("Nothing new. Your next update comes when someone checks your result.")
                    .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                    .padding(.top, Theme.Space.xl)
            } else {
                // E01: each row has one button; the newest row's is the primary.
                ForEach(Array(MockData.updates.enumerated()), id: \.element.id) { i, u in
                    Card(highlighted: i == 0) {
                        VStack(alignment: .leading, spacing: Theme.Space.s) {
                            HStack(alignment: .top, spacing: 10) {
                                Circle()
                                    .fill(u.unread ? Theme.Colors.textPrimary : .clear)
                                    .overlay(Circle().stroke(Theme.Colors.textSecondary, lineWidth: 1))
                                    .frame(width: 9, height: 9).padding(.top, 6)
                                Text(u.text).font(Theme.Fonts.body).fixedSize(horizontal: false, vertical: true)
                            }
                            HStack {
                                Spacer()
                                Chip(icon: "chevron.right", text: u.button, filled: i == 0) {
                                    // Each update opens the chat card it's about.
                                    app.go(.conversation(.answer(u.opens)))
                                }
                            }
                        }
                    }
                }
            }
        } bottom: {
            if state == .empty {
                PrimaryButton(title: "Chat with OpenAgents") { app.go(.newChat(.returning)) }
            }
        }
    }
}

#Preview("SCR-12 Updates") {
    NavigationStack { SCR12Updates(state: .normal) }.environment(MockApp())
}

#Preview("SCR-12 Empty") {
    NavigationStack { SCR12Updates(state: .empty) }.environment(MockApp())
}
