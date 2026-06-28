import SwiftUI

/// Key management: mint a free key or paste an existing one. Shows the honest
/// free-tier data-sharing disclosure at mint time.
struct SettingsView: View {
    @Environment(\.dismiss) private var dismiss
    @Binding var hasKey: Bool

    @State private var pastedKey: String = ""
    @State private var minting = false
    @State private var errorText: String?
    @State private var mintedConfirmation = false
    @State private var acceptedFreeTierDisclosure = FreeTierDisclosureStore.hasAccepted

    var body: some View {
        NavigationStack {
            Form {
                Section("Khala API key") {
                    if hasKey {
                        Label("A key is stored in the Keychain.", systemImage: "key.fill")
                            .foregroundStyle(.green)
                        Button("Remove stored key", role: .destructive) {
                            KeychainStore.deleteAPIKey()
                            hasKey = false
                        }
                    } else {
                        Button {
                            Task { await mint() }
                        } label: {
                            HStack {
                                Text("Mint a free key")
                                if minting { ProgressView().padding(.leading, 6) }
                            }
                        }
                        .disabled(minting || !acceptedFreeTierDisclosure)

                        VStack(alignment: .leading, spacing: 6) {
                            Text("…or paste a key")
                                .font(.footnote)
                                .foregroundStyle(.secondary)
                            SecureField("oa_agent_…", text: $pastedKey)
                                .textInputAutocapitalization(.never)
                                .autocorrectionDisabled()
                                .font(.system(.body, design: .monospaced))
                            Button("Save pasted key") {
                                let trimmed = pastedKey.trimmingCharacters(in: .whitespacesAndNewlines)
                                guard !trimmed.isEmpty else { return }
                                guard acceptedFreeTierDisclosure || !FreeTierDisclosureStore.requiresDisclosure(for: trimmed) else {
                                    errorText = "Acknowledge the free-tier disclosure before saving this key."
                                    return
                                }
                                if acceptedFreeTierDisclosure {
                                    FreeTierDisclosureStore.accept()
                                }
                                guard KeychainStore.saveAPIKey(trimmed) else {
                                    errorText = "Couldn't save the key in Keychain."
                                    return
                                }
                                hasKey = true
                                pastedKey = ""
                                errorText = nil
                            }
                            .disabled(
                                pastedKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                                    || (FreeTierDisclosureStore.requiresDisclosure(for: pastedKey) && !acceptedFreeTierDisclosure)
                            )
                        }
                    }

                    if let errorText {
                        Text(errorText).foregroundStyle(.red).font(.footnote)
                    }
                    if mintedConfirmation {
                        Text("Free key minted and stored.")
                            .foregroundStyle(.green).font(.footnote)
                    }
                }

                Section("Free-tier data sharing") {
                    Text(
                        "The free Khala API is captured by default: your traffic "
                        + "becomes a redacted, private-by-default trace that may be "
                        + "used to improve and train OpenAgents models, and the tokens "
                        + "count on the public served-token counter. Capture grants no "
                        + "payout. Pay for privacy (or run confidential compute) to opt "
                        + "out of capture."
                    )
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    Toggle(isOn: $acceptedFreeTierDisclosure) {
                        Text("I understand this disclosure before minting or using a free key.")
                    }
                    .font(.footnote)
                    .onChange(of: acceptedFreeTierDisclosure) { _, accepted in
                        if accepted {
                            FreeTierDisclosureStore.accept()
                        }
                    }
                    Link(
                        "Read the full terms",
                        destination: URL(string: "https://openagents.com/api/public/free-tier-data-sharing")!
                    )
                    .font(.footnote)
                }
            }
            .navigationTitle("Settings")
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
        }
    }

    private func mint() async {
        errorText = nil
        mintedConfirmation = false
        guard acceptedFreeTierDisclosure else {
            errorText = "Acknowledge the free-tier disclosure before minting a key."
            return
        }
        FreeTierDisclosureStore.accept()
        minting = true
        defer { minting = false }
        do {
            let token = try await KhalaClient.mintFreeKey()
            guard KeychainStore.saveAPIKey(token) else {
                errorText = "Key minted, but couldn't be saved in Keychain."
                return
            }
            hasKey = true
            mintedConfirmation = true
        } catch {
            errorText = (error as? LocalizedError)?.errorDescription ?? "Couldn't mint a key."
        }
    }

}
