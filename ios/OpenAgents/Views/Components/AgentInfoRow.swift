import SwiftUI

struct AgentInfoRow: View {
    let name: String

    var body: some View {
        HStack(spacing: 8) {
            VStack(alignment: .leading, spacing: 2) {
                Text(name)
                    .font(OAFonts.ui(.body, 14))
                    .foregroundStyle(OATheme.Colors.textPrimary)

                Text("Enabled")
                    .font(OAFonts.ui(.caption, 11))
                    .foregroundStyle(OATheme.Colors.textSecondary)
            }

            Spacer()

            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 16))
                .foregroundStyle(OATheme.Colors.success)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(OATheme.Colors.border.opacity(0.5))
        )
    }
}

