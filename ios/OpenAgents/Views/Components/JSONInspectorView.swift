import SwiftUI

struct JSONInspectorView: View {
    let json: String
    var body: some View {
        NavigationStack {
            ScrollView {
                Text(json)
                    .font(.system(.footnote, design: .monospaced))
                    .textSelection(.enabled)
                    .padding()
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .background(OATheme.Colors.background)
            .navigationTitle("Tool Call JSON")
            .toolbarTitleDisplayMode(.inline)
        }
    }
}

