import SwiftUI

private enum AppSection: String, CaseIterable, Identifiable {
    case inbox
    case approvals
    case settings
    case audit

    var id: String { rawValue }

    var title: String {
        switch self {
        case .inbox:
            return "Inbox"
        case .approvals:
            return "Approvals"
        case .settings:
            return "Settings"
        case .audit:
            return "Audit"
        }
    }

    var icon: String {
        switch self {
        case .inbox:
            return "tray.full"
        case .approvals:
            return "checkmark.seal"
        case .settings:
            return "gear"
        case .audit:
            return "clock.arrow.circlepath"
        }
    }
}

struct ContentView: View {
    @EnvironmentObject private var model: AppModel
    @State private var selectedSection: AppSection? = .inbox

    var body: some View {
        NavigationSplitView {
            List(AppSection.allCases, selection: $selectedSection) { section in
                Label(section.title, systemImage: section.icon)
                    .tag(section)
            }
            .navigationTitle("Inbox Autopilot")
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button {
                        Task { await model.refreshEverything() }
                    } label: {
                        Label("Refresh", systemImage: "arrow.clockwise")
                    }
                }
            }
        } detail: {
            VStack(alignment: .leading, spacing: 0) {
                statusStrip

                Divider()

                Group {
                    switch selectedSection {
                    case .inbox:
                        InboxSectionView(
                            onOpenApprovals: { selectedSection = .approvals },
                            onOpenAudit: { selectedSection = .audit }
                        )
                    case .approvals:
                        ApprovalsSectionView(onOpenThread: { threadID in
                            selectedSection = .inbox
                            Task { await model.openThread(id: threadID) }
                        })
                    case .settings:
                        SettingsSectionView()
                    case .audit:
                        AuditSectionView()
                    case nil:
                        Text("Select a section")
                            .foregroundStyle(.secondary)
                            .frame(maxWidth: .infinity, maxHeight: .infinity)
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .alert("Error", isPresented: Binding(get: {
            model.errorMessage != nil
        }, set: { isPresented in
            if !isPresented {
                model.errorMessage = nil
            }
        })) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(model.errorMessage ?? "Unknown error")
        }
        .sheet(isPresented: $model.needsOnboarding) {
            OnboardingSheetView()
                .environmentObject(model)
        }
    }

    private var statusStrip: some View {
        HStack(spacing: 12) {
            statusBadge(title: "Daemon", ok: model.daemonConnected)
            statusBadge(title: "Gmail", ok: model.gmailConnected)
            statusBadge(title: "ChatGPT", ok: model.chatGPTConnected)

            Spacer()

            if let notice = model.notice {
                Text(notice)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }

            if model.isBusy {
                ProgressView()
                    .controlSize(.small)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
    }

    private func statusBadge(title: String, ok: Bool) -> some View {
        HStack(spacing: 6) {
            Circle()
                .fill(ok ? Color.green : Color.orange)
                .frame(width: 8, height: 8)
            Text(title)
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 4)
        .background(Color.gray.opacity(0.12), in: Capsule())
    }
}

private struct OnboardingSheetView: View {
    @EnvironmentObject private var model: AppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Welcome to Inbox Autopilot")
                .font(.title2)
                .fontWeight(.semibold)

            Text("Connect Gmail, choose privacy mode, and select your default backfill range.")
                .foregroundStyle(.secondary)

            Picker("Privacy mode", selection: $model.settings.privacyMode) {
                ForEach(PrivacyMode.allCases, id: \.self) { mode in
                    Text(mode.title).tag(mode)
                }
            }
            .pickerStyle(.segmented)

            Picker("Backfill range", selection: $model.settings.backfillDays) {
                Text("90 days").tag(90)
                Text("12 months").tag(365)
            }
            .pickerStyle(.segmented)

            HStack(spacing: 8) {
                Button(model.gmailConnected ? "Gmail Connected" : "Connect Gmail") {
                    Task { await model.connectGmail() }
                }
                .disabled(model.gmailConnected)

                SecureField("Optional OpenAI API key", text: $model.chatGPTAPIKeyInput)
                    .textFieldStyle(.roundedBorder)

                Button(model.chatGPTConnected ? "ChatGPT Connected" : "Connect ChatGPT") {
                    Task { await model.connectChatGPT() }
                }
                .disabled(model.chatGPTConnected || model.chatGPTAPIKeyInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }

            HStack {
                Button("Save & Continue") {
                    Task {
                        await model.saveSettings()
                        if model.gmailConnected {
                            model.needsOnboarding = false
                        }
                    }
                }
                .keyboardShortcut(.defaultAction)
                .disabled(!model.gmailConnected)

                Button("Skip for now") {
                    model.needsOnboarding = false
                }
            }
        }
        .padding(20)
        .frame(width: 560)
    }
}
