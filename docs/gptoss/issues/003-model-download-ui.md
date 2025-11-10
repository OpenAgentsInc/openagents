# Issue #3: Implement Model Download UI with Progress Tracking

**Phase:** 4 (UI & Polish)
**Priority:** P1
**Estimated Effort:** 1 day
**Dependencies:** #1 (Dependencies), #2 (Provider Core)
**Related Issues:** #8 (Settings UI)

---

## Summary

Implement a simple card in the sidebar (bottom left) with progress tracking, disk space validation, and resume support for the GPTOSS 20B model (~12.1 GB). This provides quick visibility and access to model download status without navigating to Settings.

## Acceptance Criteria

- [ ] Simple card appears in sidebar, bottom left of main chat window
- [ ] Card shows model status: "Not Downloaded" / "Downloading..." / "Ready" / "Error"
- [ ] Download progress bar shows percentage (0-100%) when downloading
- [ ] Disk space check before download (require 25 GB free)
- [ ] Pause/Resume functionality works across app restarts
- [ ] Download uses `Hub.snapshot` with `progressHandler`
- [ ] Error handling with retry button
- [ ] Model verification after download (file count, total size)
- [ ] Card is compact and unobtrusive when model is ready
- [ ] Card expands on hover/click to show details

## Technical Implementation

**Upgrade GPTOSSModelManager** to use `Hub.snapshot`:

```swift
import Hub

public actor GPTOSSModelManager {
    public func downloadModel(progressHandler: @escaping (DownloadProgress) -> Void) async throws {
        let repo = Hub.Repo(id: config.modelID)
        let files = ["*.safetensors", "config.json", "tokenizer.json",
                     "tokenizer_config.json", "generation_config.json"]

        let modelDir = try await Hub.snapshot(
            from: repo,
            matching: files,
            progressHandler: { progress in
                let downloadProgress = DownloadProgress(
                    fractionCompleted: progress.fractionCompleted,
                    bytesDownloaded: progress.completedUnitCount,
                    totalBytes: progress.totalUnitCount
                )
                progressHandler(downloadProgress)
            }
        )

        // Verify files
        try verifyDownload(at: modelDir)
    }
}

public struct DownloadProgress: Sendable {
    public var fractionCompleted: Double  // 0.0-1.0
    public var bytesDownloaded: Int64
    public var totalBytes: Int64
    public var estimatedTimeRemaining: TimeInterval?
}
```

**SwiftUI View** (macOS Sidebar Card):

```swift
struct GPTOSSStatusCard: View {
    @ObservedObject var viewModel: GPTOSSDownloadViewModel
    @State private var isExpanded = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Compact header (always visible)
            HStack(spacing: 8) {
                statusIndicator
                VStack(alignment: .leading, spacing: 2) {
                    Text("GPTOSS 20B")
                        .font(.caption)
                        .fontWeight(.medium)
                    Text(viewModel.statusText)
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
                Spacer()

                if viewModel.isDownloaded {
                    Image(systemName: "checkmark.circle.fill")
                        .font(.caption)
                        .foregroundColor(.green)
                }
            }
            .onTapGesture {
                withAnimation {
                    isExpanded.toggle()
                }
            }

            // Expanded details (when downloading or on hover/click)
            if isExpanded || viewModel.isDownloading {
                VStack(alignment: .leading, spacing: 8) {
                    if viewModel.isDownloading {
                        ProgressView(value: viewModel.progress)
                            .progressViewStyle(.linear)

                        HStack {
                            Text("\(Int(viewModel.progress * 100))%")
                            Spacer()
                            Text("\(viewModel.downloadedGB) / \(viewModel.totalGB) GB")
                        }
                        .font(.caption2)
                        .foregroundColor(.secondary)

                        HStack(spacing: 4) {
                            Button("Pause") {
                                viewModel.pauseDownload()
                            }
                            .buttonStyle(.borderless)
                            .font(.caption2)

                            Button("Cancel") {
                                viewModel.cancelDownload()
                            }
                            .buttonStyle(.borderless)
                            .font(.caption2)
                            .foregroundColor(.red)
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
                            .font(.caption2)
                        }
                    } else {
                        // Model ready - show minimal info
                        Text("Ready for use")
                            .font(.caption2)
                            .foregroundColor(.green)
                    }
                }
                .padding(.leading, 4)
            }
        }
        .padding(12)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(Color(nsColor: .controlBackgroundColor))
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(Color.gray.opacity(0.2), lineWidth: 1)
        )
    }

    private var statusIndicator: some View {
        Circle()
            .fill(viewModel.statusColor)
            .frame(width: 8, height: 8)
    }
}
```

**Integration into Sidebar**:

```swift
// In SessionSidebarView or equivalent
VStack {
    // ... existing session list ...

    Spacer()

    // GPTOSS status card at bottom
    GPTOSSStatusCard(viewModel: gptossDownloadViewModel)
        .padding(.horizontal, 8)
        .padding(.bottom, 8)
}
```

## Layout Specifications

**Sidebar Card Placement:**
- Position: Bottom left of sidebar (above any other bottom UI elements)
- Width: Full sidebar width minus padding (16px total margin)
- Height: Compact when ready (~50px), expanded when downloading (~120px)
- Z-index: Above sidebar background, below modals

**Visual Design:**
- Background: `Color(nsColor: .controlBackgroundColor)` with OATheme black surface
- Border: 1px gray at 20% opacity, 8px corner radius
- Status indicator: 8px circle (gray/blue/green/red)
- Typography: Berkeley Mono for status text
- Spacing: 8-12px internal padding

**States:**
1. **Not Downloaded**: Collapsed card with "Download Model" button when expanded
2. **Downloading**: Auto-expanded with progress bar, pause/cancel buttons
3. **Ready**: Collapsed card with green checkmark, expands on click to show "Ready for use"
4. **Error**: Collapsed card with red indicator, expands to show error + retry button

## Testing

- Verify card appears in correct position (bottom left sidebar)
- Test expand/collapse behavior
- Simulate slow network (Network Link Conditioner)
- Test pause/resume across app restarts
- Test with insufficient disk space
- Test network failure and retry
- Verify progress accuracy
- Check card doesn't interfere with session list scrolling

## References

- Hub.snapshot docs: https://github.com/huggingface/swift-transformers
- MLXEmbeddingProvider download pattern
- SessionSidebarView for integration point
- OATheme for color/surface styling
- Integration Spec Section 7.1

## Definition of Done

- [ ] Card implemented and positioned in sidebar (bottom left)
- [ ] Compact when model ready, expanded when downloading
- [ ] Progress tracking accurate (±5%)
- [ ] Pause/resume works reliably
- [ ] Disk space validation prevents insufficient space downloads
- [ ] Error messages clear and actionable
- [ ] Card visually consistent with sidebar design (OATheme surfaces, Berkeley Mono)
- [ ] Doesn't interfere with session list scrolling
- [ ] Committed with message: "Add GPTOSS status card to sidebar with download progress"
