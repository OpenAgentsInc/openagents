// Firm reason: 6 (iOS SwiftUI chat transcript + interactive approval banner + attachments + / slash commands + queued follow-ups + native Antigravity composer).
import PhotosUI
import SwiftUI
import UIKit
import UniformTypeIdentifiers

struct ChatView: View {
    @ObservedObject var store: AppStore
    @FocusState private var composerFocused: Bool
    @State private var showFileImporter = false
    @State private var showPhotoPicker = false
    @State private var selectedPhotoItem: PhotosPickerItem?

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
                ApprovalBannerView(interaction: interaction) { allow, note in
                    UIImpactFeedbackGenerator(style: .medium).impactOccurred()
                    Task { await store.respondToInteraction(allow: allow, note: note) }
                }
                .padding(.horizontal, 12)
                .padding(.bottom, 8)
            }

            if !store.matchingSlashCommands.isEmpty {
                slashCommandMenu
                    .padding(.horizontal, 12)
                    .padding(.bottom, 6)
            }

            if let queued = store.queuedDraft, !queued.isEmpty {
                queuedBanner(queued)
                    .padding(.horizontal, 12)
                    .padding(.bottom, 6)
            }

            Divider().overlay(OATheme.hairline)
            composerBar
        }
        .background(OATheme.background)
        .photosPicker(isPresented: $showPhotoPicker, selection: $selectedPhotoItem, matching: .images)
        .onChange(of: selectedPhotoItem) { _, newItem in
            guard let newItem else { return }
            Task {
                if let data = try? await newItem.loadTransferable(type: Data.self) {
                    let att = FileAttachment(
                        name: "photo-\(Int(Date().timeIntervalSince1970) % 10000).jpg",
                        mimeType: "image/jpeg",
                        textPreview: nil,
                        byteCount: data.count
                    )
                    store.addAttachment(att)
                }
                selectedPhotoItem = nil
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
            let preview = String(data: data.prefix(4000), encoding: .utf8)
            let mime = preview != nil ? "text/plain" : "application/octet-stream"
            store.addAttachment(FileAttachment(
                name: url.lastPathComponent,
                mimeType: mime,
                textPreview: preview,
                byteCount: data.count
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
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 14) {
                    if let note = store.activeSession?.note, !note.isEmpty {
                        noticeCard(text: note, kind: "note")
                    }
                    ForEach(store.items) { item in
                        TranscriptRowView(item: item) { sub in
                            store.openSubagent(sub)
                        }
                        .id(item.id)
                    }
                    if store.activeSession?.busy == true {
                        HStack(spacing: 8) {
                            ProgressView()
                                .controlSize(.small)
                                .tint(OATheme.textSecondary)
                            Text("Antigravity is working…")
                                .font(.system(size: 13))
                                .foregroundStyle(OATheme.textSecondary)
                        }
                        .padding(.top, 4)
                        .id("working-indicator")
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 16)
            }
            .scrollDismissesKeyboard(.interactively)
            .onChange(of: store.items.count) { _, _ in
                if let last = store.items.last {
                    withAnimation(.easeOut(duration: 0.2)) {
                        proxy.scrollTo(last.id, anchor: .bottom)
                    }
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
            if !store.pendingAttachments.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(store.pendingAttachments) { att in
                            HStack(spacing: 5) {
                                Image(systemName: att.textPreview != nil ? "doc.text" : "photo")
                                    .font(.system(size: 10))
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
                .focused($composerFocused)
                .padding(.horizontal, 12)
                .padding(.top, store.pendingAttachments.isEmpty ? 10 : 2)
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
                } label: {
                    Image(systemName: "plus")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(OATheme.textSecondary)
                        .frame(width: 28, height: 28)
                        .background(OATheme.elevated, in: Circle())
                        .overlay(Circle().stroke(OATheme.hairline, lineWidth: 1))
                }
                .accessibilityIdentifier("chat.attachButton")

                // Antigravity Model Picker
                Menu {
                    ForEach(store.availableModels) { model in
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
                } label: {
                    chipLabel(icon: nil, text: store.selectedModelOption.label, showMark: true)
                }
                .accessibilityIdentifier("chat.modelPicker")

                // Thinking Effort Picker
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

                Spacer(minLength: 4)

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
        HStack(spacing: 4) {
            if showMark {
                AntigravityMark(size: 10, color: OATheme.textSecondary)
            } else if let icon {
                Image(systemName: icon)
                    .font(.system(size: 10, weight: .medium))
                    .foregroundStyle(OATheme.textSecondary)
            }
            Text(text)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(OATheme.textSecondary)
                .lineLimit(1)
        }
        .padding(.horizontal, 8)
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
