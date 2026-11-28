// Firm reason: 6 (iOS pure-black SwiftUI Account & Passkey sheet for OpenAgents Cloud Chat and cross-device sync).
import SwiftUI

struct AccountView: View {
    @ObservedObject var account: MobileAccountController
    @ObservedObject var cloudChat: MobileCloudChat
    @Environment(\.dismiss) private var dismiss

    @State private var emailDraft = ""
    @State private var codeDraft = ""
    @State private var passwordDraft = ""
    @State private var showPasswordMode = false

    var body: some View {
        NavigationStack {
            List {
                if let err = account.errorMessage, !err.isEmpty {
                    Section {
                        Text(err)
                            .font(.system(size: 13))
                            .foregroundStyle(OATheme.statusError)
                            .listRowBackground(OATheme.surface)
                    }
                }
                if let msg = account.statusMessage, !msg.isEmpty {
                    Section {
                        Text(msg)
                            .font(.system(size: 13))
                            .foregroundStyle(OATheme.statusReady)
                            .listRowBackground(OATheme.surface)
                    }
                }

                if account.isLoggedIn {
                    signedInSections
                } else if !account.pendingOrgs.isEmpty {
                    orgSelectionSection
                } else if account.awaitingEmailCode {
                    emailCodeSection
                } else {
                    signInSections
                }
            }
            .scrollContentBackground(.hidden)
            .background(OATheme.background)
            .navigationTitle("Account & Cloud")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }
                        .accessibilityIdentifier("account.done")
                }
            }
        }
        .preferredColorScheme(.dark)
        .task {
            await account.refresh()
        }
    }

    @ViewBuilder
    private var signedInSections: some View {
        Section {
            HStack {
                VStack(alignment: .leading, spacing: 3) {
                    Text(account.email.isEmpty ? "OpenAgents Account" : account.email)
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(OATheme.textPrimary)
                    if !account.accountID.isEmpty {
                        Text(account.accountID)
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundStyle(OATheme.textMuted)
                    }
                }
                Spacer()
                Text("Logged In")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(Color.black)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 3)
                    .background(OATheme.statusReady, in: Capsule())
            }
            .listRowBackground(OATheme.surface)

            HStack {
                Text("Paid Credits")
                    .font(.system(size: 14))
                    .foregroundStyle(OATheme.textSecondary)
                Spacer()
                Text(account.formattedPaidCredits)
                    .font(.system(size: 14, weight: .semibold, design: .monospaced))
                    .foregroundStyle(OATheme.textPrimary)
            }
            .listRowBackground(OATheme.surface)

            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Promo Credits")
                        .font(.system(size: 14))
                        .foregroundStyle(OATheme.textSecondary)
                    if !account.formattedPromoExpiry.isEmpty {
                        Text("Expires \(account.formattedPromoExpiry)")
                            .font(.system(size: 11))
                            .foregroundStyle(OATheme.textMuted)
                    }
                }
                Spacer()
                Text(account.formattedPromoCredits)
                    .font(.system(size: 14, weight: .semibold, design: .monospaced))
                    .foregroundStyle(account.promoMicros > 0 ? OATheme.statusReady : OATheme.textSecondary)
            }
            .listRowBackground(OATheme.surface)

            HStack {
                Text("Cloud Chat Sync")
                    .font(.system(size: 14))
                    .foregroundStyle(OATheme.textSecondary)
                Spacer()
                let count = cloudChat.conversations.count
                let noun = count == 1 ? "chat" : "chats"
                Text(cloudChat.isStreamConnected ? "Live · \(count) \(noun)" : "\(count) synced \(noun)")
                    .font(.system(size: 13))
                    .foregroundStyle(cloudChat.isStreamConnected ? OATheme.statusReady : OATheme.textSecondary)
            }
            .listRowBackground(OATheme.surface)

            HStack {
                Text("GitHub")
                    .font(.system(size: 14))
                    .foregroundStyle(OATheme.textSecondary)
                Spacer()
                if account.githubConnected {
                    Text(account.formattedGitHubStatus)
                        .font(.system(size: 13, weight: .medium, design: .monospaced))
                        .foregroundStyle(OATheme.statusReady)
                } else {
                    Button("Connect GitHub") {
                        Task { await account.connectGitHub() }
                    }
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(Color.white)
                    .disabled(account.isBusy)
                }
            }
            .accessibilityIdentifier("account.github")
            .listRowBackground(OATheme.surface)
        } header: {
            Text("OpenAgents Cloud")
        }

        if account.organizations.count > 1 {
            Section {
                ForEach(account.organizations, id: \.id) { org in
                    Button {
                        Task { await account.switchOrganization(org) }
                    } label: {
                        HStack {
                            Text(org.name)
                                .font(.system(size: 14, weight: .medium))
                                .foregroundStyle(OATheme.textPrimary)
                            Spacer()
                            if account.activeOrgID == org.id {
                                Image(systemName: "checkmark")
                                    .font(.system(size: 13, weight: .semibold))
                                    .foregroundStyle(Color.white)
                            }
                        }
                    }
                    .disabled(account.isBusy)
                    .listRowBackground(OATheme.surface)
                }
            } header: {
                Text("Organization")
            }
        }

        Section {
            Button {
                Task { await account.addPasskey() }
            } label: {
                HStack {
                    Image(systemName: "person.badge.key.fill")
                    Text("Add Passkey on This Device")
                    Spacer()
                    if account.isBusy { ProgressView().controlSize(.small) }
                }
                .font(.system(size: 14, weight: .medium))
            }
            .disabled(account.isBusy)
            .accessibilityIdentifier("account.addPasskey")
            .listRowBackground(OATheme.surface)

            Button {
                Task {
                    await account.refresh()
                    await cloudChat.refreshConversations()
                }
            } label: {
                HStack {
                    Image(systemName: "arrow.clockwise")
                    Text("Refresh Account & Cloud Chats")
                }
                .font(.system(size: 14, weight: .medium))
            }
            .disabled(account.isBusy)
            .listRowBackground(OATheme.surface)
        } header: {
            Text("Security & Sync")
        }

        Section {
            Button(role: .destructive) {
                Task { await account.signOut() }
            } label: {
                Text("Log Out")
                    .font(.system(size: 14, weight: .semibold))
                    .frame(maxWidth: .infinity)
            }
            .disabled(account.isBusy)
            .accessibilityIdentifier("account.logout")
            .listRowBackground(OATheme.surface)
        }
    }

    @ViewBuilder
    private var signInSections: some View {
        Section {
            TextField(text: $emailDraft, prompt: Text(verbatim: "you@example.com").foregroundStyle(OATheme.textMuted)) {
                EmptyView()
            }
                .font(.system(size: 14))
                .foregroundStyle(OATheme.textPrimary)
                .keyboardType(.emailAddress)
                .textContentType(.emailAddress)
                .autocorrectionDisabled()
                .textInputAutocapitalization(.never)
                .accessibilityIdentifier("account.email")
                .listRowBackground(OATheme.surface)

            if showPasswordMode {
                SecureField("Password", text: $passwordDraft)
                    .font(.system(size: 14))
                    .textContentType(.password)
                    .listRowBackground(OATheme.surface)

                Button {
                    Task { await account.passwordSignIn(email: emailDraft, password: passwordDraft, register: false) }
                } label: {
                    HStack {
                        if account.isBusy { ProgressView().controlSize(.small) }
                        Text("Log in with Password")
                            .font(.system(size: 14, weight: .semibold))
                    }
                    .frame(maxWidth: .infinity)
                }
                .disabled(account.isBusy || emailDraft.trimmingCharacters(in: .whitespaces).isEmpty || passwordDraft.isEmpty)
                .listRowBackground(OATheme.elevated)
            } else {
                Button {
                    Task { await account.sendEmailCode(to: emailDraft) }
                } label: {
                    HStack {
                        if account.isBusy { ProgressView().controlSize(.small) }
                        Text("Send 6-Digit Email Code")
                            .font(.system(size: 14, weight: .semibold))
                    }
                    .frame(maxWidth: .infinity)
                }
                .disabled(account.isBusy || emailDraft.trimmingCharacters(in: .whitespaces).isEmpty)
                .accessibilityIdentifier("account.sendCode")
                .listRowBackground(OATheme.elevated)
            }

            if account.workosEnabled {
                Button(showPasswordMode ? "Use 6-Digit Email Code Instead" : "Use Email & Password") {
                    showPasswordMode.toggle()
                }
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textSecondary)
                .listRowBackground(OATheme.surface)
            }
        } header: {
            Text("Email")
        } footer: {
            Text("Log in to chat directly in OpenAgents Cloud and sync conversations across iPhone, Mac Desktop, and Web.")
        }

        if account.workosEnabled {
            Section {
                oauthButton(title: "Continue with Google", provider: "GoogleOAuth")
                oauthButton(title: "Continue with Apple", provider: "AppleOAuth")
            } header: {
                Text("Continue With")
            }
        }
    }

    private func oauthButton(title: String, provider: String) -> some View {
        Button {
            Task { await account.signInWithWorkOS(provider: provider, emailHint: emailDraft) }
        } label: {
            Text(title)
                .font(.system(size: 14, weight: .medium))
                .frame(maxWidth: .infinity)
        }
        .disabled(account.isBusy)
        .listRowBackground(OATheme.surface)
    }

    @ViewBuilder
    private var emailCodeSection: some View {
        Section {
            Text("Code sent to \(account.pendingEmailAddress)")
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textSecondary)
                .listRowBackground(OATheme.surface)

            TextField("6-digit code", text: $codeDraft)
                .font(.system(size: 16, design: .monospaced))
                .keyboardType(.numberPad)
                .textContentType(.oneTimeCode)
                .accessibilityIdentifier("account.code")
                .listRowBackground(OATheme.surface)

            Button {
                Task { await account.verifyEmailCode(codeDraft) }
            } label: {
                HStack {
                    if account.isBusy { ProgressView().controlSize(.small) }
                    Text("Verify & Log in")
                        .font(.system(size: 14, weight: .semibold))
                }
                .frame(maxWidth: .infinity)
            }
            .disabled(account.isBusy || codeDraft.trimmingCharacters(in: .whitespaces).isEmpty)
            .accessibilityIdentifier("account.verifyCode")
            .listRowBackground(OATheme.elevated)

            Button("Start Over") {
                codeDraft = ""
                account.cancelEmailFlow()
            }
            .font(.system(size: 13))
            .foregroundStyle(OATheme.textSecondary)
            .listRowBackground(OATheme.surface)
        } header: {
            Text("Verify Email")
        }
    }

    @ViewBuilder
    private var orgSelectionSection: some View {
        Section {
            ForEach(account.pendingOrgs, id: \.id) { org in
                Button {
                    Task { await account.selectOrganization(org.id) }
                } label: {
                    Text(org.name)
                        .font(.system(size: 14, weight: .medium))
                }
                .disabled(account.isBusy)
                .listRowBackground(OATheme.surface)
            }
            Button("Cancel") {
                account.cancelEmailFlow()
            }
            .font(.system(size: 13))
            .foregroundStyle(OATheme.textSecondary)
            .listRowBackground(OATheme.surface)
        } header: {
            Text("Choose Organization")
        }
    }
}
