// Connect a computer (SCR-22) and Connected (SCR-23). Rust writes every word,
// decides what a scanned or pasted code is, and pairs; this view draws the
// camera and the paste field and hands Rust the text of one code.
import SwiftUI
import UIKit

enum ConnectCode {
    /// A code from OpenAgents on a computer: the connect code the desktop app
    /// shows, or a host invitation. Only the size and prefix are checked
    /// here; Rust parses it.
    static func accept(maximumBytes: Int) -> (String) throws -> String {
        { text in
            let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
            guard trimmed.hasPrefix("openagents-connect:") || trimmed.hasPrefix("coder-host:"),
                  trimmed.utf8.count <= maximumBytes else {
                throw QRInvitation.Failure.message("This isn't a code from OpenAgents on a computer. Scan the code the OpenAgents app on your computer shows.")
            }
            return trimmed
        }
    }
}

struct ConnectView: View {
    let screen: ConnectScreen
    @ObservedObject var bridge: MobileBridge
    @State private var pasting = false
    @State private var pasted = ""

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 16) {
                switch screen.stage {
                case "connected": connected
                case "connecting": connecting
                default: scanning
                }
                Spacer(minLength: 0)
            }
            .padding(16)
            .background(Color.black.ignoresSafeArea())
            .navigationTitle(screen.title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                if screen.stage != "connected" {
                    ToolbarItem(placement: .topBarLeading) {
                        Button("Close") { bridge.connectClose() }
                            .accessibilityIdentifier("connect-close")
                    }
                }
            }
        }
        .preferredColorScheme(.dark)
    }

    private var scanning: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let nearby = screen.nearby, !pasting {
                nearbyList(nearby)
            }
            if !pasting {
                InlineQRScanner(prompt: screen.prompt ?? "",
                                accept: ConnectCode.accept(maximumBytes: screen.max_bytes)) { code in
                    bridge.connectCode(code)
                }
                // A refused code shows its reason and scans again.
                .id(screen.notice ?? "")
            }
            if let notice = screen.notice {
                Text(notice).font(.callout).foregroundStyle(.secondary)
                    .accessibilityIdentifier("connect-notice")
            }
            if let paste = screen.paste {
                if pasting {
                    TextField(paste, text: $pasted, axis: .vertical)
                        .lineLimit(1...6)
                        .autocorrectionDisabled().textInputAutocapitalization(.never)
                        .padding(10)
                        .background(Color(white: 0.12), in: RoundedRectangle(cornerRadius: 10))
                        .accessibilityIdentifier("connect-paste-field")
                    HStack {
                        Button("Connect") {
                            let code = pasted
                            pasted = ""
                            bridge.connectCode(code)
                        }
                        .disabled(pasted.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                                  || pasted.utf8.count > screen.max_bytes)
                        .accessibilityIdentifier("connect-paste-send")
                        Spacer()
                        Button("Scan instead") { pasting = false; pasted = "" }
                    }
                } else {
                    Button(paste, systemImage: "doc.on.clipboard") {
                        pasted = UIPasteboard.general.string ?? ""
                        pasting = true
                    }
                    .accessibilityIdentifier("connect-paste")
                }
            }
            if let getApp = screen.get_app {
                Text(getApp).font(.footnote).foregroundStyle(.secondary)
            }
        }
    }

    /// Computers on this Wi-Fi. Anyone can name a computer anything, so a
    /// tap only starts a pairing the computer must approve after the codes
    /// match.
    private func nearbyList(_ nearby: ConnectScreen.Nearby) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(nearby.title).font(.headline)
            ForEach(nearby.computers, id: \.self) { row in
                Button {
                    bridge.connectNearby(row.id)
                } label: {
                    Label(row.label, systemImage: "desktopcomputer")
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(12)
                        .background(Color(white: 0.12), in: RoundedRectangle(cornerRadius: 10))
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("connect-nearby-\(row.label)")
            }
            if let empty = nearby.empty {
                Text(empty).font(.footnote).foregroundStyle(.secondary)
            }
        }
        .accessibilityIdentifier("connect-nearby")
    }

    private var connecting: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let code = screen.code {
                Text(code)
                    .font(.system(size: 44, weight: .bold, design: .monospaced))
                    .frame(maxWidth: .infinity)
                    .accessibilityIdentifier("connect-code")
            } else {
                ProgressView()
            }
            if let notice = screen.notice { Text(notice).font(.headline) }
        }
        .accessibilityIdentifier("connect-connecting")
    }

    private var connected: some View {
        VStack(spacing: 16) {
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 56))
                .foregroundStyle(.green)
                .accessibilityHidden(true)
            if let computer = screen.computer {
                Text(computer).font(.title2.bold()).accessibilityIdentifier("connect-computer")
            }
            if let notice = screen.notice {
                Text(notice).font(.callout).foregroundStyle(.secondary).multilineTextAlignment(.center)
            }
            if let done = screen.done {
                // The app tints white, so the label is black to show on it.
                Button(done) { bridge.connectClose() }
                    .buttonStyle(.borderedProminent).tint(.white).foregroundStyle(.black)
                    .accessibilityIdentifier("connect-done")
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.top, 32)
    }
}
