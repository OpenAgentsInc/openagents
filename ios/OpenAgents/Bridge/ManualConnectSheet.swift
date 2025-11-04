import SwiftUI
#if os(iOS)
import UIKit
#endif

struct ManualConnectSheet: View {
    @Environment(\.dismiss) var dismiss
    @State private var host: String = "192.168.1.11"
    @State private var port: String = "9099"
    var onConnect: (String, Int) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Manual Connect")
                .font(Font.custom(BerkeleyFont.defaultName(), size: 16, relativeTo: .headline))
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
    }
}
