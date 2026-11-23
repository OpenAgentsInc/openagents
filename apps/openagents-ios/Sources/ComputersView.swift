// Firm reason: 6 (iOS SwiftUI Tailscale Computers sheet with relay-free one-tap direct Tailscale pairing, MagicDNS/IP entry, and refusal diagnostics).
import SwiftUI

struct ComputersView: View {
    @ObservedObject var store: AppStore
    @Environment(\.dismiss) private var dismiss

    @State private var manualAddress = ""
    @State private var manualName = ""
    @State private var manualPort = String(AppTailnet.port)
    @State private var autoPair = true
    @State private var manualError: String?
    @State private var isAdding = false
    @State private var copiedKey = false

    var body: some View {
        NavigationStack {
            List {
                Section {
                    if store.computers.isEmpty {
                        VStack(alignment: .leading, spacing: 6) {
                            Text("No Tailscale computers found yet")
                                .font(.system(size: 14, weight: .medium))
                                .foregroundStyle(OATheme.textPrimary)
                            Text("Run OpenAgents on your Mac (or coder host serve --no-relay) on the same Tailscale account, or add its MagicDNS name / 100.x IP below.")
                                .font(.system(size: 13))
                                .foregroundStyle(OATheme.textSecondary)
                        }
                        .padding(.vertical, 6)
                        .listRowBackground(OATheme.surface)
                    } else {
                        ForEach(store.computers) { computer in
                            computerRow(computer)
                                .listRowBackground(OATheme.surface)
                        }
                    }
                } header: {
                    Text("Tailscale Computers")
                }

                Section {
                    TextField("mac.tailnet.ts.net or 100.x.y.z", text: $manualAddress)
                        .font(.system(size: 14, design: .monospaced))
                        .autocorrectionDisabled()
                        .textInputAutocapitalization(.never)
                        .keyboardType(.URL)
                        .accessibilityIdentifier("computers.addAddress")
                        .listRowBackground(OATheme.surface)

                    TextField("Display name (optional)", text: $manualName)
                        .font(.system(size: 14))
                        .accessibilityIdentifier("computers.addName")
                        .listRowBackground(OATheme.surface)

                    HStack {
                        Text("Port")
                            .font(.system(size: 14))
                            .foregroundStyle(OATheme.textSecondary)
                        Spacer()
                        TextField("47109", text: $manualPort)
                            .font(.system(size: 14, design: .monospaced))
                            .keyboardType(.numberPad)
                            .multilineTextAlignment(.trailing)
                            .frame(width: 90)
                    }
                    .listRowBackground(OATheme.surface)

                    Toggle("Pair directly over Tailscale (no relay)", isOn: $autoPair)
                        .font(.system(size: 14))
                        .tint(.white)
                        .listRowBackground(OATheme.surface)

                    if let manualError {
                        Text(manualError)
                            .font(.system(size: 12))
                            .foregroundStyle(OATheme.statusError)
                            .listRowBackground(OATheme.surface)
                    }

                    Button {
                        Task {
                            isAdding = true
                            manualError = nil
                            let port = UInt16(manualPort.trimmingCharacters(in: .whitespaces)) ?? AppTailnet.port
                            if let err = await store.addManualComputer(
                                address: manualAddress,
                                name: manualName,
                                port: port,
                                autoPair: autoPair
                            ) {
                                manualError = err
                            } else {
                                manualAddress = ""
                                manualName = ""
                            }
                            isAdding = false
                        }
                    } label: {
                        HStack {
                            if isAdding {
                                ProgressView().controlSize(.small)
                            }
                            Text(autoPair ? "Add & Pair over Tailscale" : "Add & Probe over Tailscale")
                                .font(.system(size: 14, weight: .semibold))
                        }
                        .frame(maxWidth: .infinity)
                    }
                    .disabled(manualAddress.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || isAdding)
                    .accessibilityIdentifier("computers.addButton")
                    .listRowBackground(OATheme.elevated)
                } header: {
                    Text("Add by MagicDNS or Tailscale IP")
                } footer: {
                    Text("Direct grants and NIP-44 E2E RPCs travel straight over your Tailscale WireGuard tunnel on port 47109. Public relays are disabled.")
                }

                Section {
                    HStack {
                        VStack(alignment: .leading, spacing: 4) {
                            Text("This Device's Key")
                                .font(.system(size: 13, weight: .medium))
                                .foregroundStyle(OATheme.textPrimary)
                            Text(shortDeviceKey)
                                .font(.system(size: 12, design: .monospaced))
                                .foregroundStyle(OATheme.textSecondary)
                        }
                        Spacer()
                        Button {
                            OAClipboard.copy(store.devicePublicKeyHex)
                            copiedKey = true
                            DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { copiedKey = false }
                        } label: {
                            Text(copiedKey ? "Copied" : "Copy")
                                .font(.system(size: 12, weight: .medium))
                        }
                    }
                    .listRowBackground(OATheme.surface)
                } header: {
                    Text("Local Device Identity")
                } footer: {
                    Text("Stored only in this device's Keychain (Coax secp256k1). Never synced or sent to a cloud account.")
                }
            }
            .scrollContentBackground(.hidden)
            .background(OATheme.background)
            .navigationTitle("Computers")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }
                        .accessibilityIdentifier("computers.done")
                }
            }
        }
        .preferredColorScheme(.dark)
    }

    private var shortDeviceKey: String {
        let key = store.devicePublicKeyHex
        guard key.count >= 16 else { return key.isEmpty ? "Unavailable" : key }
        return "\(key.prefix(8))…\(key.suffix(8))"
    }

    private func computerRow(_ computer: ComputerEntry) -> some View {
        let isActive = store.activeComputerID == computer.id
        let isPairing = store.isPairingHostID == computer.id
        let needsPair = computer.paired == nil || computer.paired?.isExpired == true

        return VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 10) {
                Circle()
                    .fill(dotColor(for: computer))
                    .frame(width: 8, height: 8)

                VStack(alignment: .leading, spacing: 2) {
                    HStack(spacing: 6) {
                        Text(computer.name)
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundStyle(OATheme.textPrimary)
                        if isActive {
                            Text("Active")
                                .font(.system(size: 10, weight: .semibold))
                                .foregroundStyle(Color.black)
                                .padding(.horizontal, 6)
                                .padding(.vertical, 2)
                                .background(Color.white, in: Capsule())
                        }
                    }
                    Text(computer.displayAddress)
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(OATheme.textSecondary)
                }

                Spacer()

                if let paired = computer.paired {
                    Text(paired.isExpired ? "Expired" : "Paired")
                        .font(.system(size: 11, weight: .medium))
                        .foregroundStyle(paired.isExpired ? OATheme.statusWarn : OATheme.statusReady)
                } else {
                    Text("Discovered")
                        .font(.system(size: 11, weight: .medium))
                        .foregroundStyle(OATheme.textMuted)
                }
            }

            Text(computer.reachability.label)
                .font(.system(size: 12))
                .foregroundStyle(reachabilityColor(for: computer.reachability))

            HStack(spacing: 8) {
                if needsPair && computer.canRequestDirectGrant {
                    Button {
                        Task {
                            if let err = await store.pairOverTailscale(computer) {
                                manualError = err
                            } else {
                                dismiss()
                            }
                        }
                    } label: {
                        HStack(spacing: 4) {
                            if isPairing { ProgressView().controlSize(.mini) }
                            Text(isPairing ? "Pairing…" : "Pair over Tailscale")
                        }
                        .font(.system(size: 12, weight: .semibold))
                    }
                    .buttonStyle(.borderedProminent)
                    .tint(.white)
                    .foregroundStyle(Color.black)
                    .disabled(isPairing)
                    .accessibilityIdentifier("computers.pairButton")
                } else {
                    Button("Select") {
                        store.selectComputer(computer)
                        dismiss()
                    }
                    .font(.system(size: 12, weight: .semibold))
                    .buttonStyle(.bordered)
                    .tint(.white)
                }

                Button("Probe") {
                    Task { await store.probeComputer(computer) }
                }
                .font(.system(size: 12, weight: .medium))
                .buttonStyle(.bordered)
                .tint(OATheme.textSecondary)

                Spacer()

                Button(role: .destructive) {
                    store.forgetComputer(computer)
                } label: {
                    Image(systemName: "trash")
                        .font(.system(size: 12))
                }
                .buttonStyle(.borderless)
            }
        }
        .padding(.vertical, 6)
    }

    private func dotColor(for computer: ComputerEntry) -> Color {
        switch computer.reachability {
        case .reachable: return OATheme.statusReady
        case .checking: return OATheme.statusWarn
        case .refused, .unreachable: return OATheme.statusError
        case .unknown: return computer.canSendRPC ? OATheme.statusWarn : OATheme.textMuted
        }
    }

    private func reachabilityColor(for state: ReachabilityState) -> Color {
        switch state {
        case .reachable: return OATheme.statusReady
        case .checking: return OATheme.statusWarn
        case .refused, .unreachable: return OATheme.statusError
        case .unknown: return OATheme.textMuted
        }
    }
}
