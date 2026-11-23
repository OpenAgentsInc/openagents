// Firm reason: 6 (macOS pure-black OLED 3-column SwiftUI workspace: persistent multi-host sidebar, Antigravity chat/composer with drag-drop attachments, and toggleable Workspace/Subagent right inspector).
import AppKit
import SwiftUI
import UniformTypeIdentifiers

struct MacRootView: View {
    @ObservedObject var store: AntigravityStore
    @ObservedObject var hostServer: TailscaleHostServer
    @Binding var showCommandPalette: Bool
    @Binding var showInspector: Bool

    @State private var workingExpanded = true
    @State private var isDropTargeted = false

    var body: some View {
        ZStack {
            HStack(spacing: 0) {
                sidebarColumn
                    .frame(width: 272)
                    .background(OATheme.background)

                Rectangle()
                    .fill(OATheme.hairline)
                    .frame(width: 1)

                centerColumn
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .background(OATheme.background)

                if showInspector || store.activeSubagentItem != nil {
                    Rectangle()
                        .fill(OATheme.hairline)
                        .frame(width: 1)

                    inspectorColumn
                        .frame(width: 360)
                        .background(OATheme.background)
                }
            }
            .background(OATheme.background)

            if showCommandPalette {
                MacCommandPalette(
                    store: store,
                    showInspector: $showInspector,
                    isPresented: $showCommandPalette
                )
            }
        }
        .sheet(isPresented: $store.isComputersPresented) {
            MacHostSettingsView(store: store, hostServer: hostServer)
                .preferredColorScheme(.dark)
        }
        .onChange(of: store.activeSubagentItem) { _, newVal in
            if newVal != nil {
                showInspector = true
            }
        }
    }

    // MARK: - Left Sidebar

    private var sidebarColumn: some View {
        VStack(spacing: 0) {
            VStack(spacing: 10) {
                HStack(spacing: 8) {
                    AntigravityMark(size: 16, color: OATheme.textPrimary)
                    Text("OpenAgents")
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(OATheme.textPrimary)
                    Spacer()
                    if hostServer.isListening {
                        HStack(spacing: 4) {
                            Circle()
                                .fill(hostServer.daemonRunning ? OATheme.statusReady : OATheme.statusWarn)
                                .frame(width: 6, height: 6)
                            Text(":\(hostServer.listenPort)")
                                .font(.system(size: 10, weight: .medium, design: .monospaced))
                                .foregroundStyle(OATheme.textMuted)
                        }
                        .help("Tailscale host listener port")
                    }
                }
                .padding(.top, 28) // clear macOS traffic lights under hidden titlebar

                // Multi-host selector (This Mac + any paired remote Tailscale Macs)
                Menu {
                    ForEach(store.computers) { computer in
                        Button {
                            store.selectComputer(computer)
                        } label: {
                            HStack {
                                Text(computer.isLocalMac ? "\(computer.name) (Local)" : computer.name)
                                if store.activeComputerID == computer.id {
                                    Image(systemName: "checkmark")
                                }
                            }
                        }
                    }
                    Divider()
                    Button("Host & Tailscale Settings…") {
                        store.isComputersPresented = true
                    }
                } label: {
                    HStack(spacing: 8) {
                        Circle()
                            .fill(activeHostDotColor)
                            .frame(width: 7, height: 7)
                        Text(store.activeComputer?.name ?? "This Mac")
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(OATheme.textPrimary)
                            .lineLimit(1)
                        Spacer()
                        Image(systemName: "chevron.up.chevron.down")
                            .font(.system(size: 10))
                            .foregroundStyle(OATheme.textMuted)
                    }
                    .padding(.horizontal, 10)
                    .padding(.vertical, 7)
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                }
                .menuStyle(.borderlessButton)

                Button {
                    store.startNewChat()
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: "plus")
                            .font(.system(size: 12, weight: .semibold))
                        Text("New chat")
                            .font(.system(size: 13, weight: .medium))
                        Spacer()
                        Text("⌘N")
                            .font(.system(size: 10, design: .monospaced))
                            .foregroundStyle(OATheme.textMuted)
                    }
                    .foregroundStyle(OATheme.textPrimary)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 8)
                    .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                }
                .buttonStyle(.plain)

                HStack(spacing: 7) {
                    Image(systemName: "magnifyingglass")
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.textMuted)
                    TextField("Search chats", text: $store.searchText)
                        .textFieldStyle(.plain)
                        .font(.system(size: 13))
                        .foregroundStyle(OATheme.textPrimary)
                }
                .padding(.horizontal, 9)
                .padding(.vertical, 7)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
            }
            .padding(.horizontal, 12)
            .padding(.bottom, 12)

            Divider().overlay(OATheme.hairline)

            ScrollView {
                LazyVStack(alignment: .leading, spacing: 14) {
                    if !store.workingSessions.isEmpty {
                        VStack(alignment: .leading, spacing: 5) {
                            Button {
                                workingExpanded.toggle()
                            } label: {
                                HStack(spacing: 6) {
                                    Text("WORKING")
                                        .font(.system(size: 10, weight: .semibold))
                                        .tracking(0.6)
                                        .foregroundStyle(OATheme.textMuted)
                                    Text("\(store.workingSessions.count)")
                                        .font(.system(size: 10, weight: .semibold, design: .monospaced))
                                        .foregroundStyle(OATheme.textSecondary)
                                    Spacer()
                                    Image(systemName: workingExpanded ? "chevron.down" : "chevron.right")
                                        .font(.system(size: 9, weight: .semibold))
                                        .foregroundStyle(OATheme.textMuted)
                                }
                                .padding(.horizontal, 6)
                            }
                            .buttonStyle(.plain)

                            if workingExpanded {
                                ForEach(store.workingSessions) { session in
                                    sidebarRow(session)
                                }
                            }
                        }
                    }

                    VStack(alignment: .leading, spacing: 5) {
                        Text("RECENTS")
                            .font(.system(size: 10, weight: .semibold))
                            .tracking(0.6)
                            .foregroundStyle(OATheme.textMuted)
                            .padding(.horizontal, 6)

                        if store.recentSessions.isEmpty && store.workingSessions.isEmpty {
                            Text(store.activeComputer?.canSendRPC == true
                                 ? "No Antigravity chats yet."
                                 : "Start the local daemon or pair a Tailscale host.")
                                .font(.system(size: 12))
                                .foregroundStyle(OATheme.textMuted)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 6)
                        } else {
                            ForEach(store.recentSessions) { session in
                                sidebarRow(session)
                            }
                        }
                    }
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 10)
            }

            Divider().overlay(OATheme.hairline)

            Button {
                store.isComputersPresented = true
            } label: {
                HStack(spacing: 9) {
                    Image(systemName: "antenna.radiowaves.left.and.right")
                        .font(.system(size: 13))
                        .foregroundStyle(hostServer.isListening ? OATheme.statusReady : OATheme.textMuted)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Tailscale Host & Computers")
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(OATheme.textPrimary)
                        Text(hostServer.owner.map { $0.dns.isEmpty ? $0.ip : $0.dns } ?? "Configure Tailscale")
                            .font(.system(size: 10, design: .monospaced))
                            .foregroundStyle(OATheme.textMuted)
                            .lineLimit(1)
                    }
                    Spacer()
                    Image(systemName: "gearshape")
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.textMuted)
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 10)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
        }
    }

    private func sidebarRow(_ session: ChatSessionSummary) -> some View {
        let isSelected = store.activeSessionID == session.id
        return Button {
            store.selectSession(session)
        } label: {
            VStack(alignment: .leading, spacing: 3) {
                HStack(spacing: 6) {
                    if session.busy {
                        ProgressView().controlSize(.mini).scaleEffect(0.75)
                    } else if !session.interactionID.isEmpty {
                        Circle().fill(OATheme.statusWarn).frame(width: 6, height: 6)
                    }
                    Text(session.displayTitle)
                        .font(.system(size: 13, weight: isSelected ? .semibold : .regular))
                        .foregroundStyle(OATheme.textPrimary)
                        .lineLimit(1)
                    Spacer(minLength: 4)
                    Text(OATimeFormat.shortAge(updatedAtMs: session.updatedAtMs))
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundStyle(OATheme.textMuted)
                }
                HStack(spacing: 5) {
                    AntigravityMark(size: 9, color: OATheme.textMuted)
                    Text(session.model.isEmpty ? "Antigravity" : session.model)
                        .font(.system(size: 11))
                        .foregroundStyle(OATheme.textSecondary)
                        .lineLimit(1)
                    if session.isNativeAntigravity {
                        Text("local")
                            .font(.system(size: 9, weight: .semibold, design: .monospaced))
                            .foregroundStyle(OATheme.textMuted)
                            .padding(.horizontal, 4)
                            .padding(.vertical, 1)
                            .background(OATheme.surface, in: Capsule())
                    }
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 7)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(isSelected ? OATheme.elevated : Color.clear)
            )
        }
        .buttonStyle(.plain)
    }

    // MARK: - Center Chat Column

    private var centerColumn: some View {
        VStack(spacing: 0) {
            centerTopBar
            Divider().overlay(OATheme.hairline)

            if let toast = store.statusToast, !toast.isEmpty {
                HStack(spacing: 8) {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.statusReady)
                    Text(toast)
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.textPrimary)
                    Spacer()
                    Button { store.statusToast = nil } label: {
                        Image(systemName: "xmark").font(.system(size: 10)).foregroundStyle(OATheme.textMuted)
                    }
                    .buttonStyle(.plain)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 7)
                .background(OATheme.surface)
                Divider().overlay(OATheme.hairline)
            }

            if let error = store.bannerError, !error.isEmpty {
                HStack(spacing: 8) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.statusWarn)
                    Text(error)
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.textPrimary)
                        .lineLimit(2)
                    Spacer()
                    Button { store.bannerError = nil } label: {
                        Image(systemName: "xmark").font(.system(size: 10)).foregroundStyle(OATheme.textMuted)
                    }
                    .buttonStyle(.plain)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 8)
                .background(OATheme.surface)
                Divider().overlay(OATheme.hairline)
            }

            Group {
                if store.activeComputer?.isLocalMac == true && !hostServer.daemonRunning && store.items.isEmpty {
                    localDaemonSetupState
                } else if store.items.isEmpty {
                    emptyReadyState
                } else {
                    transcriptScroll
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)

            if let interaction = store.pendingInteraction {
                ApprovalBannerView(interaction: interaction) { allow, note, optionID in
                    await store.respondToInteraction(allow: allow, note: note, optionID: optionID)
                }
                .padding(.horizontal, 20)
                .padding(.bottom, 8)
            }

            if !store.matchingSlashCommands.isEmpty {
                macSlashMenu
                    .padding(.horizontal, 20)
                    .padding(.bottom, 6)
            } else if !store.matchingFolders.isEmpty {
                macFolderMenu
                    .padding(.horizontal, 20)
                    .padding(.bottom, 6)
            }

            if let queued = store.queuedDraft, !queued.isEmpty {
                HStack(spacing: 8) {
                    Image(systemName: "clock.arrow.circlepath")
                        .font(.system(size: 11))
                        .foregroundStyle(OATheme.textSecondary)
                    Text("Queued: \(queued)")
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.textPrimary)
                        .lineLimit(1)
                    Spacer()
                    Button { store.clearQueuedDraft() } label: {
                        Image(systemName: "xmark").font(.system(size: 10)).foregroundStyle(OATheme.textMuted)
                    }
                    .buttonStyle(.plain)
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 7)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                .padding(.horizontal, 20)
                .padding(.bottom, 6)
            }

            Divider().overlay(OATheme.hairline)
            macComposer
        }
        .onDrop(of: [.fileURL], isTargeted: $isDropTargeted) { providers in
            for provider in providers {
                _ = provider.loadObject(ofClass: URL.self) { url, _ in
                    guard let url, let data = try? Data(contentsOf: url) else { return }
                    let att = Self.makeAttachment(url: url, data: data)
                    DispatchQueue.main.async {
                        MainActor.assumeIsolated { store.addAttachment(att) }
                    }
                }
            }
            return true
        }
        .overlay {
            if isDropTargeted {
                RoundedRectangle(cornerRadius: 12)
                    .stroke(Color.white.opacity(0.6), style: StrokeStyle(lineWidth: 2, dash: [6]))
                    .padding(8)
            }
        }
    }

    private var centerTopBar: some View {
        HStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 1) {
                Text(store.activeSession?.displayTitle ?? (store.activeComputer?.name ?? "Antigravity"))
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(OATheme.textPrimary)
                    .lineLimit(1)
                if !store.workspaceName.isEmpty {
                    Text(store.workspaceName)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(OATheme.textMuted)
                }
            }

            Spacer()

            Button {
                showCommandPalette.toggle()
            } label: {
                HStack(spacing: 5) {
                    Image(systemName: "command")
                        .font(.system(size: 10))
                    Text("K")
                        .font(.system(size: 11, weight: .medium, design: .monospaced))
                }
                .foregroundStyle(OATheme.textSecondary)
                .padding(.horizontal, 8)
                .padding(.vertical, 5)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
            }
            .buttonStyle(.plain)
            .help("Command Palette (⌘K)")

            Button {
                showInspector.toggle()
            } label: {
                HStack(spacing: 5) {
                    Image(systemName: "sidebar.right")
                        .font(.system(size: 12))
                    Text("Workspace")
                        .font(.system(size: 12, weight: .medium))
                }
                .foregroundStyle(showInspector ? Color.black : OATheme.textPrimary)
                .padding(.horizontal, 10)
                .padding(.vertical, 5)
                .background(showInspector ? Color.white : OATheme.surface, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
            }
            .buttonStyle(.plain)
            .help("Toggle Workspace & Diff Inspector (⇧⌘I)")
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 10)
        .background(OATheme.background)
    }

    private var localDaemonSetupState: some View {
        VStack(spacing: 16) {
            Spacer()
            AntigravityMark(size: 36, color: OATheme.textPrimary)
            Text("OpenAgents macOS")
                .font(.system(size: 20, weight: .semibold))
                .foregroundStyle(OATheme.textPrimary)
            Text("Run the local headless Antigravity host daemon (no public relay) to chat locally and serve your iPhone over Tailscale.")
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textSecondary)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 420)

            HStack(spacing: 10) {
                Button("Start Local Host Daemon") {
                    hostServer.startLocalDaemon(devicePubkey: store.devicePublicKeyHex)
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.0) {
                        store.reloadComputers()
                        Task { await store.refreshActiveComputer() }
                    }
                }
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(Color.black)
                .padding(.horizontal, 16)
                .padding(.vertical, 9)
                .background(Color.white, in: Capsule())
                .buttonStyle(.plain)

                Button("Host & Tailscale Settings") {
                    store.isComputersPresented = true
                }
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(OATheme.textPrimary)
                .padding(.horizontal, 16)
                .padding(.vertical, 9)
                .background(OATheme.elevated, in: Capsule())
                .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                .buttonStyle(.plain)
            }
            Spacer()
        }
        .padding(24)
    }

    private var emptyReadyState: some View {
        VStack(spacing: 12) {
            Spacer()
            AntigravityMark(size: 30, color: OATheme.textPrimary)
            Text(store.activeComputer?.name ?? "Antigravity")
                .font(.system(size: 18, weight: .semibold))
                .foregroundStyle(OATheme.textPrimary)
            Text(store.workspaceName.isEmpty
                 ? "Start a new Antigravity task, or press ⌘K to jump anywhere."
                 : "Workspace: \(store.workspaceName)")
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textSecondary)

            if !store.folders.isEmpty {
                Menu {
                    Button("Workspace root") { store.selectedFolder = "" }
                    ForEach(store.folders, id: \.self) { folder in
                        Button(folder) { store.selectedFolder = folder }
                    }
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "folder").font(.system(size: 11))
                        Text(store.selectedFolder.isEmpty ? "Workspace root" : store.selectedFolder)
                            .font(.system(size: 12, design: .monospaced))
                    }
                    .foregroundStyle(OATheme.textSecondary)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 5)
                    .background(OATheme.surface, in: Capsule())
                    .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                }
                .menuStyle(.borderlessButton)
                .fixedSize()
            }
            Spacer()
        }
        .padding(24)
    }

    private var transcriptScroll: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 14) {
                    if let note = store.activeSession?.note, !note.isEmpty {
                        HStack(alignment: .top, spacing: 8) {
                            Image(systemName: "info.circle")
                                .font(.system(size: 13))
                                .foregroundStyle(OATheme.textSecondary)
                            Text(note)
                                .font(.system(size: 13))
                                .foregroundStyle(OATheme.textSecondary)
                        }
                        .padding(10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                    }
                    ForEach(store.items) { item in
                        TranscriptRowView(
                            item: item,
                            localImageLookup: { store.localImageData(for: $0) },
                            onOpenSubagent: { sub in
                                store.openSubagent(sub)
                            }
                        )
                        .id(item.id)
                    }
                    if let statusLabel = store.streamingStatusLabel {
                        StreamingStatusRowView(label: statusLabel)
                            .id("working-indicator")
                    }
                    Color.clear
                        .frame(height: 1)
                        .id("bottom-anchor")
                }
                .padding(.horizontal, 24)
                .padding(.vertical, 18)
                .frame(maxWidth: 820, alignment: .leading)
                .frame(maxWidth: .infinity, alignment: .center)
            }
            .onChange(of: store.streamingContentFingerprint) { _, _ in
                withAnimation(.easeOut(duration: 0.16)) {
                    proxy.scrollTo("bottom-anchor", anchor: .bottom)
                }
            }
        }
    }

    private var macFolderMenu: some View {
        VStack(alignment: .leading, spacing: 2) {
            ForEach(store.matchingFolders, id: \.self) { folder in
                Button {
                    store.applyFolderMention(folder)
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: folder.contains(".") ? "doc.text" : "folder")
                            .font(.system(size: 11))
                            .foregroundStyle(OATheme.textSecondary)
                        Text("@\(folder)")
                            .font(.system(size: 12, weight: .medium, design: .monospaced))
                            .foregroundStyle(OATheme.textPrimary)
                        Spacer()
                    }
                    .padding(.horizontal, 12)
                    .padding(.vertical, 7)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        .frame(maxWidth: 820)
    }

    private var macSlashMenu: some View {
        VStack(alignment: .leading, spacing: 2) {
            ForEach(store.matchingSlashCommands) { cmd in
                Button {
                    store.applySlashCommand(cmd)
                } label: {
                    HStack(spacing: 10) {
                        Text(cmd.command)
                            .font(.system(size: 12, weight: .semibold, design: .monospaced))
                            .foregroundStyle(OATheme.textPrimary)
                        Text(cmd.subtitle)
                            .font(.system(size: 12))
                            .foregroundStyle(OATheme.textSecondary)
                        Spacer()
                    }
                    .padding(.horizontal, 12)
                    .padding(.vertical, 7)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        .frame(maxWidth: 820)
    }

    private var macComposer: some View {
        VStack(spacing: 8) {
            if !store.pendingAttachments.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(store.pendingAttachments) { att in
                            HStack(spacing: 6) {
                                if let imgData = att.imageData {
                                    InlineCGImageView(data: imgData, maxHeight: 20)
                                        .frame(width: 20, height: 20)
                                        .clipped()
                                        .clipShape(RoundedRectangle(cornerRadius: 4, style: .continuous))
                                } else {
                                    Image(systemName: att.textPreview != nil ? "doc.text" : "photo").font(.system(size: 10))
                                }
                                Text(att.name).font(.system(size: 11, design: .monospaced))
                                Button { store.removeAttachment(att) } label: {
                                    Image(systemName: "xmark.circle.fill").font(.system(size: 11))
                                }
                                .buttonStyle(.plain)
                            }
                            .foregroundStyle(OATheme.textSecondary)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(OATheme.elevated, in: Capsule())
                            .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                        }
                    }
                    .padding(.horizontal, 12)
                    .padding(.top, 8)
                }
            }

            TextField("Ask Antigravity, drop files, or type /…", text: $store.draftText, axis: .vertical)
                .textFieldStyle(.plain)
                .lineLimit(1...8)
                .font(.system(size: 14))
                .foregroundStyle(OATheme.textPrimary)
                .padding(.horizontal, 12)
                .padding(.top, store.pendingAttachments.isEmpty ? 10 : 2)
                .onSubmit {
                    guard !store.isSending else { return }
                    Task { await store.sendMessage() }
                }

            HStack(spacing: 8) {
                Button {
                    openFilePanel()
                } label: {
                    Image(systemName: "plus")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(OATheme.textSecondary)
                        .frame(width: 26, height: 26)
                        .background(OATheme.elevated, in: Circle())
                        .overlay(Circle().stroke(OATheme.hairline, lineWidth: 1))
                }
                .buttonStyle(.plain)
                .help("Attach file")

                // Model Picker
                Menu {
                    ForEach(store.availableModels) { model in
                        Button(model.label) { store.chooseModel(model.id) }
                    }
                } label: {
                    composerChip(mark: true, icon: nil, title: store.selectedModelOption.label)
                }
                .menuStyle(.borderlessButton)
                .fixedSize()

                // Effort Picker
                Menu {
                    ForEach(store.supportedEffortsForSelectedModel) { eff in
                        Button(eff.label) { store.chooseEffort(eff) }
                    }
                } label: {
                    composerChip(mark: false, icon: "sparkles", title: store.selectedEffort.label)
                }
                .menuStyle(.borderlessButton)
                .fixedSize()

                // Review Mode Picker
                Menu {
                    ForEach(ReviewMode.allCases) { mode in
                        Button("\(mode.label) — \(mode.description)") { store.chooseReviewMode(mode) }
                    }
                } label: {
                    composerChip(mark: false, icon: store.reviewMode.icon, title: store.reviewMode.label)
                }
                .menuStyle(.borderlessButton)
                .fixedSize()

                Spacer()

                macActionButton
            }
            .padding(.horizontal, 10)
            .padding(.bottom, 10)
        }
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        .frame(maxWidth: 820)
        .padding(.horizontal, 20)
        .padding(.vertical, 12)
    }

    private func composerChip(mark: Bool, icon: String?, title: String) -> some View {
        HStack(spacing: 5) {
            if mark {
                AntigravityMark(size: 10, color: OATheme.textSecondary)
            } else if let icon {
                Image(systemName: icon).font(.system(size: 10)).foregroundStyle(OATheme.textSecondary)
            }
            Text(title)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(OATheme.textSecondary)
        }
        .padding(.horizontal, 9)
        .padding(.vertical, 5)
        .background(OATheme.elevated, in: Capsule())
        .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
    }

    @ViewBuilder
    private var macActionButton: some View {
        let hasPayload = !store.draftText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !store.pendingAttachments.isEmpty
        let isRunning = store.activeSession?.busy == true

        if isRunning && !hasPayload {
            Button {
                Task { await store.stopActiveTurn() }
            } label: {
                Label("Stop", systemImage: "stop.fill")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(Color.black)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .background(Color.white, in: Capsule())
            }
            .buttonStyle(.plain)
        } else {
            HStack(spacing: 6) {
                if isRunning && hasPayload {
                    Button {
                        store.queueCurrentDraft()
                    } label: {
                        Label("Queue", systemImage: "clock.arrow.circlepath")
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(OATheme.textPrimary)
                            .padding(.horizontal, 10)
                            .padding(.vertical, 6)
                            .background(OATheme.elevated, in: Capsule())
                            .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                    }
                    .buttonStyle(.plain)
                }

                Button {
                    Task { await store.sendMessage() }
                } label: {
                    Image(systemName: "arrow.up")
                        .font(.system(size: 12, weight: .bold))
                        .foregroundStyle(hasPayload ? Color.black : OATheme.textMuted)
                        .frame(width: 28, height: 28)
                        .background(hasPayload ? Color.white : OATheme.elevated, in: Circle())
                }
                .buttonStyle(.plain)
                .disabled(!hasPayload || store.isSending)
            }
        }
    }

    // MARK: - Right Inspector Pane (Workspace Diff/PR OR Subagent)

    @ViewBuilder
    private var inspectorColumn: some View {
        if let sub = store.activeSubagentItem {
            VStack(spacing: 0) {
                HStack {
                    Text("SUBAGENT INSPECTOR")
                        .font(.system(size: 10, weight: .semibold))
                        .tracking(0.6)
                        .foregroundStyle(OATheme.textMuted)
                    Spacer()
                    Button {
                        store.closeSubagent()
                    } label: {
                        Image(systemName: "xmark")
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(OATheme.textSecondary)
                    }
                    .buttonStyle(.plain)
                }
                .padding(.horizontal, 14)
                .padding(.vertical, 10)
                Divider().overlay(OATheme.hairline)
                SubagentInspectorView(store: store, subagent: sub)
            }
        } else {
            WorkspaceInspectorView(store: store)
        }
    }

    private var activeHostDotColor: Color {
        guard let c = store.activeComputer else { return OATheme.textMuted }
        if c.isLocalMac {
            return hostServer.daemonRunning ? OATheme.statusReady : OATheme.statusWarn
        }
        switch c.reachability {
        case .reachable: return OATheme.statusReady
        case .checking: return OATheme.statusWarn
        case .refused, .unreachable: return OATheme.statusError
        case .unknown: return c.canSendRPC ? OATheme.statusWarn : OATheme.textMuted
        }
    }

    private func openFilePanel() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = true
        panel.canChooseDirectories = false
        guard panel.runModal() == .OK else { return }
        for url in panel.urls {
            guard let data = try? Data(contentsOf: url) else { continue }
            store.addAttachment(Self.makeAttachment(url: url, data: data))
        }
    }

    private static func makeAttachment(url: URL, data: Data) -> FileAttachment {
        let ext = url.pathExtension.lowercased()
        let imageExts: Set<String> = ["png", "jpg", "jpeg", "heic", "heif", "webp", "gif", "bmp", "tif", "tiff"]
        if imageExts.contains(ext) {
            return FileAttachment.prepareImage(data: data, name: url.lastPathComponent)
        }
        let textSlice: String? = (data.count <= 131_072 ? String(data: data, encoding: .utf8) : nil)
            ?? String(decoding: data.prefix(8_192), as: UTF8.self).nilIfContainsBinaryNUL()
        if let text = textSlice {
            return FileAttachment(
                name: url.lastPathComponent,
                mimeType: ext == "json" ? "application/json" : "text/plain",
                textPreview: text.truncatedToUTF8Bytes(4_000),
                byteCount: data.count
            )
        }
        let maybeImg = FileAttachment.prepareImage(data: data, name: url.lastPathComponent)
        if maybeImg.isImage && maybeImg.imageBase64 != nil {
            return maybeImg
        }
        let mime = ext == "pdf" ? "application/pdf" : "application/octet-stream"
        let binB64 = data.count <= 20_000 ? data.base64EncodedString() : nil
        return FileAttachment(
            name: url.lastPathComponent,
            mimeType: mime,
            textPreview: nil,
            byteCount: data.count,
            binaryBase64: binB64
        )
    }
}
