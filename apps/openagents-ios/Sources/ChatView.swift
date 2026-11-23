// Firm reason: 6 (iOS SwiftUI chat transcript + interactive approval banner + attachments + / slash commands + queued follow-ups + native Antigravity composer).
import PhotosUI
import SwiftUI
import UIKit
import UniformTypeIdentifiers

struct ChatView: View {
    @ObservedObject var store: AppStore
    @StateObject private var dictation = VoiceDictationController()
    @FocusState private var composerFocused: Bool
    @State private var showFileImporter = false
    @State private var showPhotoPicker = false
    @State private var selectedPhotoItem: PhotosPickerItem?
    @State private var isLoadingAttachment = false
    @State private var isPinnedToBottom = true
    @State private var editingMessageItem: TranscriptItem?
    @State private var editingMessageDraft: String = ""

    var body: some View {
        VStack(spacing: 0) {
            if let toast = store.statusToast, !toast.isEmpty {
                toastBanner(toast)
            }
            if let error = store.bannerError, !error.isEmpty {
                errorBanner(error)
            }

            Group {
                if store.activeComputer?.canSendRPC != true && store.items.isEmpty {
                    tailscaleOnboardingState
                } else if store.items.isEmpty {
                    readyEmptyChatState
                } else {
                    transcriptScrollView
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .contentShape(Rectangle())
            .onTapGesture {
                composerFocused = false
            }

            if let interaction = store.pendingInteraction {
                ApprovalBannerView(interaction: interaction) { allow, note, optionID in
                    UIImpactFeedbackGenerator(style: .medium).impactOccurred()
                    await store.respondToInteraction(allow: allow, note: note, optionID: optionID)
                }
                .padding(.horizontal, 12)
                .padding(.bottom, 8)
            }

            if !store.matchingSlashCommands.isEmpty {
                slashCommandMenu
                    .padding(.horizontal, 12)
                    .padding(.bottom, 6)
            } else if !store.matchingMentions.isEmpty {
                mentionMenu
                    .padding(.horizontal, 12)
                    .padding(.bottom, 6)
            }

            if !store.inlineReviewComments.isEmpty {
                inlineCommentsChip
                    .padding(.horizontal, 12)
                    .padding(.bottom, 6)
            }

            if let queued = store.queuedDraft, !queued.isEmpty {
                queuedBanner(queued)
                    .padding(.horizontal, 12)
                    .padding(.bottom, 6)
            }

            SessionTodosAndUsageStrip(
                todos: store.activeTodos,
                usage: store.activeUsage,
                onCompact: { Task { await store.triggerCompact() } }
            )

            Divider().overlay(OATheme.hairline)
            composerBar
        }
        .background(OATheme.background)
        .sheet(isPresented: $store.isSideQuestionPresented) {
            SideQuestionSheetView(store: store)
                .preferredColorScheme(.dark)
        }
        .alert("Edit & Resend Message", isPresented: Binding(
            get: { editingMessageItem != nil },
            set: { if !$0 { editingMessageItem = nil } }
        )) {
            TextField("Message", text: $editingMessageDraft)
            Button("Send Edited") {
                if let target = editingMessageItem {
                    let updated = editingMessageDraft
                    editingMessageItem = nil
                    Task { await store.editAndResendMessage(target, editedText: updated) }
                }
            }
            Button("Cancel", role: .cancel) { editingMessageItem = nil }
        }
        .onChange(of: dictation.errorMessage) { _, msg in
            if let msg, !msg.isEmpty {
                store.bannerError = msg
            }
        }
        .photosPicker(isPresented: $showPhotoPicker, selection: $selectedPhotoItem, matching: .images)
        .onChange(of: selectedPhotoItem) { _, newItem in
            guard let newItem else { return }
            isLoadingAttachment = true
            Task {
                defer {
                    isLoadingAttachment = false
                    selectedPhotoItem = nil
                }
                guard let data = try? await newItem.loadTransferable(type: Data.self) else {
                    store.bannerError = "Couldn't read the selected photo."
                    return
                }
                let filename = "photo-\(Int(Date().timeIntervalSince1970) % 10000).jpg"
                let att: FileAttachment = await Task.detached(priority: .userInitiated) {
                    FileAttachment.prepareImage(data: data, name: filename)
                }.value
                store.addAttachment(att)
            }
        }
        .fileImporter(
            isPresented: $showFileImporter,
            allowedContentTypes: [.plainText, .sourceCode, .json, .data, .image],
            allowsMultipleSelection: false
        ) { result in
            guard case .success(let urls) = result, let url = urls.first else { return }
            let access = url.startAccessingSecurityScopedResource()
            defer { if access { url.stopAccessingSecurityScopedResource() } }
            guard let data = try? Data(contentsOf: url) else { return }
            let ext = url.pathExtension.lowercased()
            let imageExts: Set<String> = ["png", "jpg", "jpeg", "heic", "heif", "webp", "gif", "bmp", "tif", "tiff"]
            if imageExts.contains(ext) {
                store.addAttachment(FileAttachment.prepareImage(data: data, name: url.lastPathComponent))
                return
            }
            // Decode full UTF-8 (bounded to 128 KB read) before UTF-8-safe byte truncation so multi-byte characters
            // at byte 4000 never corrupt into nil and misclassify text files as binary (#14002).
            let textSlice: String? = (data.count <= 131_072 ? String(data: data, encoding: .utf8) : nil)
                ?? String(decoding: data.prefix(8_192), as: UTF8.self).nilIfContainsBinaryNUL()
            if let text = textSlice {
                store.addAttachment(FileAttachment(
                    name: url.lastPathComponent,
                    mimeType: ext == "json" ? "application/json" : "text/plain",
                    textPreview: text.truncatedToUTF8Bytes(4_000),
                    byteCount: data.count
                ))
                return
            }
            let maybeImg = FileAttachment.prepareImage(data: data, name: url.lastPathComponent)
            if maybeImg.isImage && maybeImg.imageBase64 != nil {
                store.addAttachment(maybeImg)
                return
            }
            let mime = ext == "pdf" ? "application/pdf" : "application/octet-stream"
            let binB64 = data.count <= 20_000 ? data.base64EncodedString() : nil
            store.addAttachment(FileAttachment(
                name: url.lastPathComponent,
                mimeType: mime,
                textPreview: nil,
                byteCount: data.count,
                binaryBase64: binB64
            ))
        }
    }

    // MARK: - Banners

    private func toastBanner(_ text: String) -> some View {
        HStack(spacing: 8) {
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 12))
                .foregroundStyle(OATheme.statusReady)
            Text(text)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(OATheme.textPrimary)
            Spacer()
            Button {
                store.statusToast = nil
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(OATheme.textMuted)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 8)
        .background(OATheme.surface)
        .overlay(alignment: .bottom) { Divider().overlay(OATheme.hairline) }
    }

    private func errorBanner(_ text: String) -> some View {
        HStack(spacing: 10) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 13))
                .foregroundStyle(OATheme.statusWarn)
            Text(text)
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textPrimary)
                .lineLimit(3)
            Spacer()
            Button {
                store.bannerError = nil
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(OATheme.textSecondary)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(OATheme.surface)
        .overlay(alignment: .bottom) { Divider().overlay(OATheme.hairline) }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("chat.errorBanner")
    }

    private func queuedBanner(_ text: String) -> some View {
        HStack(spacing: 8) {
            Image(systemName: "clock.arrow.circlepath")
                .font(.system(size: 12))
                .foregroundStyle(OATheme.textSecondary)
            Text("Queued: \(text)")
                .font(.system(size: 12))
                .foregroundStyle(OATheme.textPrimary)
                .lineLimit(1)
            Spacer()
            Button {
                store.clearQueuedDraft()
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(OATheme.textMuted)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
    }

    private var slashCommandMenu: some View {
        VStack(alignment: .leading, spacing: 2) {
            ForEach(store.matchingSlashCommands) { cmd in
                Button {
                    store.applySlashCommand(cmd)
                } label: {
                    HStack(spacing: 10) {
                        Text(cmd.command)
                            .font(.system(size: 13, weight: .semibold, design: .monospaced))
                            .foregroundStyle(OATheme.textPrimary)
                        Text(cmd.subtitle)
                            .font(.system(size: 12))
                            .foregroundStyle(OATheme.textSecondary)
                            .lineLimit(1)
                        Spacer()
                    }
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        .accessibilityIdentifier("chat.slashMenu")
    }

    private var mentionMenu: some View {
        VStack(alignment: .leading, spacing: 2) {
            ForEach(store.matchingMentions) { candidate in
                Button {
                    store.applyMention(candidate)
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: candidate.icon)
                            .font(.system(size: 11))
                            .foregroundStyle(OATheme.textSecondary)
                        Text(candidate.label)
                            .font(.system(size: 12.5, weight: .medium, design: .monospaced))
                            .foregroundStyle(OATheme.textPrimary)
                            .lineLimit(1)
                        Spacer()
                        if !candidate.subtitle.isEmpty {
                            Text(candidate.subtitle)
                                .font(.system(size: 10.5))
                                .foregroundStyle(OATheme.textMuted)
                                .lineLimit(1)
                        }
                    }
                    .padding(.horizontal, 12)
                    .padding(.vertical, 8)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        .accessibilityIdentifier("chat.folderMenu")
    }

    private var inlineCommentsChip: some View {
        HStack(spacing: 8) {
            Image(systemName: "text.bubble.fill")
                .font(.system(size: 11))
                .foregroundStyle(OATheme.statusWarn)
            Text("\(store.inlineReviewComments.count) review comment\(store.inlineReviewComments.count == 1 ? "" : "s") attached")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(OATheme.textPrimary)
            Spacer()
            Button("Inspect") {
                store.isWorkspacePresented = true
            }
            .font(.system(size: 11, weight: .semibold))
            .foregroundStyle(OATheme.textSecondary)
            Button {
                store.inlineReviewComments = []
            } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(OATheme.textMuted)
            }
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
    }

    // MARK: - Empty states

    private var tailscaleOnboardingState: some View {
        ScrollView {
            VStack(spacing: 18) {
                Spacer(minLength: 44)
                AntigravityMark(size: 36, color: OATheme.textPrimary)

                VStack(spacing: 8) {
                    Text("Antigravity over Tailscale")
                        .font(.system(size: 20, weight: .semibold))
                        .foregroundStyle(OATheme.textPrimary)
                    Text("Connect directly to Antigravity running on your Mac over your private Tailscale network — no cloud relay or account required.")
                        .font(.system(size: 14))
                        .foregroundStyle(OATheme.textSecondary)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 320)
                }

                VStack(alignment: .leading, spacing: 10) {
                    stepRow(number: "1", text: "Sign into Tailscale on this device and on your Mac.")
                    stepRow(number: "2", text: "Keep OpenAgents macOS or coder host running on port 47109.")
                    stepRow(number: "3", text: "Tap Connect Computer to pair directly over Tailscale in one tap.")
                }
                .padding(14)
                .frame(maxWidth: 340, alignment: .leading)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))

                Button {
                    store.isComputersPresented = true
                } label: {
                    HStack(spacing: 8) {
                        Image(systemName: "desktopcomputer")
                            .font(.system(size: 14, weight: .semibold))
                        Text("Connect Computer")
                            .font(.system(size: 15, weight: .semibold))
                    }
                    .foregroundStyle(Color.black)
                    .padding(.horizontal, 20)
                    .padding(.vertical, 11)
                    .background(Color.white, in: Capsule())
                }
                .accessibilityIdentifier("chat.connectTailscale")

                Spacer(minLength: 32)
            }
            .padding(.horizontal, 20)
        }
    }

    private func stepRow(number: String, text: String) -> some View {
        HStack(alignment: .top, spacing: 10) {
            Text(number)
                .font(.system(size: 11, weight: .bold, design: .monospaced))
                .foregroundStyle(OATheme.textPrimary)
                .frame(width: 20, height: 20)
                .background(OATheme.elevated, in: Circle())
                .overlay(Circle().stroke(OATheme.hairline, lineWidth: 1))
            Text(text)
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textSecondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private var readyEmptyChatState: some View {
        VStack(spacing: 14) {
            Spacer()
            AntigravityMark(size: 30, color: OATheme.textPrimary)
            Text(store.activeComputer?.name ?? "Antigravity")
                .font(.system(size: 18, weight: .semibold))
                .foregroundStyle(OATheme.textPrimary)
            Text(store.workspaceName.isEmpty
                 ? "Ready for a new Antigravity task over Tailscale."
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
                        Image(systemName: "folder")
                            .font(.system(size: 12))
                        Text(store.selectedFolder.isEmpty ? "Workspace root" : store.selectedFolder)
                            .font(.system(size: 12, weight: .medium, design: .monospaced))
                        Image(systemName: "chevron.down")
                            .font(.system(size: 9))
                    }
                    .foregroundStyle(OATheme.textSecondary)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .background(OATheme.surface, in: Capsule())
                    .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                }
            }
            Spacer()
        }
        .padding(.horizontal, 24)
    }

    // MARK: - Transcript

    private var transcriptScrollView: some View {
        ScrollViewReader { proxy in
            ZStack(alignment: .bottomTrailing) {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 14) {
                        if store.hasOlderMessages {
                            Button {
                                Task { await store.loadOlderMessages() }
                            } label: {
                                HStack(spacing: 6) {
                                    if store.isLoadingOlderMessages {
                                        ProgressView().controlSize(.mini).tint(OATheme.textSecondary)
                                    }
                                    Text("Load earlier messages")
                                        .font(.system(size: 12, weight: .medium))
                                }
                                .foregroundStyle(OATheme.textSecondary)
                                .frame(maxWidth: .infinity)
                                .padding(.vertical, 8)
                                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                                .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                            }
                            .buttonStyle(.plain)
                            .disabled(store.isLoadingOlderMessages)
                        }
                        if let note = store.activeSession?.note, !note.isEmpty {
                            noticeCard(text: note, kind: "note")
                        }
                        ForEach(store.items) { item in
                            TranscriptRowView(
                                item: item,
                                localImageLookup: { store.localImageData(for: $0) },
                                onOpenSubagent: { sub in
                                    store.openSubagent(sub)
                                },
                                onOpenFileLink: { path, line in
                                    store.openFileFromLink(line.map { "\(path)#L\($0)" } ?? path)
                                },
                                onEditAndResend: { msg in
                                    editingMessageDraft = ParsedUserMessage.parse(msg.text).text
                                    editingMessageItem = msg
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
                            .onAppear { isPinnedToBottom = true }
                            .onDisappear { isPinnedToBottom = false }
                    }
                    .padding(.horizontal, 16)
                    .padding(.vertical, 16)
                }
                .scrollDismissesKeyboard(.interactively)
                .onChange(of: store.streamingContentFingerprint) { _, _ in
                    guard isPinnedToBottom else { return }
                    withAnimation(.easeOut(duration: 0.16)) {
                        proxy.scrollTo("bottom-anchor", anchor: .bottom)
                    }
                }
                .onChange(of: store.activeSessionID) { _, _ in
                    isPinnedToBottom = true
                }

                if !isPinnedToBottom {
                    Button {
                        isPinnedToBottom = true
                        withAnimation(.easeOut(duration: 0.2)) {
                            proxy.scrollTo("bottom-anchor", anchor: .bottom)
                        }
                    } label: {
                        Image(systemName: "arrow.down")
                            .font(.system(size: 12, weight: .bold))
                            .foregroundStyle(OATheme.textPrimary)
                            .frame(width: 32, height: 32)
                            .background(OATheme.elevated, in: Circle())
                            .overlay(Circle().stroke(OATheme.hairlineStrong, lineWidth: 1))
                    }
                    .buttonStyle(.plain)
                    .padding(14)
                    .accessibilityLabel("Jump to bottom")
                }
            }
        }
    }

    private func noticeCard(text: String, kind: String) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: kind == "note" ? "info.circle" : "exclamationmark.circle")
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textSecondary)
            Text(text)
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textSecondary)
        }
        .padding(10)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
    }

    // MARK: - Composer

    private var composerBar: some View {
        VStack(spacing: 8) {
            if !store.pendingAttachments.isEmpty || isLoadingAttachment {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 5) {
                        if isLoadingAttachment {
                            HStack(spacing: 6) {
                                ProgressView().controlSize(.mini).tint(OATheme.textSecondary)
                                Text("Preparing photo…")
                                    .font(.system(size: 11, design: .monospaced))
                            }
                            .foregroundStyle(OATheme.textSecondary)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 5)
                            .background(OATheme.elevated, in: Capsule())
                            .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                        }
                        ForEach(store.pendingAttachments) { att in
                            HStack(spacing: 6) {
                                if let imgData = att.imageData {
                                    InlineCGImageView(data: imgData, maxHeight: 22)
                                        .frame(width: 22, height: 22)
                                        .clipped()
                                        .clipShape(RoundedRectangle(cornerRadius: 5, style: .continuous))
                                } else {
                                    Image(systemName: att.textPreview != nil ? "doc.text" : "photo")
                                        .font(.system(size: 10))
                                }
                                Text(att.name)
                                    .font(.system(size: 11, design: .monospaced))
                                    .lineLimit(1)
                                Button {
                                    store.removeAttachment(att)
                                } label: {
                                    Image(systemName: "xmark.circle.fill")
                                        .font(.system(size: 11))
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

            TextField("Ask Antigravity or type /…", text: $store.draftText, axis: .vertical)
                .lineLimit(1...6)
                .font(.system(size: 15))
                .foregroundStyle(OATheme.textPrimary)
                .scrollIndicators(.hidden)
                .focused($composerFocused)
                .frame(minHeight: 36, alignment: .center)
                .padding(.horizontal, 12)
                .padding(.top, store.pendingAttachments.isEmpty ? 6 : 2)
                .padding(.bottom, 2)
                .contentShape(Rectangle())
                .onTapGesture { composerFocused = true }
                .accessibilityIdentifier("chat.composer")

            HStack(spacing: 6) {
                // + Attachment & Slash Menu
                Menu {
                    Button {
                        showFileImporter = true
                    } label: {
                        Label("Attach File", systemImage: "doc.badge.plus")
                    }
                    Button {
                        showPhotoPicker = true
                    } label: {
                        Label("Attach Photo", systemImage: "photo")
                    }
                    Divider()
                    Button {
                        store.draftText = "/"
                        composerFocused = true
                    } label: {
                        Label("Slash Commands", systemImage: "slash.circle")
                    }
                    Button {
                        store.isSideQuestionPresented = true
                    } label: {
                        Label("/btw Side Question", systemImage: "bubble.left.and.exclamationmark.bubble.right")
                    }
                    if store.activeSession != nil {
                        Button {
                            Task { await store.forkActiveSession() }
                        } label: {
                            Label("Fork Session", systemImage: "arrow.triangle.branch")
                        }
                    }
                } label: {
                    Image(systemName: "plus")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(OATheme.textSecondary)
                        .frame(width: 28, height: 28)
                        .background(OATheme.elevated, in: Circle())
                        .overlay(Circle().stroke(OATheme.hairline, lineWidth: 1))
                }
                .accessibilityIdentifier("chat.attachButton")

                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 5) {
                        // Antigravity Model Picker (grouped by family, full dynamic ACP catalog; #14005)
                        Menu {
                            ForEach(store.groupedAvailableModels, id: \.family) { group in
                                Section(group.family) {
                                    ForEach(group.models) { model in
                                        Button {
                                            store.chooseModel(model.id)
                                        } label: {
                                            if model.id == store.selectedModelID {
                                                Label(model.label, systemImage: "checkmark")
                                            } else {
                                                Text(model.label)
                                            }
                                        }
                                    }
                                }
                            }
                        } label: {
                            chipLabel(icon: nil, text: store.selectedModelOption.label, showMark: true)
                        }
                        .accessibilityIdentifier("chat.modelPicker")

                        // Thinking Effort Picker (hidden when selected model disables --effort, e.g. Orchestrator; #14005)
                        if store.selectedModelSupportsThinking {
                            Menu {
                                ForEach(store.supportedEffortsForSelectedModel) { effort in
                                    Button {
                                        store.chooseEffort(effort)
                                    } label: {
                                        if effort == store.selectedEffort {
                                            Label(effort.label, systemImage: "checkmark")
                                        } else {
                                            Text(effort.label)
                                        }
                                    }
                                }
                            } label: {
                                chipLabel(icon: "sparkles", text: store.selectedEffort.label, showMark: false)
                            }
                            .accessibilityIdentifier("chat.effortPicker")
                        }

                        // Review Mode Chip (Auto / Ask / Plan)
                        Menu {
                            ForEach(ReviewMode.allCases) { mode in
                                Button {
                                    store.chooseReviewMode(mode)
                                } label: {
                                    if mode == store.reviewMode {
                                        Label("\(mode.label) — \(mode.description)", systemImage: "checkmark")
                                    } else {
                                        Text("\(mode.label) — \(mode.description)")
                                    }
                                }
                            }
                        } label: {
                            chipLabel(icon: store.reviewMode.icon, text: store.reviewMode.label, showMark: false)
                        }
                        .accessibilityIdentifier("chat.reviewMode")
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)

                // Voice Dictation Toggle (Speech / AVFoundation)
                Button {
                    UIImpactFeedbackGenerator(style: .light).impactOccurred()
                    dictation.toggle(existingText: store.draftText) { updated in
                        store.draftText = updated
                    }
                } label: {
                    Image(systemName: dictation.isListening ? "waveform.circle.fill" : "mic")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(dictation.isListening ? Color.black : OATheme.textSecondary)
                        .frame(width: 28, height: 28)
                        .background(dictation.isListening ? Color.white : OATheme.elevated, in: Circle())
                        .overlay(Circle().stroke(OATheme.hairline, lineWidth: 1))
                }
                .accessibilityIdentifier("chat.micButton")
                .accessibilityLabel(dictation.isListening ? "Stop dictation" : "Voice dictation")

                actionButton
            }
            .padding(.horizontal, 10)
            .padding(.bottom, 10)
        }
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(OATheme.background)
    }

    private func chipLabel(icon: String?, text: String, showMark: Bool) -> some View {
        HStack(spacing: 3.5) {
            if showMark {
                AntigravityMark(size: 9.5, color: OATheme.textSecondary)
            } else if let icon {
                Image(systemName: icon)
                    .font(.system(size: 9.5, weight: .medium))
                    .foregroundStyle(OATheme.textSecondary)
            }
            Text(text)
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(OATheme.textSecondary)
                .lineLimit(1)
        }
        .padding(.horizontal, 7)
        .padding(.vertical, 5)
        .background(OATheme.elevated, in: Capsule())
        .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
    }

    @ViewBuilder
    private var actionButton: some View {
        let hasPayload = !store.draftText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !store.pendingAttachments.isEmpty
        let isRunning = store.activeSession?.busy == true

        if isRunning && !hasPayload {
            Button {
                UIImpactFeedbackGenerator(style: .medium).impactOccurred()
                Task { await store.stopActiveTurn() }
            } label: {
                Image(systemName: "stop.fill")
                    .font(.system(size: 12, weight: .bold))
                    .foregroundStyle(Color.black)
                    .frame(width: 30, height: 30)
                    .background(Color.white, in: Circle())
            }
            .accessibilityIdentifier("chat.stop")
            .accessibilityLabel("Stop turn")
        } else {
            HStack(spacing: 6) {
                if isRunning && hasPayload {
                    Button {
                        UIImpactFeedbackGenerator(style: .light).impactOccurred()
                        store.queueCurrentDraft()
                    } label: {
                        Image(systemName: "clock.arrow.circlepath")
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(OATheme.textPrimary)
                            .frame(width: 28, height: 28)
                            .background(OATheme.elevated, in: Circle())
                            .overlay(Circle().stroke(OATheme.hairline, lineWidth: 1))
                    }
                    .accessibilityIdentifier("chat.queue")
                    .accessibilityLabel("Queue follow-up")
                }

                Button {
                    if dictation.isListening { dictation.stop() }
                    composerFocused = false
                    UIImpactFeedbackGenerator(style: .light).impactOccurred()
                    Task { await store.sendMessage() }
                } label: {
                    Group {
                        if store.isSending {
                            ProgressView()
                                .controlSize(.mini)
                                .tint(Color.black)
                        } else {
                            Image(systemName: "arrow.up")
                                .font(.system(size: 13, weight: .bold))
                                .foregroundStyle(hasPayload ? Color.black : OATheme.textMuted)
                        }
                    }
                    .frame(width: 30, height: 30)
                    .background(hasPayload ? Color.white : OATheme.elevated, in: Circle())
                }
                .disabled(!hasPayload || store.isSending)
                .accessibilityIdentifier("chat.send")
                .accessibilityLabel(isRunning ? "Steer Antigravity" : "Send message")
            }
        }
    }
}
