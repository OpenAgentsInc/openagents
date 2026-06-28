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
                        .disabled(minting)

                        VStack(alignment: .leading, spacing: 6) {
                            Text("…or paste a key")
                                .font(.footnote)
                                .foregroundStyle(.secondary)
                            TextField("oa_agent_…", text: $pastedKey)
                                .textInputAutocapitalization(.never)
                                .autocorrectionDisabled()
                                .font(.system(.body, design: .monospaced))
                            Button("Save pasted key") {
                                let trimmed = pastedKey.trimmingCharacters(in: .whitespacesAndNewlines)
                                guard !trimmed.isEmpty else { return }
                                KeychainStore.saveAPIKey(trimmed)
                                hasKey = true
                                pastedKey = ""
                            }
                            .disabled(pastedKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
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
                    Link(
                        "Read the full terms",
                        destination: URL(string: "https://openagents.com/api/public/free-tier-data-sharing")!
                    )
                    .font(.footnote)
                }

                Section("Apple FM local bridge") {
                    Text("Apple FM uses the local bridge health endpoint before inference. Base URL order: PROBE_APPLE_FM_BASE_URL, OPENAGENTS_APPLE_FM_BASE_URL, then http://127.0.0.1:11435.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                    Text("If the bridge reports unsupported, use an admitted Apple Silicon Mac with Apple Intelligence enabled and the packaged helper installed/executable.")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
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
        minting = true
        defer { minting = false }
        do {
            let token = try await KhalaClient.mintFreeKey()
            KeychainStore.saveAPIKey(token)
            hasKey = true
            mintedConfirmation = true
        } catch {
            errorText = (error as? LocalizedError)?.errorDescription ?? "Couldn't mint a key."
        }
    }
}
