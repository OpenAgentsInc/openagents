import SwiftUI

struct ManualConnectSheet: View {
    @EnvironmentObject var bridge: BridgeManager
    @Environment(\.dismiss) var dismiss
    @State private var host: String = ""
    @State private var port: String = "9099"

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Manual Connect")
                .font(Font.custom(BerkeleyFont.defaultName(), size: 16, relativeTo: .headline))
                .foregroundStyle(OATheme.Colors.textPrimary)

            TextField("Host (e.g. 192.168.1.10)", text: $host)
                .textFieldStyle(.roundedBorder)
            TextField("Port", text: $port)
                .textFieldStyle(.roundedBorder)
                .keyboardType(.numberPad)

            HStack {
                Spacer()
                Button("Connect") {
                    if let p = Int(port) {
                        bridge.log("manual", "Connecting to \(host):\(p)")
                        bridge.performManualConnect(host: host, port: p)
                        dismiss()
                    }
                }
                .buttonStyle(.borderedProminent)
            }
        }
        .padding(16)
        .background(OATheme.Colors.sidebarBackground)
    }
}

