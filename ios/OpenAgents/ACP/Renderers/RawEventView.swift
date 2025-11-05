import SwiftUI

struct RawEventView: View {
    let line: String

    var pretty: String {
        if let data = line.data(using: .utf8),
           let obj = try? JSONSerialization.jsonObject(with: data),
           let pd = try? JSONSerialization.data(withJSONObject: obj, options: [.prettyPrinted, .sortedKeys]),
           let s = String(data: pd, encoding: .utf8) {
            return s
        }
        return line
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(pretty)
                .font(OAFonts.mono(.footnote, 12))
                .foregroundStyle(OATheme.Colors.textTertiary)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(10)
        .background(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(Color.black.opacity(0.20))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .stroke(OATheme.Colors.border.opacity(0.6), lineWidth: 1)
        )
    }
}
