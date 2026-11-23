// Firm reason: 6 (cross-Apple pure-black OLED Workspace File Viewer, Git Staging/Commit/Diff, Branches, Worktrees, and Pull Requests inspector shared by OpenAgents iOS and OpenAgents macOS).
import PDFKit
import SwiftUI

struct WorkspaceInspectorView: View {
    @ObservedObject var store: AntigravityStore

    enum Tab: String, CaseIterable, Identifiable {
        case files = "Files"
        case diff = "Diff"
        case branches = "Branches"
        case worktrees = "Worktrees"
        case prs = "PRs"
        var id: String { rawValue }
    }

    enum FileBrowseMode: String, CaseIterable, Identifiable {
        case changed = "Changed"
        case tree = "All Files"
        var id: String { rawValue }
    }

    enum DiffScope: String, CaseIterable, Identifiable {
        case uncommitted = "Uncommitted"
        case branch = "Branch"
        case agentEdits = "Agent Edits"
        case forTurn = "For Turn"
        var id: String { rawValue }
    }

    @State private var selectedTab: Tab = .diff
    @State private var fileBrowseMode: FileBrowseMode = .changed
    @State private var diffScope: DiffScope = .uncommitted
    @State private var selectedFilePath: String?
    @State private var fileFilter: String = ""
    @State private var markdownPreview: Bool = true
    @State private var copiedFilePath: String?
    @State private var commentLineRange: String = ""
    @State private var commentDraft: String = ""
    @State private var commentingPath: String?
    @State private var showCommitForm: Bool = false
    @State private var commitSubject: String = ""
    @State private var commitBody: String = ""
    @State private var commitAmend: Bool = false
    @State private var newBranchName = ""
    @State private var newWorktreeBranch = ""
    @State private var prTitle = ""
    @State private var prBase = "main"
    @State private var prBody = ""
    @State private var showCreatePR = false
    @State private var editingPRNumber: Int?
    @State private var editPRTitle = ""
    @State private var editPRBody = ""

    var body: some View {
        VStack(spacing: 0) {
            headerBar
            Divider().overlay(OATheme.hairline)

            Picker("Workspace tab", selection: $selectedTab) {
                ForEach(Tab.allCases) { tab in
                    Text(tab.rawValue).tag(tab)
                }
            }
            .pickerStyle(.segmented)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .accessibilityIdentifier("workspace.tabs")

            if let err = store.workspaceError, !err.isEmpty {
                HStack(spacing: 8) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.statusWarn)
                    Text(err)
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.textPrimary)
                        .lineLimit(3)
                    Spacer()
                    Button { store.workspaceError = nil } label: {
                        Image(systemName: "xmark")
                            .font(.system(size: 10, weight: .bold))
                            .foregroundStyle(OATheme.textMuted)
                    }
                    .buttonStyle(.plain)
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .background(OATheme.surface)
                Divider().overlay(OATheme.hairline)
            }

            if !store.inlineReviewComments.isEmpty {
                inlineCommentsBar
                Divider().overlay(OATheme.hairline)
            }

            if store.isLoadingWorkspace && store.workspaceSnapshot == nil {
                VStack(spacing: 10) {
                    Spacer()
                    ProgressView().tint(OATheme.textPrimary)
                    Text("Loading workspace from computer…")
                        .font(.system(size: 13))
                        .foregroundStyle(OATheme.textSecondary)
                    Spacer()
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if let snap = store.workspaceSnapshot {
                ScrollView {
                    VStack(alignment: .leading, spacing: 14) {
                        switch selectedTab {
                        case .files:
                            filesTab(snap)
                        case .diff:
                            diffTab(snap)
                        case .branches:
                            branchesTab(snap)
                        case .worktrees:
                            worktreesTab(snap)
                        case .prs:
                            prsTab(snap)
                        }
                    }
                    .padding(14)
                }
            } else {
                VStack(spacing: 10) {
                    Spacer()
                    Image(systemName: "arrow.triangle.branch")
                        .font(.system(size: 26))
                        .foregroundStyle(OATheme.textMuted)
                    Text(store.activeComputer?.canSendRPC == true
                         ? "Tap refresh to inspect files, Git staging, diffs, branches, and PRs."
                         : "Connect a computer over Tailscale to review its workspace.")
                        .font(.system(size: 13))
                        .foregroundStyle(OATheme.textSecondary)
                        .multilineTextAlignment(.center)
                        .padding(.horizontal, 24)
                    if store.activeComputer?.canSendRPC == true {
                        Button("Load Workspace") {
                            Task { await store.refreshWorkspace() }
                        }
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(Color.black)
                        .padding(.horizontal, 16)
                        .padding(.vertical, 8)
                        .background(Color.white, in: Capsule())
                        .buttonStyle(.plain)
                    }
                    Spacer()
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .background(OATheme.background)
        .onChange(of: store.activeFileContent?.path) { _, newPath in
            if let path = newPath, !path.isEmpty {
                selectedTab = .files
                selectedFilePath = path
            }
        }
        .task(id: store.activeComputerID) {
            if store.activeComputer?.canSendRPC == true && store.workspaceSnapshot == nil {
                await store.refreshWorkspace()
            }
        }
    }

    private var headerBar: some View {
        HStack(spacing: 8) {
            Image(systemName: "arrow.triangle.branch")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(OATheme.textSecondary)
            VStack(alignment: .leading, spacing: 1) {
                Text(store.workspaceSnapshot?.branch.isEmpty == false ? (store.workspaceSnapshot?.branch ?? "Workspace") : "Workspace")
                    .font(.system(size: 13, weight: .semibold, design: .monospaced))
                    .foregroundStyle(OATheme.textPrimary)
                    .lineLimit(1)
                    .accessibilityIdentifier("workspace.branchHeader")
                if let repo = store.workspaceSnapshot?.repository, !repo.isEmpty {
                    Text(repo)
                        .font(.system(size: 11))
                        .foregroundStyle(OATheme.textMuted)
                        .lineLimit(1)
                }
            }
            Spacer()
            if store.workspaceSnapshot != nil {
                Button {
                    Task { await store.pushCurrentBranch() }
                } label: {
                    HStack(spacing: 4) {
                        Image(systemName: "arrow.up.circle")
                            .font(.system(size: 12))
                        Text("Push")
                            .font(.system(size: 12, weight: .medium))
                    }
                    .foregroundStyle(OATheme.textPrimary)
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(OATheme.elevated, in: Capsule())
                    .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                }
                .buttonStyle(.plain)
                .disabled(store.workspaceActionBusy)
                .accessibilityIdentifier("workspace.push")
            }
            Button {
                Task { await store.refreshWorkspace() }
            } label: {
                Image(systemName: "arrow.clockwise")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(OATheme.textSecondary)
                    .frame(width: 28, height: 28)
            }
            .buttonStyle(.plain)
            .disabled(store.isLoadingWorkspace)
            .accessibilityIdentifier("workspace.refresh")
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(OATheme.background)
    }

    private var inlineCommentsBar: some View {
        HStack(spacing: 8) {
            Image(systemName: "text.bubble.fill")
                .font(.system(size: 11))
                .foregroundStyle(OATheme.statusWarn)
            Text("\(store.inlineReviewComments.count) queued review comment\(store.inlineReviewComments.count == 1 ? "" : "s") — sent with next message")
                .font(.system(size: 11.5, weight: .medium))
                .foregroundStyle(OATheme.textPrimary)
                .lineLimit(1)
            Spacer()
            Button("Send Now") {
                store.isWorkspacePresented = false
                Task { await store.sendMessage() }
            }
            .font(.system(size: 11, weight: .semibold))
            .foregroundStyle(Color.black)
            .padding(.horizontal, 9)
            .padding(.vertical, 4)
            .background(Color.white, in: Capsule())
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 7)
        .background(OATheme.surface)
    }

    // MARK: - Files Tab (Remote Tree + Pure-SwiftUI File Viewer, Markdown, Jupyter, PDF, Quote & Inline Comments)

    @ViewBuilder
    private func filesTab(_ snap: WorkspaceSnapshot) -> some View {
        HStack(spacing: 8) {
            Picker("Browse mode", selection: $fileBrowseMode) {
                ForEach(FileBrowseMode.allCases) { mode in
                    Text(mode.rawValue).tag(mode)
                }
            }
            .pickerStyle(.segmented)
            .onChange(of: fileBrowseMode) { _, newMode in
                if newMode == .tree && store.workspaceFiles.isEmpty {
                    Task { await store.loadWorkspaceTree(prefix: fileFilter, reset: true) }
                }
            }
        }

        HStack(spacing: 8) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 12))
                .foregroundStyle(OATheme.textMuted)
            TextField(fileBrowseMode == .tree ? "Filter workspace path prefix…" : "Filter changed files…", text: $fileFilter)
                .textFieldStyle(.plain)
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textPrimary)
                .onSubmit {
                    if fileBrowseMode == .tree {
                        Task { await store.loadWorkspaceTree(prefix: fileFilter, reset: true) }
                    }
                }
            if !fileFilter.isEmpty {
                Button {
                    fileFilter = ""
                    if fileBrowseMode == .tree {
                        Task { await store.loadWorkspaceTree(prefix: "", reset: true) }
                    }
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.textMuted)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 7)
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))

        if let active = store.activeFileContent {
            remoteFileViewerCard(active, rootPath: snap.path)
        } else if store.isLoadingFileContent {
            HStack(spacing: 8) {
                ProgressView().controlSize(.small).tint(OATheme.textSecondary)
                Text("Loading file…")
                    .font(.system(size: 12))
                    .foregroundStyle(OATheme.textSecondary)
            }
            .padding(10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        }

        if fileBrowseMode == .changed {
            changedFilesList(snap)
        } else {
            remoteTreeList(snap)
        }
    }

    @ViewBuilder
    private func changedFilesList(_ snap: WorkspaceSnapshot) -> some View {
        let allFiles = snap.changedFiles
        let q = fileFilter.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let files = q.isEmpty ? allFiles : allFiles.filter { $0.path.lowercased().contains(q) }

        if files.isEmpty {
            HStack(spacing: 8) {
                Image(systemName: "checkmark.circle")
                    .foregroundStyle(OATheme.statusReady)
                Text(allFiles.isEmpty
                     ? "No modified files in \(snap.branch.isEmpty ? "workspace" : snap.branch). Switch to \"All Files\" to browse the repository."
                     : "No modified files matching \"\(fileFilter)\".")
                    .font(.system(size: 13))
                    .foregroundStyle(OATheme.textSecondary)
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        } else {
            VStack(spacing: 8) {
                ForEach(files) { file in
                    let isOpen = selectedFilePath == file.path
                    VStack(alignment: .leading, spacing: 6) {
                        HStack(spacing: 8) {
                            Button {
                                selectedFilePath = isOpen ? nil : file.path
                            } label: {
                                HStack(spacing: 8) {
                                    Text(file.code)
                                        .font(.system(size: 10, weight: .bold, design: .monospaced))
                                        .foregroundStyle(file.code.contains("?") || file.code.contains("A") ? OATheme.statusReady : OATheme.statusWarn)
                                        .frame(width: 22)
                                    Text(file.path)
                                        .font(.system(size: 12, weight: .medium, design: .monospaced))
                                        .foregroundStyle(OATheme.textPrimary)
                                        .lineLimit(1)
                                        .truncationMode(.middle)
                                    Spacer()
                                }
                            }
                            .buttonStyle(.plain)

                            Button {
                                Task { await store.readWorkspaceFile(path: file.path, startLine: 1, append: false) }
                            } label: {
                                Text("Open")
                                    .font(.system(size: 10.5, weight: .semibold))
                                    .foregroundStyle(OATheme.textPrimary)
                                    .padding(.horizontal, 8)
                                    .padding(.vertical, 3)
                                    .background(OATheme.elevated, in: Capsule())
                                    .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                            }
                            .buttonStyle(.plain)
                        }
                        .padding(.horizontal, 10)
                        .padding(.vertical, 9)
                        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))

                        if isOpen && !file.diff.isEmpty {
                            DiffBlockView(path: file.path, diff: file.diff, truncated: snap.diffCut)
                        }
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func remoteTreeList(_ snap: WorkspaceSnapshot) -> some View {
        let q = fileFilter.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let filtered = q.isEmpty ? store.workspaceFiles : store.workspaceFiles.filter { $0.lowercased().contains(q) }

        if store.isLoadingWorkspaceFiles && store.workspaceFiles.isEmpty {
            HStack(spacing: 8) {
                ProgressView().controlSize(.small).tint(OATheme.textSecondary)
                Text("Loading repository file tree…")
                    .font(.system(size: 12.5))
                    .foregroundStyle(OATheme.textSecondary)
            }
            .padding(10)
        } else if filtered.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text("No files loaded yet.")
                    .font(.system(size: 12.5))
                    .foregroundStyle(OATheme.textMuted)
                Button("Fetch Workspace Tree") {
                    Task { await store.loadWorkspaceTree(prefix: fileFilter, reset: true) }
                }
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(Color.black)
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .background(Color.white, in: Capsule())
                .buttonStyle(.plain)
            }
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        } else {
            VStack(spacing: 5) {
                ForEach(filtered, id: \.self) { path in
                    Button {
                        selectedFilePath = path
                        Task { await store.readWorkspaceFile(path: path, startLine: 1, append: false) }
                    } label: {
                        HStack(spacing: 8) {
                            Image(systemName: fileIcon(for: path))
                                .font(.system(size: 11))
                                .foregroundStyle(OATheme.textMuted)
                                .frame(width: 16)
                            Text(path)
                                .font(.system(size: 12, design: .monospaced))
                                .foregroundStyle(OATheme.textPrimary)
                                .lineLimit(1)
                                .truncationMode(.middle)
                            Spacer()
                            Image(systemName: "chevron.right")
                                .font(.system(size: 10))
                                .foregroundStyle(OATheme.textMuted)
                        }
                        .padding(.horizontal, 10)
                        .padding(.vertical, 7)
                        .background(
                            (store.activeFileContent?.path == path ? OATheme.elevated : OATheme.surface),
                            in: RoundedRectangle(cornerRadius: 8, style: .continuous)
                        )
                        .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                    }
                    .buttonStyle(.plain)
                }

                if store.workspaceFilesNextCursor > 0 {
                    Button {
                        Task { await store.loadWorkspaceTree(prefix: fileFilter, reset: false) }
                    } label: {
                        HStack(spacing: 6) {
                            if store.isLoadingWorkspaceFiles {
                                ProgressView().controlSize(.small).tint(OATheme.textPrimary)
                            }
                            Text("Load More Files")
                                .font(.system(size: 12, weight: .semibold))
                        }
                        .foregroundStyle(OATheme.textPrimary)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 8)
                        .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                    }
                    .buttonStyle(.plain)
                    .disabled(store.isLoadingWorkspaceFiles)
                }
            }
        }
    }

    @ViewBuilder
    private func remoteFileViewerCard(_ file: WorkspaceFileContent, rootPath: String) -> some View {
        let ext = URL(fileURLWithPath: file.path).pathExtension.lowercased()
        let isMarkdown = ext == "md" || ext == "markdown"
        let isNotebook = ext == "ipynb"
        let isPDF = ext == "pdf"

        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 8) {
                Image(systemName: fileIcon(for: file.path))
                    .font(.system(size: 12))
                    .foregroundStyle(OATheme.textSecondary)
                Text(file.path)
                    .font(.system(size: 12, weight: .semibold, design: .monospaced))
                    .foregroundStyle(OATheme.textPrimary)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Spacer()
                Button {
                    store.activeFileContent = nil
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .font(.system(size: 14))
                        .foregroundStyle(OATheme.textMuted)
                }
                .buttonStyle(.plain)
            }

            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 6) {
                    Button {
                        OAClipboard.copy(file.path)
                        copiedFilePath = file.path
                        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                            if copiedFilePath == file.path { copiedFilePath = nil }
                        }
                    } label: {
                        Label(copiedFilePath == file.path ? "Copied" : "Copy Path", systemImage: copiedFilePath == file.path ? "checkmark" : "doc.on.doc")
                            .font(.system(size: 11, weight: .medium))
                            .foregroundStyle(OATheme.textSecondary)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(OATheme.elevated, in: Capsule())
                            .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                    }
                    .buttonStyle(.plain)

                    Button {
                        store.quoteFileRange(path: file.path, lineRange: commentLineRange)
                    } label: {
                        Label("Quote to Chat", systemImage: "quote.opening")
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(OATheme.textPrimary)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(OATheme.elevated, in: Capsule())
                            .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                    }
                    .buttonStyle(.plain)

                    Button {
                        commentingPath = commentingPath == file.path ? nil : file.path
                    } label: {
                        Label("Comment", systemImage: "text.bubble")
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(OATheme.textPrimary)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 4)
                            .background(OATheme.elevated, in: Capsule())
                            .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                    }
                    .buttonStyle(.plain)

                    if isMarkdown {
                        Button(markdownPreview ? "Raw Source" : "Preview MD") {
                            markdownPreview.toggle()
                        }
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(OATheme.textPrimary)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background(OATheme.elevated, in: Capsule())
                        .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                        .buttonStyle(.plain)
                    }
                }
            }

            if commentingPath == file.path {
                inlineCommentComposer(for: file.path)
            }

            if isPDF, let pdfURL = localFileURL(root: rootPath, relativePath: file.path) {
                NativePDFPreview(url: pdfURL)
                    .frame(height: 360)
                    .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
            } else if file.isBinary {
                HStack(spacing: 8) {
                    Image(systemName: "doc.zipper")
                        .foregroundStyle(OATheme.textMuted)
                    Text("Binary file — inline text preview unavailable.")
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.textMuted)
                }
                .padding(10)
            } else if isNotebook {
                JupyterNotebookView(rawJSON: file.content)
            } else if isMarkdown && markdownPreview {
                MarkdownContentView(text: file.content, onOpenFileLink: { path, line in
                    store.openFileFromLink(line.map { "\(path)#L\($0)" } ?? path)
                })
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(OATheme.background, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            } else {
                numberedSourceView(content: file.content, language: ext, startLine: file.startLine)
            }

            if file.truncated && file.nextLine > 0 && !file.isBinary {
                Button {
                    Task { await store.readWorkspaceFile(path: file.path, startLine: file.nextLine, append: true) }
                } label: {
                    HStack(spacing: 6) {
                        if store.isLoadingFileContent {
                            ProgressView().controlSize(.small).tint(OATheme.textPrimary)
                        }
                        Text("Load More Lines (from L\(file.nextLine))")
                            .font(.system(size: 11.5, weight: .semibold))
                    }
                    .foregroundStyle(OATheme.textPrimary)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 7)
                    .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                }
                .buttonStyle(.plain)
                .disabled(store.isLoadingFileContent)
            }
        }
        .padding(10)
        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairlineStrong, lineWidth: 1))
    }

    private func numberedSourceView(content: String, language: String, startLine: Int) -> some View {
        let lines = content.split(separator: "\n", omittingEmptySubsequences: false)
        return ScrollView(.horizontal, showsIndicators: true) {
            VStack(alignment: .leading, spacing: 2) {
                ForEach(Array(lines.enumerated()), id: \.offset) { idx, line in
                    let lineNum = startLine + idx
                    HStack(alignment: .top, spacing: 10) {
                        Text("\(lineNum)")
                            .font(.system(size: 10.5, design: .monospaced))
                            .foregroundStyle(OATheme.textMuted)
                            .frame(width: 36, alignment: .trailing)
                        Text(CodeSyntaxHighlighter.highlight(String(line), language: language))
                            .font(.system(size: 11.5, design: .monospaced))
                            .textSelection(.enabled)
                    }
                }
            }
            .padding(10)
        }
        .background(Color(red: 0.03, green: 0.03, blue: 0.035), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
    }

    private func inlineCommentComposer(for path: String) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                TextField("Lines (e.g. L12-L24)", text: $commentLineRange)
                    .textFieldStyle(.plain)
                    .font(.system(size: 11.5, design: .monospaced))
                    .foregroundStyle(OATheme.textPrimary)
                    .padding(7)
                    .background(OATheme.background, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                    .frame(width: 140)
                Spacer()
            }
            TextField("Add inline code review note…", text: $commentDraft, axis: .vertical)
                .textFieldStyle(.plain)
                .lineLimit(2...4)
                .font(.system(size: 12.5))
                .foregroundStyle(OATheme.textPrimary)
                .padding(7)
                .background(OATheme.background, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))

            HStack {
                Button("Queue Comment") {
                    store.addInlineComment(filePath: path, lineRange: commentLineRange, comment: commentDraft)
                    commentDraft = ""
                    commentLineRange = ""
                    commentingPath = nil
                }
                .font(.system(size: 11.5, weight: .semibold))
                .foregroundStyle(Color.black)
                .padding(.horizontal, 10)
                .padding(.vertical, 5)
                .background(Color.white, in: Capsule())
                .buttonStyle(.plain)
                .disabled(commentDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)

                Button("Cancel") {
                    commentingPath = nil
                }
                .font(.system(size: 11.5))
                .foregroundStyle(OATheme.textSecondary)
                .buttonStyle(.plain)
            }
        }
        .padding(8)
        .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
    }

    private func fileIcon(for path: String) -> String {
        switch URL(fileURLWithPath: path).pathExtension.lowercased() {
        case "swift", "coax", "c", "h", "py", "ts", "js", "rs", "go": return "chevron.left.forwardslash.chevron.right"
        case "md", "markdown": return "doc.richtext"
        case "ipynb": return "book.pages"
        case "pdf": return "doc.text.fill"
        case "png", "jpg", "jpeg", "webp": return "photo"
        case "json", "yml", "yaml", "toml": return "curlybraces"
        default: return "doc.text"
        }
    }

    private func localFileURL(root: String, relativePath: String) -> URL? {
        #if os(macOS)
        guard !root.isEmpty, !relativePath.contains("..") else { return nil }
        let rootURL = URL(fileURLWithPath: root).standardizedFileURL
        let fileURL = rootURL.appendingPathComponent(relativePath).standardizedFileURL
        guard fileURL.path.hasPrefix(rootURL.path + "/"),
              FileManager.default.fileExists(atPath: fileURL.path) else { return nil }
        return fileURL
        #else
        return nil
        #endif
    }

    // MARK: - Diff Tab (Uncommitted / Branch / Agent Edits / For Turn + Git Stage / Unstage / Discard / Commit)

    private func agentEditItems(forTurnOnly: Bool) -> [TranscriptItem] {
        let slice: [TranscriptItem]
        if forTurnOnly, let lastUserIdx = store.items.lastIndex(where: { $0.role == .user }) {
            slice = Array(store.items[(lastUserIdx + 1)...])
        } else {
            slice = store.items
        }
        return slice.filter { !$0.diff.isEmpty }
    }

    @ViewBuilder
    private func diffTab(_ snap: WorkspaceSnapshot) -> some View {
        Picker("Diff scope", selection: $diffScope) {
            ForEach(DiffScope.allCases) { scope in
                Text(scope.rawValue).tag(scope)
            }
        }
        .pickerStyle(.segmented)
        .onChange(of: diffScope) { _, newScope in
            if newScope == .branch {
                Task { await store.loadBranchDiff(base: prBase) }
            }
        }

        if diffScope == .uncommitted {
            // Commit drawer toggle
            HStack(spacing: 8) {
                Button {
                    showCommitForm.toggle()
                    if showCommitForm && commitSubject.isEmpty {
                        let count = snap.changedFiles.count
                        commitSubject = count == 1
                            ? "Update \(snap.changedFiles[0].path)"
                            : "Update \(count) files"
                    }
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: showCommitForm ? "chevron.up" : "checkmark.seal")
                            .font(.system(size: 12))
                        Text(showCommitForm ? "Hide Commit Form" : "Commit Changes")
                            .font(.system(size: 12.5, weight: .semibold))
                        Spacer()
                        Text("\(snap.changedFiles.count) changed")
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundStyle(OATheme.textMuted)
                    }
                    .foregroundStyle(OATheme.textPrimary)
                    .padding(10)
                    .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("workspace.commitToggle")
            }

            if showCommitForm {
                VStack(alignment: .leading, spacing: 8) {
                    TextField("Commit message subject", text: $commitSubject)
                        .textFieldStyle(.plain)
                        .font(.system(size: 13))
                        .foregroundStyle(OATheme.textPrimary)
                        .padding(9)
                        .background(OATheme.background, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))

                    TextField("Optional commit body", text: $commitBody, axis: .vertical)
                        .textFieldStyle(.plain)
                        .lineLimit(2...4)
                        .font(.system(size: 12.5))
                        .foregroundStyle(OATheme.textPrimary)
                        .padding(9)
                        .background(OATheme.background, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))

                    HStack {
                        Toggle("Amend last commit", isOn: $commitAmend)
                            .font(.system(size: 12))
                            .foregroundStyle(OATheme.textSecondary)
                            .toggleStyle(.switch)
                            .controlSize(.small)
                        Spacer()
                        Button("Commit") {
                            let subj = commitSubject
                            let b = commitBody
                            let amend = commitAmend
                            Task {
                                await store.commitChanges(subject: subj, body: b, amend: amend)
                                if store.workspaceError == nil {
                                    commitSubject = ""
                                    commitBody = ""
                                    commitAmend = false
                                    showCommitForm = false
                                }
                            }
                        }
                        .font(.system(size: 12.5, weight: .semibold))
                        .foregroundStyle(Color.black)
                        .padding(.horizontal, 14)
                        .padding(.vertical, 7)
                        .background(Color.white, in: Capsule())
                        .buttonStyle(.plain)
                        .disabled(commitSubject.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || store.workspaceActionBusy)
                        .accessibilityIdentifier("workspace.commitSubmit")
                    }
                }
                .padding(10)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
            }

            if !snap.changedFiles.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("WORKING TREE (\(snap.changedFiles.count))")
                        .font(.system(size: 11, weight: .semibold))
                        .tracking(0.6)
                        .foregroundStyle(OATheme.textMuted)
                    ForEach(snap.changedFiles) { file in
                        VStack(alignment: .leading, spacing: 6) {
                            HStack(spacing: 6) {
                                Text(file.code)
                                    .font(.system(size: 10, weight: .bold, design: .monospaced))
                                    .foregroundStyle(file.code.contains("?") || file.code.hasPrefix("A") || file.code.hasPrefix("M") ? OATheme.statusReady : OATheme.statusWarn)
                                    .frame(width: 22)
                                Text(file.path)
                                    .font(.system(size: 11.5, design: .monospaced))
                                    .foregroundStyle(OATheme.textPrimary)
                                    .lineLimit(1)
                                    .truncationMode(.middle)
                                Spacer()
                                Button {
                                    Task { await store.stageFile(file.path) }
                                } label: {
                                    Image(systemName: "plus")
                                        .font(.system(size: 10, weight: .bold))
                                        .foregroundStyle(OATheme.statusReady)
                                        .frame(width: 22, height: 22)
                                        .background(OATheme.elevated, in: Circle())
                                }
                                .buttonStyle(.plain)
                                .disabled(store.workspaceActionBusy)
                                .help("Stage file")

                                Button {
                                    Task { await store.unstageFile(file.path) }
                                } label: {
                                    Image(systemName: "minus")
                                        .font(.system(size: 10, weight: .bold))
                                        .foregroundStyle(OATheme.textSecondary)
                                        .frame(width: 22, height: 22)
                                        .background(OATheme.elevated, in: Circle())
                                }
                                .buttonStyle(.plain)
                                .disabled(store.workspaceActionBusy)
                                .help("Unstage file")

                                Button {
                                    Task { await store.discardFile(file.path) }
                                } label: {
                                    Image(systemName: "arrow.uturn.backward")
                                        .font(.system(size: 10, weight: .bold))
                                        .foregroundStyle(OATheme.statusError)
                                        .frame(width: 22, height: 22)
                                        .background(OATheme.elevated, in: Circle())
                                }
                                .buttonStyle(.plain)
                                .disabled(store.workspaceActionBusy)
                                .help("Discard changes")

                                Button {
                                    commentingPath = commentingPath == file.path ? nil : file.path
                                } label: {
                                    Image(systemName: "text.bubble")
                                        .font(.system(size: 10, weight: .bold))
                                        .foregroundStyle(OATheme.textSecondary)
                                        .frame(width: 22, height: 22)
                                        .background(OATheme.elevated, in: Circle())
                                }
                                .buttonStyle(.plain)
                                .help("Add inline comment")
                            }
                            if commentingPath == file.path {
                                inlineCommentComposer(for: file.path)
                            }
                        }
                        .padding(.horizontal, 10)
                        .padding(.vertical, 7)
                        .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                        .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                    }
                }
            }

            if snap.diff.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                HStack(spacing: 8) {
                    Image(systemName: "checkmark.circle")
                        .foregroundStyle(OATheme.statusReady)
                    Text("Working tree is clean.")
                        .font(.system(size: 13))
                        .foregroundStyle(OATheme.textSecondary)
                }
                .padding(12)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
            } else {
                VStack(alignment: .leading, spacing: 6) {
                    Text("UNIFIED DIFF")
                        .font(.system(size: 11, weight: .semibold))
                        .tracking(0.6)
                        .foregroundStyle(OATheme.textMuted)
                    DiffBlockView(path: snap.branch, diff: snap.diff, truncated: snap.diffCut)
                }
            }
        } else if diffScope == .branch {
            HStack(spacing: 8) {
                Text("Base:")
                    .font(.system(size: 12))
                    .foregroundStyle(OATheme.textSecondary)
                TextField("main", text: $prBase)
                    .textFieldStyle(.plain)
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(OATheme.textPrimary)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 6)
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                Button("Compare") {
                    Task { await store.loadBranchDiff(base: prBase) }
                }
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(Color.black)
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .background(Color.white, in: Capsule())
                .buttonStyle(.plain)
            }
            if store.branchDiffText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                Text("No committed branch diff against \(prBase.isEmpty ? "main" : prBase).")
                    .font(.system(size: 12.5))
                    .foregroundStyle(OATheme.textMuted)
                    .padding(12)
            } else {
                DiffBlockView(path: "\(prBase)...\(snap.branch)", diff: store.branchDiffText, truncated: false)
            }
        } else {
            let edits = agentEditItems(forTurnOnly: diffScope == .forTurn)
            if edits.isEmpty {
                Text(diffScope == .forTurn
                     ? "No agent file diffs recorded in the latest turn."
                     : "No agent file diffs recorded in this conversation.")
                    .font(.system(size: 12.5))
                    .foregroundStyle(OATheme.textMuted)
                    .padding(12)
            } else {
                ForEach(edits) { edit in
                    DiffBlockView(
                        path: edit.path.isEmpty ? edit.toolName : edit.path,
                        diff: edit.diff,
                        truncated: edit.diffCut
                    )
                }
            }
        }
    }

    // MARK: - Branches Tab

    @ViewBuilder
    private func branchesTab(_ snap: WorkspaceSnapshot) -> some View {
        HStack(spacing: 8) {
            TextField("new-branch-name", text: $newBranchName)
                .textFieldStyle(.plain)
                .font(.system(size: 13, design: .monospaced))
                .foregroundStyle(OATheme.textPrimary)
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))

            Button("Create") {
                let target = newBranchName.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !target.isEmpty else { return }
                newBranchName = ""
                Task { await store.createBranch(name: target, base: snap.branch) }
            }
            .font(.system(size: 12, weight: .semibold))
            .foregroundStyle(Color.black)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(Color.white, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            .buttonStyle(.plain)
            .disabled(newBranchName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || store.workspaceActionBusy)
        }

        VStack(spacing: 6) {
            ForEach(snap.branches, id: \.self) { branch in
                let isCurrent = branch == snap.branch
                HStack {
                    Image(systemName: isCurrent ? "checkmark.circle.fill" : "arrow.triangle.branch")
                        .font(.system(size: 12))
                        .foregroundStyle(isCurrent ? OATheme.statusReady : OATheme.textMuted)
                    Text(branch)
                        .font(.system(size: 13, weight: isCurrent ? .semibold : .regular, design: .monospaced))
                        .foregroundStyle(OATheme.textPrimary)
                        .lineLimit(1)
                    Spacer()
                    if !isCurrent {
                        Button("Switch") {
                            Task { await store.switchBranch(branch) }
                        }
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(OATheme.textPrimary)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 4)
                        .background(OATheme.elevated, in: Capsule())
                        .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                        .buttonStyle(.plain)
                        .disabled(store.workspaceActionBusy)
                        .accessibilityIdentifier("workspace.switchBranch.\(branch)")
                    }
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 9)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
            }
        }
    }

    // MARK: - Worktrees Tab

    @ViewBuilder
    private func worktreesTab(_ snap: WorkspaceSnapshot) -> some View {
        HStack(spacing: 8) {
            TextField("worktree-branch", text: $newWorktreeBranch)
                .textFieldStyle(.plain)
                .font(.system(size: 13, design: .monospaced))
                .foregroundStyle(OATheme.textPrimary)
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))

            Button("Add") {
                let target = newWorktreeBranch.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !target.isEmpty else { return }
                newWorktreeBranch = ""
                Task { await store.createWorktree(branch: target, base: snap.branch) }
            }
            .font(.system(size: 12, weight: .semibold))
            .foregroundStyle(Color.black)
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(Color.white, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            .buttonStyle(.plain)
            .disabled(newWorktreeBranch.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || store.workspaceActionBusy)
        }

        if snap.worktrees.isEmpty {
            Text("No secondary worktrees.")
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textMuted)
        } else {
            VStack(spacing: 8) {
                ForEach(snap.worktrees) { tree in
                    HStack {
                        VStack(alignment: .leading, spacing: 3) {
                            Text(tree.branch.isEmpty ? tree.id : tree.branch)
                                .font(.system(size: 13, weight: .semibold, design: .monospaced))
                                .foregroundStyle(OATheme.textPrimary)
                            Text(tree.path)
                                .font(.system(size: 11, design: .monospaced))
                                .foregroundStyle(OATheme.textMuted)
                                .lineLimit(1)
                                .truncationMode(.middle)
                        }
                        Spacer()
                        if !tree.locked && tree.path != snap.path && !tree.id.isEmpty {
                            Button("Remove") {
                                Task { await store.removeWorktree(id: tree.id) }
                            }
                            .font(.system(size: 11, weight: .medium))
                            .foregroundStyle(OATheme.statusError)
                            .buttonStyle(.plain)
                            .disabled(store.workspaceActionBusy)
                            .accessibilityIdentifier("workspace.removeWorktree.\(tree.id.prefix(8))")
                        }
                    }
                    .padding(10)
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                }
            }
        }
    }

    // MARK: - PRs Tab (Create, Edit & Clickable URLs)

    @ViewBuilder
    private func prsTab(_ snap: WorkspaceSnapshot) -> some View {
        Button {
            showCreatePR.toggle()
        } label: {
            HStack {
                Image(systemName: showCreatePR ? "minus" : "plus")
                Text(showCreatePR ? "Hide PR form" : "New Pull Request from \(snap.branch)")
                    .font(.system(size: 13, weight: .semibold))
                Spacer()
            }
            .foregroundStyle(OATheme.textPrimary)
            .padding(10)
            .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        }
        .buttonStyle(.plain)

        if showCreatePR {
            VStack(alignment: .leading, spacing: 8) {
                TextField("PR title", text: $prTitle)
                    .textFieldStyle(.plain)
                    .font(.system(size: 13))
                    .foregroundStyle(OATheme.textPrimary)
                    .padding(9)
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))

                TextField("Base branch (e.g. main)", text: $prBase)
                    .textFieldStyle(.plain)
                    .font(.system(size: 13, design: .monospaced))
                    .foregroundStyle(OATheme.textPrimary)
                    .padding(9)
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))

                TextField("Description (optional)", text: $prBody, axis: .vertical)
                    .textFieldStyle(.plain)
                    .lineLimit(2...5)
                    .font(.system(size: 13))
                    .foregroundStyle(OATheme.textPrimary)
                    .padding(9)
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))

                Button("Create Pull Request") {
                    let title = prTitle.trimmingCharacters(in: .whitespacesAndNewlines)
                    let base = prBase.trimmingCharacters(in: .whitespacesAndNewlines)
                    guard !title.isEmpty, !base.isEmpty else { return }
                    Task {
                        await store.createPullRequest(title: title, body: prBody, base: base)
                        prTitle = ""
                        prBody = ""
                        showCreatePR = false
                    }
                }
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(Color.black)
                .padding(.horizontal, 14)
                .padding(.vertical, 8)
                .background(Color.white, in: Capsule())
                .buttonStyle(.plain)
                .disabled(prTitle.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || store.workspaceActionBusy)
            }
            .padding(10)
            .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
        }

        if !snap.githubError.isEmpty {
            Text(HostErrorFormatter.describe(snap.githubError, method: "ReadWorkspace"))
                .font(.system(size: 12))
                .foregroundStyle(OATheme.statusWarn)
        }

        if snap.pullRequests.isEmpty {
            Text("No open pull requests found.")
                .font(.system(size: 13))
                .foregroundStyle(OATheme.textMuted)
        } else {
            VStack(spacing: 8) {
                ForEach(snap.pullRequests) { pr in
                    VStack(alignment: .leading, spacing: 6) {
                        HStack {
                            Text("#\(pr.number)")
                                .font(.system(size: 12, weight: .semibold, design: .monospaced))
                                .foregroundStyle(OATheme.textSecondary)
                            Text(pr.title)
                                .font(.system(size: 13, weight: .semibold))
                                .foregroundStyle(OATheme.textPrimary)
                                .lineLimit(2)
                            Spacer()
                            Text(pr.state.uppercased())
                                .font(.system(size: 10, weight: .semibold, design: .monospaced))
                                .foregroundStyle(OATheme.statusReady)
                        }
                        HStack(spacing: 8) {
                            Text("\(pr.head) → \(pr.base)")
                                .font(.system(size: 11, design: .monospaced))
                                .foregroundStyle(OATheme.textMuted)
                            Spacer()
                            if let url = URL(string: pr.url), !pr.url.isEmpty {
                                Link(destination: url) {
                                    Label("Open", systemImage: "arrow.up.right.square")
                                        .font(.system(size: 11, weight: .medium))
                                        .foregroundStyle(OATheme.textPrimary)
                                }
                            }
                            Button("Edit") {
                                if editingPRNumber == pr.number {
                                    editingPRNumber = nil
                                } else {
                                    editingPRNumber = pr.number
                                    editPRTitle = pr.title
                                    editPRBody = pr.body
                                }
                            }
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(OATheme.textSecondary)
                            .buttonStyle(.plain)
                        }

                        if editingPRNumber == pr.number {
                            VStack(alignment: .leading, spacing: 6) {
                                TextField("PR Title", text: $editPRTitle)
                                    .textFieldStyle(.plain)
                                    .font(.system(size: 12.5))
                                    .foregroundStyle(OATheme.textPrimary)
                                    .padding(8)
                                    .background(OATheme.background, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                                    .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                                TextField("PR Description", text: $editPRBody, axis: .vertical)
                                    .textFieldStyle(.plain)
                                    .lineLimit(2...5)
                                    .font(.system(size: 12))
                                    .foregroundStyle(OATheme.textPrimary)
                                    .padding(8)
                                    .background(OATheme.background, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
                                    .overlay(RoundedRectangle(cornerRadius: 7, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                                HStack {
                                    Button("Save PR") {
                                        let t = editPRTitle
                                        let b = editPRBody
                                        Task {
                                            await store.editPullRequest(number: pr.number, title: t, body: b)
                                            editingPRNumber = nil
                                        }
                                    }
                                    .font(.system(size: 11.5, weight: .semibold))
                                    .foregroundStyle(Color.black)
                                    .padding(.horizontal, 12)
                                    .padding(.vertical, 5)
                                    .background(Color.white, in: Capsule())
                                    .buttonStyle(.plain)
                                    .disabled(editPRTitle.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || store.workspaceActionBusy)

                                    Button("Cancel") { editingPRNumber = nil }
                                        .font(.system(size: 11.5))
                                        .foregroundStyle(OATheme.textSecondary)
                                        .buttonStyle(.plain)
                                }
                            }
                            .padding(.top, 4)
                        }
                    }
                    .padding(10)
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                }
            }
        }
    }
}

// MARK: - Pure-SwiftUI Jupyter Notebook (.ipynb) & Native PDFKit Renderers (#14012)

private struct JupyterNotebookView: View {
    let rawJSON: String

    private struct NotebookCell: Identifiable {
        let id: Int
        let cellType: String
        let source: String
        let outputs: [String]
    }

    private func parseCells() -> [NotebookCell] {
        guard let data = rawJSON.data(using: .utf8),
              let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let cells = obj["cells"] as? [[String: Any]] else {
            return []
        }
        return cells.enumerated().map { idx, cell in
            let kind = cell["cell_type"] as? String ?? "code"
            let src: String
            if let arr = cell["source"] as? [String] {
                src = arr.joined()
            } else {
                src = cell["source"] as? String ?? ""
            }
            var outs: [String] = []
            if let rawOuts = cell["outputs"] as? [[String: Any]] {
                for out in rawOuts {
                    if let textArr = out["text"] as? [String] {
                        outs.append(textArr.joined())
                    } else if let text = out["text"] as? String {
                        outs.append(text)
                    } else if let dataMap = out["data"] as? [String: Any],
                              let plain = dataMap["text/plain"] {
                        if let arr = plain as? [String] { outs.append(arr.joined()) }
                        else if let s = plain as? String { outs.append(s) }
                    }
                }
            }
            return NotebookCell(id: idx, cellType: kind, source: src, outputs: outs)
        }
    }

    var body: some View {
        let cells = parseCells()
        if cells.isEmpty {
            CodeBlockView(language: "json", code: rawJSON)
        } else {
            VStack(alignment: .leading, spacing: 10) {
                ForEach(cells) { cell in
                    VStack(alignment: .leading, spacing: 6) {
                        Text(cell.cellType == "markdown" ? "MARKDOWN [\(cell.id + 1)]" : "CODE [\(cell.id + 1)]")
                            .font(.system(size: 10, weight: .semibold, design: .monospaced))
                            .foregroundStyle(OATheme.textMuted)
                        if cell.cellType == "markdown" {
                            MarkdownContentView(text: cell.source)
                        } else {
                            CodeBlockView(language: "python", code: cell.source)
                        }
                        ForEach(Array(cell.outputs.enumerated()), id: \.offset) { _, out in
                            Text(out)
                                .font(.system(size: 11, design: .monospaced))
                                .foregroundStyle(OATheme.textSecondary)
                                .padding(8)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(OATheme.background, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
                        }
                    }
                    .padding(8)
                    .background(OATheme.elevated, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 8, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                }
            }
        }
    }
}

#if os(iOS)
private struct NativePDFPreview: UIViewRepresentable {
    let url: URL
    func makeUIView(context: Context) -> PDFView {
        let view = PDFView()
        view.autoScales = true
        view.document = PDFDocument(url: url)
        return view
    }
    func updateUIView(_ uiView: PDFView, context: Context) {}
}
#elseif os(macOS)
private struct NativePDFPreview: NSViewRepresentable {
    let url: URL
    func makeNSView(context: Context) -> PDFView {
        let view = PDFView()
        view.autoScales = true
        view.document = PDFDocument(url: url)
        return view
    }
    func updateNSView(_ nsView: PDFView, context: Context) {}
}
#endif
