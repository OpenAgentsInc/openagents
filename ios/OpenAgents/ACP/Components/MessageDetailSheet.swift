import SwiftUI
import OpenAgentsCore

/// Modular detail sheet for displaying full message content and raw JSON
struct MessageDetailSheet: View {
    let message: MessageDetail
    @Binding var isPresented: Bool

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    // Full message text
                    Text(message.text)
                        .font(OAFonts.ui(.body, 15))
                        .foregroundStyle(OATheme.Colors.textPrimary)
                        .textSelection(.enabled)

                    if let json = message.rawJSON {
                        Divider().opacity(0.15)

                        // Raw JSON dump
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Raw JSON")
                                .font(OAFonts.ui(.caption, 12))
                                .foregroundStyle(OATheme.Colors.textSecondary)

                            Text(json)
                                .font(.system(.caption, design: .monospaced))
                                .foregroundStyle(OATheme.Colors.textSecondary)
                                .textSelection(.enabled)
                        }
                    }
                }
                .padding(14)
            }
            .navigationTitle("Message")
            .toolbar {
                #if os(iOS)
                ToolbarItem(placement: .topBarLeading) {
                    Button("Close") { isPresented = false }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    HStack(spacing: 12) {
                        Button("Copy Text") {
                            setClipboard(message.text)
                        }
                        if let json = message.rawJSON {
                            Button("Copy JSON") {
                                setClipboard(json)
                            }
                        }
                    }
                }
                #else
                ToolbarItem(placement: .navigation) {
                    Button("Close") { isPresented = false }
                }
                ToolbarItem {
                    HStack(spacing: 12) {
                        Button("Copy Text") {
                            setClipboard(message.text)
                        }
                        if let json = message.rawJSON {
                            Button("Copy JSON") {
                                setClipboard(json)
                            }
                        }
                    }
                }
                #endif
            }
        }
    }

    private func setClipboard(_ text: String) {
        #if os(iOS)
        UIPasteboard.general.string = text
        #elseif os(macOS)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(text, forType: .string)
        #endif
    }
}

/// Data model for message detail
struct MessageDetail: Identifiable, Equatable {
    let id: String
    let text: String
    let rawJSON: String?

    init(id: String = UUID().uuidString, text: String, rawJSON: String? = nil) {
        self.id = id
        self.text = text
        self.rawJSON = rawJSON
    }
}
