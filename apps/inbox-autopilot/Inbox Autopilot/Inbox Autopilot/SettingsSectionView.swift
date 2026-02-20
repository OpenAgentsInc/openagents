import SwiftUI

struct SettingsSectionView: View {
    @EnvironmentObject private var model: AppModel
    @State private var allowedDomainsText = ""
    @State private var showDeleteCorpusConfirm = false
    @State private var showFactoryResetConfirm = false

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                authSection
                syncSection
                policySection
                templatesSection
                dataRetentionSection
                saveSection
            }
            .padding(14)
        }
        .onAppear {
            allowedDomainsText = model.settings.allowedRecipientDomains.joined(separator: ",")
            Task { await model.mineTemplateSuggestions() }
        }
        .alert("Delete local corpus?", isPresented: $showDeleteCorpusConfirm) {
            Button("Cancel", role: .cancel) {}
            Button("Delete", role: .destructive) {
                Task { await model.deleteLocalCorpus() }
            }
        } message: {
            Text("This removes local threads, messages, drafts, and events.")
        }
        .alert("Factory reset Inbox Autopilot?", isPresented: $showFactoryResetConfirm) {
            Button("Cancel", role: .cancel) {}
            Button("Reset", role: .destructive) {
                Task { await model.factoryReset() }
            }
        } message: {
            Text("This deletes all local data, OAuth credentials, and settings.")
        }
    }

    private var authSection: some View {
        GroupBox("Auth") {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    Label(model.gmailConnected ? "Gmail connected" : "Gmail not connected", systemImage: model.gmailConnected ? "checkmark.circle.fill" : "exclamationmark.circle")
                        .foregroundStyle(model.gmailConnected ? .green : .orange)
                    Spacer()
                    Button("Connect Gmail") {
                        Task { await model.connectGmail() }
                    }
                }

                HStack(spacing: 8) {
                    TextField("Manual OAuth code", text: $model.manualGmailCodeInput)
                        .textFieldStyle(.roundedBorder)
                    Button("Exchange Code") {
                        Task { await model.connectGmailWithManualCode() }
                    }
                }

                HStack {
                    Label(model.chatGPTConnected ? "ChatGPT connected" : "ChatGPT optional", systemImage: model.chatGPTConnected ? "checkmark.circle.fill" : "person.crop.circle.badge.exclam")
                        .foregroundStyle(model.chatGPTConnected ? .green : .secondary)
                }

                HStack(spacing: 8) {
                    SecureField("OpenAI API key", text: $model.chatGPTAPIKeyInput)
                        .textFieldStyle(.roundedBorder)
                    Button("Connect ChatGPT") {
                        Task { await model.connectChatGPT() }
                    }
                }
            }
            .padding(.top, 6)
        }
    }

    private var syncSection: some View {
        GroupBox("Sync") {
            VStack(alignment: .leading, spacing: 12) {
                Stepper("Backfill days: \(model.settings.backfillDays)", value: $model.settings.backfillDays, in: 30...365)
                Stepper("Sync interval: \(model.settings.syncIntervalSeconds)s", value: $model.settings.syncIntervalSeconds, in: 15...3600, step: 15)

                HStack(spacing: 8) {
                    Button("Run Backfill") {
                        Task { await model.runBackfill() }
                    }
                    .disabled(!model.gmailConnected)

                    Button("Sync now") {
                        Task { await model.syncNow() }
                    }
                    .disabled(!model.gmailConnected)
                }
            }
            .padding(.top, 6)
        }
    }

    private var policySection: some View {
        GroupBox("Privacy & Policy") {
            VStack(alignment: .leading, spacing: 12) {
                Picker("Privacy mode", selection: $model.settings.privacyMode) {
                    ForEach(PrivacyMode.allCases, id: \.self) { mode in
                        Text(mode.title).tag(mode)
                    }
                }
                .pickerStyle(.segmented)

                Picker("Attachment storage", selection: $model.settings.attachmentStorageMode) {
                    ForEach(AttachmentStorageMode.allCases, id: \.self) { mode in
                        Text(mode.title).tag(mode)
                    }
                }

                TextField("Allowed recipient domains (comma separated)", text: $allowedDomainsText)
                    .textFieldStyle(.roundedBorder)
            }
            .padding(.top, 6)
        }
    }

    private var templatesSection: some View {
        GroupBox("Templates") {
            VStack(alignment: .leading, spacing: 8) {
                Text("Signature")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                TextEditor(text: Binding(get: {
                    model.settings.signature ?? ""
                }, set: { value in
                    model.settings.signature = value.isEmpty ? nil : value
                }))
                .frame(minHeight: 70)
                .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color.gray.opacity(0.2), lineWidth: 1))

                Text("Scheduling template")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                TextEditor(text: Binding(get: {
                    model.settings.templateScheduling ?? ""
                }, set: { value in
                    model.settings.templateScheduling = value.isEmpty ? nil : value
                }))
                .frame(minHeight: 70)
                .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color.gray.opacity(0.2), lineWidth: 1))

                Text("Report delivery template")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                TextEditor(text: Binding(get: {
                    model.settings.templateReportDelivery ?? ""
                }, set: { value in
                    model.settings.templateReportDelivery = value.isEmpty ? nil : value
                }))
                .frame(minHeight: 70)
                .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color.gray.opacity(0.2), lineWidth: 1))

                Divider()
                    .padding(.vertical, 4)

                HStack {
                    Text("Template Mining")
                        .font(.headline)
                    Spacer()
                    Button("Refresh Suggestions") {
                        Task { await model.mineTemplateSuggestions() }
                    }
                }

                if model.templateSuggestions.isEmpty {
                    Text("No repeated templates found yet.")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                } else {
                    ForEach(model.templateSuggestions) { suggestion in
                        VStack(alignment: .leading, spacing: 4) {
                            HStack {
                                Text(suggestion.category.title)
                                    .font(.caption)
                                    .padding(.horizontal, 6)
                                    .padding(.vertical, 2)
                                    .background(Color.blue.opacity(0.15), in: Capsule())
                                Text("\(suggestion.occurrences)x")
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                            }
                            Text(suggestion.templateText)
                                .font(.caption)
                                .lineLimit(3)
                        }
                        .padding(8)
                        .background(Color.gray.opacity(0.08), in: RoundedRectangle(cornerRadius: 8))
                    }
                }
            }
            .padding(.top, 6)
        }
    }

    private var dataRetentionSection: some View {
        GroupBox("Data Retention") {
            VStack(alignment: .leading, spacing: 10) {
                Text("Delete local data or reset all configuration.")
                    .font(.caption)
                    .foregroundStyle(.secondary)

                HStack(spacing: 8) {
                    Button("Delete Local Corpus", role: .destructive) {
                        showDeleteCorpusConfirm = true
                    }

                    Button("Factory Reset", role: .destructive) {
                        showFactoryResetConfirm = true
                    }
                }
            }
            .padding(.top, 6)
        }
    }

    private var saveSection: some View {
        HStack {
            Button("Save Settings") {
                model.settings.allowedRecipientDomains = allowedDomainsText
                    .split(separator: ",")
                    .map { $0.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() }
                    .filter { !$0.isEmpty }
                Task { await model.saveSettings() }
            }

            Button("Reload") {
                Task {
                    await model.refreshSettings()
                    allowedDomainsText = model.settings.allowedRecipientDomains.joined(separator: ",")
                }
            }

            Spacer()
        }
    }
}
