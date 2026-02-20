import SwiftUI

struct ContentView: View {
    @StateObject private var model = CodexHandshakeViewModel()

    var body: some View {
        NavigationStack {
            Form {
                Section("OpenAgents") {
                    HStack {
                        Text("Environment")
                        Spacer()
                        Text(model.environmentHost)
                            .foregroundStyle(.secondary)
                    }

                    Text("Auth: \(authDescription(model.authState))")
                        .font(.footnote)

                    if model.isAuthenticated {
                        Button("Sign Out") {
                            model.signOut()
                        }
                    } else {
                        TextField("Email", text: $model.email)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled(true)
                            .keyboardType(.emailAddress)

                        TextField("Verification Code", text: $model.verificationCode)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled(true)
                            .keyboardType(.numberPad)

                        HStack {
                            Button("Send Code") {
                                Task {
                                    await model.sendEmailCode()
                                }
                            }

                            Button("Verify") {
                                Task {
                                    await model.verifyEmailCode()
                                }
                            }
                            .disabled(model.verificationCode.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                        }
                    }
                }

                Section("Worker") {
                    Button("Load Workers + Handshake") {
                        Task {
                            await model.refreshWorkers()
                        }
                    }
                    .disabled(!model.isAuthenticated)

                    if model.workers.isEmpty {
                        Text("No workers loaded")
                            .foregroundStyle(.secondary)
                    } else {
                        if let selectedWorkerID = model.selectedWorkerID {
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Active Worker")
                                Text("\(selectedWorkerID) (\(model.latestSnapshot?.status ?? "unknown"))")
                                    .foregroundStyle(.secondary)
                                    .font(.footnote)
                            }
                        }

                        if model.workers.count > 1 {
                            Text("Auto-selecting freshest running desktop worker (\(model.workers.count) candidates found).")
                                .font(.footnote)
                                .foregroundStyle(.secondary)
                        }

                        if let snapshot = model.latestSnapshot {
                            VStack(alignment: .leading, spacing: 4) {
                                Text("Snapshot: \(snapshot.status)")
                                Text("Latest seq: \(snapshot.latestSeq)")
                                    .foregroundStyle(.secondary)
                                    .font(.footnote)
                            }
                        }
                    }
                }

                Section("Handshake") {
                    Text("Device ID: \(model.deviceID)")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .textSelection(.enabled)

                    Text("Stream: \(streamDescription(model.streamState))")
                        .font(.footnote)

                    Text("Handshake: \(handshakeDescription(model.handshakeState))")
                        .font(.footnote)

                    HStack {
                        Button("Connect Stream") {
                            model.connectStream()
                        }
                        .disabled(!model.isAuthenticated)

                        Button("Disconnect") {
                            model.disconnectStream()
                        }
                    }

                    Button("Send Handshake") {
                        Task {
                            await model.sendHandshake()
                        }
                    }
                    .disabled(!model.isAuthenticated || model.selectedWorkerID == nil)
                }

                Section("Recent Events") {
                    if model.recentEvents.isEmpty {
                        Text("No events yet")
                            .foregroundStyle(.secondary)
                    } else {
                        ForEach(Array(model.recentEvents.enumerated()), id: \.offset) { _, event in
                            VStack(alignment: .leading, spacing: 4) {
                                Text(event.event)
                                    .font(.caption)
                                Text(event.rawData)
                                    .font(.caption2)
                                    .foregroundStyle(.secondary)
                                    .lineLimit(3)
                            }
                        }
                    }
                }

                if let status = model.statusMessage {
                    Section("Status") {
                        Text(status)
                    }
                }

                if let error = model.errorMessage {
                    Section("Error") {
                        Text(error)
                            .foregroundStyle(.red)
                    }
                }
            }
            .navigationTitle("Codex Handshake")
        }
    }

    private func authDescription(_ state: AuthState) -> String {
        switch state {
        case .signedOut:
            return "signed out"
        case .sendingCode:
            return "sending code"
        case .codeSent(let email):
            return "code sent to \(email)"
        case .verifying:
            return "verifying"
        case .authenticated(let email):
            if let email, !email.isEmpty {
                return "signed in as \(email)"
            }
            return "signed in"
        }
    }

    private func streamDescription(_ state: StreamState) -> String {
        switch state {
        case .idle:
            return "idle"
        case .connecting:
            return "connecting (waiting for first poll)"
        case .live:
            return "live"
        case .reconnecting:
            return "reconnecting (retrying)"
        }
    }

    private func handshakeDescription(_ state: HandshakeState) -> String {
        switch state {
        case .idle:
            return "idle"
        case .sending:
            return "sending"
        case .waitingAck(let handshakeID):
            return "waiting ack (\(handshakeID))"
        case .success(let handshakeID):
            return "success (\(handshakeID))"
        case .timedOut(let handshakeID):
            return "timed out (\(handshakeID))"
        case .failed(let message):
            return "failed (\(message))"
        }
    }
}

#Preview {
    ContentView()
}
