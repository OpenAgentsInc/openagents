# Issue #8: Add GPTOSS Settings UI (macOS)

**Phase:** 4 (UI & Polish)
**Priority:** P2
**Estimated Effort:** 1 day
**Dependencies:** #3 (Download UI), #5 (Registration)
**Related Issues:** #9 (Memory Management)

---

## Summary

Create a comprehensive Settings screen for GPTOSS 20B with model status, controls, memory monitoring, and generation parameters.

## Acceptance Criteria

- [ ] Settings screen accessible via Settings → Agents → GPTOSS
- [ ] Model status card shows current state (not loaded / loading / ready / error)
- [ ] Download section integrated (#3)
- [ ] Load/Unload buttons work
- [ ] Memory usage displayed in real-time
- [ ] Temperature and top-p sliders
- [ ] Max tokens field
- [ ] Auto-unload timeout dropdown
- [ ] Preload on startup toggle
- [ ] System requirements warning for <16 GB
- [ ] All settings persist across app restarts

## Technical Implementation

**Settings View Structure**:

```swift
struct GPTOSSSettingsView: View {
    @StateObject private var viewModel = GPTOSSSettingsViewModel()

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            // Header
            headerSection

            // Model Status
            modelStatusSection

            // Download Section (if not downloaded)
            if !viewModel.isModelDownloaded {
                downloadSection
            }

            // Load/Unload Controls
            if viewModel.isModelDownloaded {
                loadUnloadSection
            }

            // Generation Settings
            if viewModel.isModelLoaded {
                generationSettingsSection
            }

            // Memory Management
            memoryManagementSection

            // System Requirements Warning
            if !viewModel.meetsSystemRequirements {
                systemRequirementsWarning
            }

            Spacer()
        }
        .padding()
        .frame(minWidth: 600, minHeight: 700)
        .onAppear {
            Task { await viewModel.refresh() }
        }
    }

    private var headerSection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("GPTOSS 20B")
                .font(.title)
                .fontWeight(.bold)

            Text("Local code generation and reasoning on Apple Silicon")
                .font(.subheadline)
                .foregroundColor(.secondary)
        }
    }

    private var modelStatusSection: some View {
        GroupBox {
            HStack {
                statusIndicator

                VStack(alignment: .leading, spacing: 4) {
                    Text("Model Status")
                        .font(.headline)
                    Text(viewModel.statusText)
                        .font(.subheadline)
                        .foregroundColor(.secondary)
                }

                Spacer()

                if viewModel.isModelLoaded {
                    VStack(alignment: .trailing, spacing: 4) {
                        Text("Memory Usage")
                            .font(.caption)
                            .foregroundColor(.secondary)
                        Text(viewModel.memoryUsageText)
                            .font(.headline)
                            .foregroundColor(viewModel.memoryWarningColor)
                    }
                }
            }
        }
    }

    private var statusIndicator: some View {
        Circle()
            .fill(viewModel.statusColor)
            .frame(width: 12, height: 12)
    }

    private var loadUnloadSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 12) {
                Text("Model Controls")
                    .font(.headline)

                HStack {
                    if !viewModel.isModelLoaded {
                        Button("Load Model") {
                            Task { await viewModel.loadModel() }
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(viewModel.isLoading)
                    } else {
                        Button("Unload Model") {
                            Task { await viewModel.unloadModel() }
                        }
                        .buttonStyle(.bordered)
                    }

                    if viewModel.isLoading {
                        ProgressView()
                            .scaleEffect(0.7)
                    }
                }

                if let lastUsed = viewModel.lastUsedDate {
                    Text("Last used: \(lastUsed, style: .relative) ago")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
            }
        }
    }

    private var generationSettingsSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 16) {
                Text("Generation Settings")
                    .font(.headline)

                // Temperature
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text("Temperature")
                        Spacer()
                        Text(String(format: "%.2f", viewModel.temperature))
                            .foregroundColor(.secondary)
                    }
                    Slider(value: $viewModel.temperature, in: 0.0...1.0)
                    Text("Controls randomness: 0.0 = deterministic, 1.0 = creative")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }

                // Top-p
                VStack(alignment: .leading, spacing: 4) {
                    HStack {
                        Text("Top-p")
                        Spacer()
                        Text(String(format: "%.2f", viewModel.topP))
                            .foregroundColor(.secondary)
                    }
                    Slider(value: $viewModel.topP, in: 0.0...1.0)
                    Text("Nucleus sampling: lower = more focused, higher = more diverse")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }

                // Max Tokens
                HStack {
                    Text("Max Tokens")
                    TextField("Unlimited", value: $viewModel.maxTokens, format: .number)
                        .frame(width: 100)
                    Text("(blank = unlimited)")
                        .font(.caption)
                        .foregroundColor(.secondary)
                }

                Button("Reset to Defaults") {
                    viewModel.resetToDefaults()
                }
                .buttonStyle(.bordered)
            }
        }
    }

    private var memoryManagementSection: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 16) {
                Text("Memory Management")
                    .font(.headline)

                // Auto-unload timeout
                HStack {
                    Text("Auto-unload after idle:")
                    Picker("", selection: $viewModel.idleTimeout) {
                        Text("Never").tag(0)
                        Text("5 minutes").tag(300)
                        Text("10 minutes").tag(600)
                        Text("30 minutes").tag(1800)
                        Text("1 hour").tag(3600)
                    }
                    .labelsHidden()
                }

                // Preload on startup
                Toggle("Preload model on app startup", isOn: $viewModel.preloadOnStartup)
                    .toggleStyle(.switch)

                Text("Preloading increases startup time but makes GPTOSS instantly available")
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
        }
    }

    private var systemRequirementsWarning: some View {
        GroupBox {
            HStack {
                Image(systemName: "exclamationmark.triangle.fill")
                    .foregroundColor(.orange)
                VStack(alignment: .leading, spacing: 4) {
                    Text("System Requirements Not Met")
                        .font(.headline)
                    Text(viewModel.requirementsMessage)
                        .font(.subheadline)
                }
            }
        }
        .background(Color.orange.opacity(0.1))
    }
}
```

**ViewModel**:

```swift
@MainActor
class GPTOSSSettingsViewModel: ObservableObject {
    @Published var statusText: String = "Not Loaded"
    @Published var statusColor: Color = .gray
    @Published var isModelDownloaded: Bool = false
    @Published var isModelLoaded: Bool = false
    @Published var isLoading: Bool = false
    @Published var memoryUsageText: String = "--"
    @Published var memoryWarningColor: Color = .primary
    @Published var temperature: Double = 0.7
    @Published var topP: Double = 0.9
    @Published var maxTokens: Int? = nil
    @Published var idleTimeout: Int = 600
    @Published var preloadOnStartup: Bool = false
    @Published var meetsSystemRequirements: Bool = true
    @Published var requirementsMessage: String = ""
    @Published var lastUsedDate: Date? = nil

    private let provider: GPTOSSAgentProvider
    private let modelManager: GPTOSSModelManager

    init(provider: GPTOSSAgentProvider = GPTOSSAgentProvider()) {
        self.provider = provider
        self.modelManager = GPTOSSModelManager()
        Task { await refresh() }
    }

    func refresh() async {
        // Update status
        let state = await modelManager.currentState
        updateStatus(state)

        // Check system requirements
        meetsSystemRequirements = await provider.isAvailable()
        if !meetsSystemRequirements {
            requirementsMessage = buildRequirementsMessage()
        }

        // Update memory usage
        if isModelLoaded {
            updateMemoryUsage()
        }
    }

    func loadModel() async {
        isLoading = true
        defer { isLoading = false }

        do {
            try await modelManager.loadModel()
            await refresh()
        } catch {
            statusText = "Load Failed: \(error.localizedDescription)"
            statusColor = .red
        }
    }

    func unloadModel() async {
        await modelManager.unloadModel()
        await refresh()
    }

    func resetToDefaults() {
        temperature = 0.7
        topP = 0.9
        maxTokens = nil
        idleTimeout = 600
        preloadOnStartup = false
    }

    private func updateStatus(_ state: GPTOSSModelState) {
        switch state {
        case .notLoaded:
            statusText = "Not Loaded"
            statusColor = .gray
            isModelLoaded = false
        case .downloading(let progress):
            statusText = "Downloading... \(Int(progress * 100))%"
            statusColor = .blue
            isModelLoaded = false
        case .loading:
            statusText = "Loading into memory..."
            statusColor = .blue
            isModelLoaded = false
        case .ready:
            statusText = "Ready"
            statusColor = .green
            isModelLoaded = true
        case .error(let message):
            statusText = "Error: \(message)"
            statusColor = .red
            isModelLoaded = false
        }
    }

    private func updateMemoryUsage() {
        // Get current memory usage
        // Implementation details...
    }

    private func buildRequirementsMessage() -> String {
        let memory = ProcessInfo.processInfo.physicalMemory / 1_000_000_000
        if memory < 16 {
            return "Requires 16 GB+ RAM (detected \(memory) GB)"
        }
        return "Requires macOS 13.0+ on Apple Silicon"
    }
}
```

## Testing

- Navigate to Settings → Agents → GPTOSS
- Verify all controls are functional
- Test load/unload
- Verify settings persist across app restarts
- Test on <16 GB Mac (if available) to see warning

## References

- Integration Spec Section 7.1
- macOS Settings patterns in app

## Definition of Done

- [ ] Settings screen implemented and accessible
- [ ] All controls functional
- [ ] Settings persist
- [ ] Memory usage updates in real-time
- [ ] System warnings display correctly
- [ ] Committed with message: "Add GPTOSS Settings UI for macOS"
