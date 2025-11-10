import SwiftUI

#if os(macOS)
struct GPTOSSStatusCard: View {
    @ObservedObject var viewModel: GPTOSSDownloadViewModel
    @State private var isExpanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                statusIndicator
                VStack(alignment: .leading, spacing: 2) {
                    Text("GPTOSS 20B")
                        .font(OAFonts.ui(.caption, 11))
                        .fontWeight(.medium)
                        .foregroundStyle(OATheme.Colors.textPrimary)
                    Text(viewModel.statusText)
                        .font(OAFonts.ui(.caption, 10))
                        .foregroundStyle(OATheme.Colors.textSecondary)
                }
                Spacer()
                if viewModel.isDownloaded {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.caption)
                        .foregroundColor(.green)
                }
            }
            .onTapGesture { withAnimation { isExpanded.toggle() } }

            // Show details when expanded, when downloading, or when not installed yet
            if isExpanded || viewModel.isDownloading || !viewModel.isDownloaded {
                VStack(alignment: .leading, spacing: 8) {
                    if viewModel.isDownloading {
                        ProgressView(value: viewModel.progress)
                            .progressViewStyle(.linear)
                        HStack {
                            Text("\(Int(viewModel.progress * 100))%")
                            Spacer()
                            Text("\(viewModel.downloadedGB) / \(viewModel.totalGB) GB")
                        }
                        .font(OAFonts.ui(.caption, 10))
                        .foregroundStyle(OATheme.Colors.textSecondary)

                        HStack(spacing: 8) {
                            Button("Pause") { viewModel.pauseDownload() }
                                .buttonStyle(.borderless)
                                .font(OAFonts.ui(.caption, 10))
                            Button("Cancel") { viewModel.cancelDownload() }
                                .buttonStyle(.borderless)
                                .font(OAFonts.ui(.caption, 10))
                                .foregroundStyle(Color.red)
                        }
                    } else if case .paused = viewModel.status {
                        HStack(spacing: 6) {
                            Image(systemName: "pause.fill").foregroundStyle(.orange)
                            Text("Paused at \(Int(viewModel.progress * 100))%")
                        }
                        .font(OAFonts.ui(.caption, 10))
                        .foregroundStyle(OATheme.Colors.textSecondary)

                        HStack(spacing: 8) {
                            Button("Resume") { Task { await viewModel.startDownload() } }
                                .buttonStyle(.bordered)
                                .controlSize(.small)
                            Button("Cancel") { viewModel.cancelDownload() }
                                .buttonStyle(.borderless)
                                .font(OAFonts.ui(.caption, 10))
                                .foregroundStyle(Color.red)
                        }
                    } else if !viewModel.isDownloaded {
                        Button("Download Model") {
                            Task { await viewModel.startDownload() }
                        }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .disabled(!viewModel.hasSufficientSpace)

                        if !viewModel.hasSufficientSpace {
                            HStack(spacing: 4) {
                                Image(systemName: "exclamationmark.triangle.fill")
                                    .foregroundColor(.orange)
                                Text("Need 25 GB free")
                            }
                            .font(OAFonts.ui(.caption, 10))
                            .foregroundStyle(OATheme.Colors.textSecondary)
                        }
                    } else {
                        Text("Ready for use")
                            .font(OAFonts.ui(.caption, 10))
                            .foregroundStyle(.green)
                    }
                }
                .padding(.leading, 4)
            }
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .fill(OATheme.Colors.bgQuaternary)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8, style: .continuous)
                .stroke(OATheme.Colors.border.opacity(0.6), lineWidth: 1)
        )
        .task {
            // On first appear, see if the model is already installed and mark Ready
            await viewModel.refreshInstalled()
        }
    }

    private var statusIndicator: some View {
        Circle()
            .fill(viewModel.statusColor)
            .frame(width: 8, height: 8)
    }
}
#endif
