import SwiftUI

struct ContentView: View {
    @StateObject private var model = CodexHandshakeViewModel()

    var body: some View {
        NavigationStack {
            Form {
                Section("Connection") {
                    TextField("API base URL", text: $model.apiBaseURL)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled(true)
                        .keyboardType(.URL)

                    SecureField("Auth token", text: $model.authToken)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled(true)

                    HStack {
                        Button("Save") {
                            model.saveConfiguration()
                            model.clearMessages()
                        }

                        Button("Load Workers") {
                            Task {
                                model.saveConfiguration()
                                await model.refreshWorkers()
                            }
                        }
                    }
                }

                Section("Worker") {
                    if model.workers.isEmpty {
                        Text("No workers loaded")
                            .foregroundStyle(.secondary)
                    } else {
                        Picker("Selected Worker", selection: $model.selectedWorkerID) {
                            ForEach(model.workers) { worker in
                                Text("\(worker.workerID) (\(worker.status))").tag(Optional(worker.workerID))
                            }
                        }
                        .pickerStyle(.navigationLink)

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
                        Button("Disconnect") {
                            model.disconnectStream()
                        }
                    }

                    Button("Send Handshake") {
                        Task {
                            await model.sendHandshake()
                        }
                    }
                    .disabled(model.selectedWorkerID == nil)
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

    private func streamDescription(_ state: StreamState) -> String {
        switch state {
        case .idle:
            return "idle"
        case .connecting:
            return "connecting"
        case .live:
            return "live"
        case .reconnecting:
            return "reconnecting"
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
