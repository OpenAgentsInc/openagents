import SwiftUI

struct AgentToggleRow: View {
    let name: String
    let detected: Bool
    @Binding var enabled: Bool
    var showStatus: Bool = true

    private var isInteractive: Bool {
        detected || name == "OpenAgents Coder"
    }

    var body: some View {
        HStack(spacing: 8) {
            VStack(alignment: .leading, spacing: 2) {
                Text(name)
                    .font(OAFonts.ui(.body, 14))
                    .foregroundStyle(OATheme.Colors.textPrimary)

                if showStatus {
                    Text(enabled ? "Enabled" : "Disabled")
                        .font(OAFonts.ui(.caption, 11))
                        .foregroundStyle(enabled ? OATheme.Colors.textSecondary : OATheme.Colors.danger)
                }
            }

            Spacer()

            Toggle("", isOn: $enabled)
                .labelsHidden()
                .disabled(!isInteractive)
                .tint(OATheme.Colors.success)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(enabled ? OATheme.Colors.border.opacity(0.5) : OATheme.Colors.card)
        )
        .contentShape(Rectangle())
        .onTapGesture {
            if isInteractive {
                enabled.toggle()
            }
        }
        .opacity(isInteractive ? 1.0 : 0.5)
    }
}

