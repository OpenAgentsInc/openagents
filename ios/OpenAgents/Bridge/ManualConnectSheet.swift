import SwiftUI
import OpenAgentsCore
#if os(iOS)
import UIKit
#endif

struct ManualConnectSheet: View {
    @Environment(\.dismiss) var dismiss
    @State private var host: String = BridgeConfig.defaultHost
    @State private var port: String = String(BridgeConfig.defaultPort)
    var onConnect: (String, Int) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Manual Connect")
                .font(OAFonts.ui(.headline, 16))
                .foregroundStyle(OATheme.Colors.textPrimary)

            TextField("Host (e.g. 192.168.1.10)", text: $host)
                .textFieldStyle(.roundedBorder)
            TextField("Port", text: $port)
                .textFieldStyle(.roundedBorder)
            #if os(iOS)
                .keyboardType(.numberPad)
            #endif

            HStack {
                Spacer()
                Button("Connect") {
                    if let p = Int(port) {
                        onConnect(host, p)
                        dismiss()
                    }
                }
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(16)
        .background(OATheme.Colors.sidebarBackground)
        .onAppear {
            // Pre-fill with last successful endpoint if available
            let d = UserDefaults.standard
            if let h = d.string(forKey: "oa.bridge.last_host") {
                let p = d.integer(forKey: "oa.bridge.last_port")
                if p > 0 { self.host = h; self.port = String(p) }
            }
        }
    }
}
