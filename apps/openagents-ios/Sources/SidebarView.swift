// Firm reason: 6 (iOS SwiftUI calm pure-black Antigravity sidebar with New chat, Working, Recents, native agy badges, and Tailscale Computers footer).
import SwiftUI

struct SidebarView: View {
    @ObservedObject var store: AppStore
    @State private var workingExpanded = true
    @State private var renamingSession: ChatSessionSummary?
    @State private var renameDraft: String = ""

    var body: some View {
        VStack(spacing: 0) {
            VStack(spacing: 10) {
                HStack(spacing: 8) {
                    AntigravityMark(size: 16, color: OATheme.textPrimary)
                    Text("OpenAgents")
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(OATheme.textPrimary)
                    Spacer()
                    if store.livePushConnected {
                        HStack(spacing: 4) {
                            Circle()
                                .fill(OATheme.statusReady)
                                .frame(width: 6, height: 6)
                            Text("Live")
                                .font(.system(size: 10, weight: .semibold, design: .monospaced))
                                .foregroundStyle(OATheme.textMuted)
                        }
                    }
                }
                .padding(.top, 12)

                Button {
                    store.startNewChat()
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: "plus")
                            .font(.system(size: 13, weight: .semibold))
                        Text("New chat")
                            .font(.system(size: 14, weight: .medium))
                        Spacer()
                    }
                    .foregroundStyle(OATheme.textPrimary)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 9)
                    .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                }
                .accessibilityIdentifier("sidebar.newChat")

                HStack(spacing: 8) {
                    Image(systemName: "magnifyingglass")
                        .font(.system(size: 13))
                        .foregroundStyle(OATheme.textMuted)
                    TextField("Search chats", text: $store.searchText)
                        .font(.system(size: 14))
                        .foregroundStyle(OATheme.textPrimary)
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                        .accessibilityIdentifier("sidebar.search")
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
            }
            .padding(.horizontal, 14)
            .padding(.bottom, 12)

            Divider().overlay(OATheme.hairline)

            ScrollView {
                LazyVStack(alignment: .leading, spacing: 14) {
                    if !store.workingSessions.isEmpty {
                        VStack(alignment: .leading, spacing: 6) {
                            Button {
                                workingExpanded.toggle()
                            } label: {
                                HStack(spacing: 6) {
                                    Text("WORKING")
                                        .font(.system(size: 11, weight: .semibold))
                                        .tracking(0.6)
                                        .foregroundStyle(OATheme.textMuted)
                                    Text("\(store.workingSessions.count)")
                                        .font(.system(size: 11, weight: .semibold, design: .monospaced))
                                        .foregroundStyle(OATheme.textSecondary)
                                    Spacer()
                                    Image(systemName: workingExpanded ? "chevron.down" : "chevron.right")
                                        .font(.system(size: 10, weight: .semibold))
                                        .foregroundStyle(OATheme.textMuted)
                                }
                                .padding(.horizontal, 6)
                            }
                            if workingExpanded {
                                ForEach(store.workingSessions) { session in
                                    sessionRow(session)
                                }
                            }
                        }
                    }

                    VStack(alignment: .leading, spacing: 6) {
                        HStack {
                            Text("RECENTS")
                                .font(.system(size: 11, weight: .semibold))
                                .tracking(0.6)
                                .foregroundStyle(OATheme.textMuted)
                            Spacer()
                            Button {
                                store.showArchivedSessions.toggle()
                            } label: {
                                Text(store.showArchivedSessions ? "Hide Archived" : "Show Archived")
                                    .font(.system(size: 10.5, weight: .medium))
                                    .foregroundStyle(OATheme.textMuted)
                            }
                            .buttonStyle(.plain)
                        }
                        .padding(.horizontal, 6)

                        if store.recentSessions.isEmpty && store.workingSessions.isEmpty {
                            Text(store.activeComputer?.canSendRPC == true
                                 ? "No Antigravity chats yet."
                                 : "Connect your Mac over Tailscale to load Antigravity chats.")
                                .font(.system(size: 13))
                                .foregroundStyle(OATheme.textMuted)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 8)
                        } else {
                            ForEach(store.recentSessions) { session in
                                sessionRow(session)
                            }
                            if store.hasMoreSessions {
                                Button {
                                    Task { await store.loadMoreSessions() }
                                } label: {
                                    HStack(spacing: 6) {
                                        if store.isLoadingMoreSessions {
                                            ProgressView().controlSize(.mini).tint(OATheme.textSecondary)
                                        }
                                        Text("Load more chats")
                                            .font(.system(size: 12, weight: .medium))
                                    }
                                    .foregroundStyle(OATheme.textSecondary)
                                    .frame(maxWidth: .infinity)
                                    .padding(.vertical, 8)
                                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                                    .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                                }
                                .buttonStyle(.plain)
                                .disabled(store.isLoadingMoreSessions)
                                .padding(.top, 4)
                            }
                        }
                    }
                }
                .padding(.horizontal, 10)
                .padding(.vertical, 12)
            }

            Divider().overlay(OATheme.hairline)

            Button {
                store.isSidebarOpen = false
                store.isComputersPresented = true
            } label: {
                HStack(spacing: 10) {
                    Image(systemName: "desktopcomputer")
                        .font(.system(size: 15))
                        .foregroundStyle(OATheme.textSecondary)
                    VStack(alignment: .leading, spacing: 2) {
                        Text(store.activeComputer?.name ?? "Computers")
                            .font(.system(size: 14, weight: .medium))
                            .foregroundStyle(OATheme.textPrimary)
                            .lineLimit(1)
                        Text(store.activeComputer?.displayAddress ?? "Connect over Tailscale")
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundStyle(OATheme.textMuted)
                            .lineLimit(1)
                    }
                    Spacer()
                    Image(systemName: "chevron.right")
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(OATheme.textMuted)
                }
                .padding(.horizontal, 14)
                .padding(.vertical, 12)
                .contentShape(Rectangle())
            }
            .accessibilityIdentifier("sidebar.computers")
        }
        .background(OATheme.background)
        .alert("Rename Chat", isPresented: Binding(
            get: { renamingSession != nil },
            set: { if !$0 { renamingSession = nil } }
        )) {
            TextField("Chat title", text: $renameDraft)
            Button("Save") {
                if let target = renamingSession {
                    let title = renameDraft
                    renamingSession = nil
                    Task { await store.renameSession(target, title: title) }
                }
            }
            Button("Cancel", role: .cancel) { renamingSession = nil }
        }
    }

    private func sessionRow(_ session: ChatSessionSummary) -> some View {
        let isSelected = store.activeSessionID == session.id
        let pinned = store.isPinned(session)
        let archived = store.isArchived(session)
        return Button {
            store.selectSession(session)
        } label: {
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 6) {
                    if session.busy {
                        ProgressView()
                            .controlSize(.mini)
                            .tint(OATheme.textPrimary)
                    } else if !session.interactionID.isEmpty {
                        Circle()
                            .fill(OATheme.statusWarn)
                            .frame(width: 6, height: 6)
                    } else if pinned {
                        Image(systemName: "pin.fill")
                            .font(.system(size: 10))
                            .foregroundStyle(OATheme.textSecondary)
                    }
                    Text(store.displayTitle(for: session))
                        .font(.system(size: 14, weight: isSelected ? .semibold : .regular))
                        .foregroundStyle(archived ? OATheme.textMuted : OATheme.textPrimary)
                        .lineLimit(1)
                    Spacer(minLength: 4)
                    Text(OATimeFormat.shortAge(updatedAtMs: session.updatedAtMs))
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(OATheme.textMuted)
                }
                HStack(spacing: 6) {
                    AntigravityMark(size: 10, color: OATheme.textMuted)
                    Text(subtitleFor(session))
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.textSecondary)
                        .lineLimit(1)
                    if archived {
                        Text("archived")
                            .font(.system(size: 9, weight: .semibold, design: .monospaced))
                            .foregroundStyle(OATheme.textMuted)
                            .padding(.horizontal, 5)
                            .padding(.vertical, 1)
                            .background(OATheme.surface, in: Capsule())
                            .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                    } else if session.isNativeAntigravity {
                        Text("local")
                            .font(.system(size: 9, weight: .semibold, design: .monospaced))
                            .foregroundStyle(OATheme.textMuted)
                            .padding(.horizontal, 5)
                            .padding(.vertical, 1)
                            .background(OATheme.surface, in: Capsule())
                            .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                    }
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 8, style: .continuous)
                    .fill(isSelected ? OATheme.elevated : Color.clear)
            )
        }
        .buttonStyle(.plain)
        .contextMenu {
            Button {
                Task { await store.togglePinSession(session) }
            } label: {
                Label(pinned ? "Unpin" : "Pin", systemImage: pinned ? "pin.slash" : "pin")
            }
            Button {
                renameDraft = store.displayTitle(for: session)
                renamingSession = session
            } label: {
                Label("Rename", systemImage: "pencil")
            }
            Button {
                store.selectSession(session)
                Task { await store.forkActiveSession() }
            } label: {
                Label("Fork Session", systemImage: "arrow.triangle.branch")
            }
            Divider()
            Button {
                Task { await store.toggleArchiveSession(session) }
            } label: {
                Label(archived ? "Restore" : "Archive", systemImage: archived ? "tray.and.arrow.up" : "archivebox")
            }
        }
    }

    private func subtitleFor(_ session: ChatSessionSummary) -> String {
        let workspace = store.workspaceName.isEmpty ? (store.activeComputer?.name ?? "Mac") : store.workspaceName
        if !session.model.isEmpty {
            return "\(workspace) · \(session.model)"
        }
        return workspace
    }
}
