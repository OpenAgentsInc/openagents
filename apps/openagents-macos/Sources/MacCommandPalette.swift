// Firm reason: 6 (macOS Cmd-K quick command palette for switching Antigravity sessions, hosts, models, review modes, and inspector).
import SwiftUI

struct MacCommandPalette: View {
    @ObservedObject var store: AntigravityStore
    @Binding var showInspector: Bool
    @Binding var isPresented: Bool

    @State private var query = ""
    @FocusState private var searchFocused: Bool

    private struct ActionItem: Identifiable {
        let id: String
        let icon: String
        let title: String
        let subtitle: String
        let run: () -> Void
    }

    private var items: [ActionItem] {
        var all: [ActionItem] = [
            ActionItem(id: "new-chat", icon: "plus", title: "New Antigravity Chat", subtitle: "Start a fresh session (⌘N)") {
                store.startNewChat()
            },
            ActionItem(id: "toggle-inspector", icon: "sidebar.right", title: showInspector ? "Hide Workspace Inspector" : "Show Workspace Inspector", subtitle: "Git Diff, Branches, Worktrees, PRs (⇧⌘I)") {
                showInspector.toggle()
            },
            ActionItem(id: "host-settings", icon: "antenna.radiowaves.left.and.right", title: "Host & Tailscale Computers", subtitle: "Manage local daemon & remote Macs (⌘,)") {
                store.isComputersPresented = true
            },
        ]

        for mode in ReviewMode.allCases {
            all.append(ActionItem(id: "mode-\(mode.id)", icon: mode.icon, title: "Review Mode: \(mode.label)", subtitle: mode.description) {
                store.chooseReviewMode(mode)
            })
        }

        for model in store.availableModels {
            all.append(ActionItem(id: "model-\(model.id)", icon: "sparkles", title: "Model: \(model.label)", subtitle: model.id) {
                store.chooseModel(model.id)
            })
        }

        for computer in store.computers {
            all.append(ActionItem(id: "host-\(computer.id)", icon: "desktopcomputer", title: "Switch Host: \(computer.name)", subtitle: computer.displayAddress) {
                store.selectComputer(computer)
            })
        }

        for session in store.sessions.prefix(12) {
            all.append(ActionItem(id: "sess-\(session.id)", icon: "bubble.left", title: session.displayTitle, subtitle: session.model.isEmpty ? "Antigravity" : session.model) {
                store.selectSession(session)
            })
        }

        let q = query.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !q.isEmpty else { return all }
        return all.filter {
            $0.title.lowercased().contains(q) || $0.subtitle.lowercased().contains(q)
        }
    }

    var body: some View {
        ZStack {
            Color.black.opacity(0.62)
                .ignoresSafeArea()
                .onTapGesture { isPresented = false }

            VStack(spacing: 0) {
                HStack(spacing: 10) {
                    Image(systemName: "magnifyingglass")
                        .font(.system(size: 14))
                        .foregroundStyle(OATheme.textMuted)
                    TextField("Jump to chat, host, model, or action…", text: $query)
                        .textFieldStyle(.plain)
                        .font(.system(size: 15))
                        .foregroundStyle(OATheme.textPrimary)
                        .focused($searchFocused)
                        .onSubmit {
                            if let first = items.first {
                                first.run()
                                isPresented = false
                            }
                        }
                    Button("Esc") { isPresented = false }
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(OATheme.textMuted)
                        .buttonStyle(.plain)
                }
                .padding(.horizontal, 14)
                .padding(.vertical, 12)

                Divider().overlay(OATheme.hairline)

                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 2) {
                        ForEach(items) { item in
                            Button {
                                item.run()
                                isPresented = false
                            } label: {
                                HStack(spacing: 10) {
                                    Image(systemName: item.icon)
                                        .font(.system(size: 13))
                                        .foregroundStyle(OATheme.textSecondary)
                                        .frame(width: 20)
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(item.title)
                                            .font(.system(size: 13, weight: .medium))
                                            .foregroundStyle(OATheme.textPrimary)
                                        Text(item.subtitle)
                                            .font(.system(size: 11))
                                            .foregroundStyle(OATheme.textMuted)
                                            .lineLimit(1)
                                    }
                                    Spacer()
                                }
                                .padding(.horizontal, 12)
                                .padding(.vertical, 8)
                                .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    .padding(6)
                }
                .frame(maxHeight: 340)
            }
            .frame(width: 520)
            .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
            .shadow(color: .black.opacity(0.6), radius: 30, y: 12)
        }
        .onAppear { searchFocused = true }
    }
}
