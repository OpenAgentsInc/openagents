// Firm reason: 6 (macOS Cmd-K quick command palette with keyboard navigation, Esc dismissal, Thinking Effort, Stop Turn, Push Branch, Antigravity models, review modes, sessions, and hosts).
import SwiftUI

struct MacCommandPalette: View {
    @ObservedObject var store: AntigravityStore
    @Binding var showInspector: Bool
    @Binding var isPresented: Bool

    @State private var query = ""
    @State private var selectedIndex = 0
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
            ActionItem(id: "host-settings", icon: "antenna.radiowaves.left.and.right", title: "Host & Tailscale Computers", subtitle: "Manage local daemon, Antigravity auth & remote Macs (⌘,)") {
                store.isComputersPresented = true
            },
        ]

        if store.activeSession?.busy == true {
            all.append(ActionItem(id: "stop-turn", icon: "stop.fill", title: "Stop Active Turn", subtitle: "Cancel the currently running Antigravity turn") {
                Task { await store.stopActiveTurn() }
            })
        }

        if let snap = store.workspaceSnapshot, !snap.branch.isEmpty {
            all.append(ActionItem(id: "push-branch", icon: "arrow.up.circle", title: "Push Current Branch (\(snap.branch))", subtitle: "Push \(snap.branch) to origin") {
                Task { await store.pushCurrentBranch() }
            })
        }

        for mode in ReviewMode.allCases {
            all.append(ActionItem(id: "mode-\(mode.id)", icon: mode.icon, title: "Review Mode: \(mode.label)", subtitle: mode.description) {
                store.chooseReviewMode(mode)
            })
        }

        for effort in store.supportedEffortsForSelectedModel {
            all.append(ActionItem(id: "effort-\(effort.id)", icon: "dial.high", title: "Thinking Effort: \(effort.label)", subtitle: "Set Antigravity reasoning effort to \(effort.rawValue)") {
                store.chooseEffort(effort)
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

    private func runSelected() {
        let list = items
        guard !list.isEmpty else { return }
        let idx = min(max(0, selectedIndex), list.count - 1)
        list[idx].run()
        isPresented = false
    }

    var body: some View {
        let list = items
        ZStack {
            Color.black.opacity(0.62)
                .ignoresSafeArea()
                .onTapGesture { isPresented = false }

            VStack(spacing: 0) {
                HStack(spacing: 10) {
                    Image(systemName: "magnifyingglass")
                        .font(.system(size: 14))
                        .foregroundStyle(OATheme.textMuted)
                    TextField("Jump to chat, host, model, effort, or action…", text: $query)
                        .textFieldStyle(.plain)
                        .font(.system(size: 15))
                        .foregroundStyle(OATheme.textPrimary)
                        .focused($searchFocused)
                        .onSubmit { runSelected() }
                    Button("Esc") { isPresented = false }
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(OATheme.textMuted)
                        .buttonStyle(.plain)
                }
                .padding(.horizontal, 14)
                .padding(.vertical, 12)

                Divider().overlay(OATheme.hairline)

                ScrollViewReader { proxy in
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 2) {
                            ForEach(Array(list.enumerated()), id: \.element.id) { idx, item in
                                let isSelected = idx == min(selectedIndex, max(0, list.count - 1))
                                Button {
                                    item.run()
                                    isPresented = false
                                } label: {
                                    HStack(spacing: 10) {
                                        Image(systemName: item.icon)
                                            .font(.system(size: 13))
                                            .foregroundStyle(isSelected ? OATheme.textPrimary : OATheme.textSecondary)
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
                                    .background(
                                        RoundedRectangle(cornerRadius: 8, style: .continuous)
                                            .fill(isSelected ? OATheme.elevated : Color.clear)
                                    )
                                    .contentShape(Rectangle())
                                }
                                .buttonStyle(.plain)
                                .id(item.id)
                            }
                        }
                        .padding(6)
                    }
                    .frame(maxHeight: 340)
                    .onChange(of: selectedIndex) { _, newIdx in
                        if newIdx >= 0 && newIdx < list.count {
                            proxy.scrollTo(list[newIdx].id, anchor: .center)
                        }
                    }
                }
            }
            .frame(width: 520)
            .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
            .shadow(color: .black.opacity(0.6), radius: 30, y: 12)
        }
        .onAppear {
            selectedIndex = 0
            searchFocused = true
        }
        .onChange(of: query) { _, _ in
            selectedIndex = 0
        }
        .onKeyPress(.escape) {
            isPresented = false
            return .handled
        }
        .onKeyPress(.downArrow) {
            if !list.isEmpty {
                selectedIndex = (selectedIndex + 1) % list.count
            }
            return .handled
        }
        .onKeyPress(.upArrow) {
            if !list.isEmpty {
                selectedIndex = (selectedIndex - 1 + list.count) % list.count
            }
            return .handled
        }
    }
}
