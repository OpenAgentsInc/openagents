// Firm reason: 6 (cross-Apple pure-black OLED Workspace Git Diff / Branches / Worktrees / Pull Requests inspector shared by OpenAgents iOS and OpenAgents macOS).
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

    @State private var selectedTab: Tab = .diff
    @State private var selectedFilePath: String?
    @State private var fileFilter: String = ""
    @State private var fileViewModeSource: Bool = false
    @State private var copiedFilePath: String?
    @State private var newBranchName = ""
    @State private var newWorktreeBranch = ""
    @State private var prTitle = ""
    @State private var prBase = "main"
    @State private var prBody = ""
    @State private var showCreatePR = false

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
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .background(OATheme.surface)
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
                         ? "Tap refresh to inspect Git status, diffs, branches, and PRs."
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

    // MARK: - Files tab (per-file inspection & inline File Viewer)

    private func readLocalSourcePreview(root: String, relativePath: String) -> String? {
        #if os(macOS)
        guard !root.isEmpty, !relativePath.contains("..") else { return nil }
        let rootURL = URL(fileURLWithPath: root).standardizedFileURL
        let fileURL = rootURL.appendingPathComponent(relativePath).standardizedFileURL
        guard fileURL.path.hasPrefix(rootURL.path + "/"),
              let data = try? Data(contentsOf: fileURL),
              let text = String(data: data.prefix(24_000), encoding: .utf8) else { return nil }
        return text
        #else
        return nil
        #endif
    }

    @ViewBuilder
    private func filesTab(_ snap: WorkspaceSnapshot) -> some View {
        let allFiles = snap.changedFiles
        let q = fileFilter.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let files = q.isEmpty ? allFiles : allFiles.filter { $0.path.lowercased().contains(q) }

        if !allFiles.isEmpty {
            HStack(spacing: 8) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 12))
                    .foregroundStyle(OATheme.textMuted)
                TextField("Filter files…", text: $fileFilter)
                    .textFieldStyle(.plain)
                    .font(.system(size: 13))
                    .foregroundStyle(OATheme.textPrimary)
                if !fileFilter.isEmpty {
                    Button { fileFilter = "" } label: {
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
        }

        if files.isEmpty {
            HStack(spacing: 8) {
                Image(systemName: "checkmark.circle")
                    .foregroundStyle(OATheme.statusReady)
                Text(allFiles.isEmpty
                     ? "No modified files in \(snap.branch.isEmpty ? "workspace" : snap.branch)."
                     : "No files matching \"\(fileFilter)\".")
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
                    let isOpen = selectedFilePath == file.path || files.count == 1
                    let localSource = isOpen ? readLocalSourcePreview(root: snap.path, relativePath: file.path) : nil
                    VStack(alignment: .leading, spacing: 6) {
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
                                Image(systemName: isOpen ? "chevron.up" : "chevron.down")
                                    .font(.system(size: 10))
                                    .foregroundStyle(OATheme.textMuted)
                            }
                            .padding(.horizontal, 10)
                            .padding(.vertical, 9)
                            .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                            .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                        }
                        .buttonStyle(.plain)

                        if isOpen {
                            HStack(spacing: 8) {
                                Button {
                                    OAClipboard.copy(file.path)
                                    copiedFilePath = file.path
                                    DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) {
                                        if copiedFilePath == file.path { copiedFilePath = nil }
                                    }
                                } label: {
                                    Label(copiedFilePath == file.path ? "Copied Path" : "Copy Path", systemImage: copiedFilePath == file.path ? "checkmark" : "doc.on.doc")
                                        .font(.system(size: 11, weight: .medium))
                                        .foregroundStyle(OATheme.textSecondary)
                                        .padding(.horizontal, 8)
                                        .padding(.vertical, 4)
                                        .background(OATheme.elevated, in: Capsule())
                                        .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                                }
                                .buttonStyle(.plain)

                                if localSource != nil {
                                    Button(fileViewModeSource ? "Show Diff" : "View File") {
                                        fileViewModeSource.toggle()
                                    }
                                    .font(.system(size: 11, weight: .semibold))
                                    .foregroundStyle(OATheme.textPrimary)
                                    .padding(.horizontal, 8)
                                    .padding(.vertical, 4)
                                    .background(OATheme.elevated, in: Capsule())
                                    .overlay(Capsule().stroke(OATheme.hairline, lineWidth: 1))
                                    .buttonStyle(.plain)
                                }
                                Spacer()
                            }
                            .padding(.horizontal, 4)

                            if fileViewModeSource, let src = localSource {
                                CodeBlockView(language: URL(fileURLWithPath: file.path).pathExtension, code: src)
                            } else if !file.diff.isEmpty {
                                DiffBlockView(path: file.path, diff: file.diff, truncated: snap.diffCut)
                            } else if let src = localSource {
                                CodeBlockView(language: URL(fileURLWithPath: file.path).pathExtension, code: src)
                            } else {
                                Text("Untracked or binary file change.")
                                    .font(.system(size: 12))
                                    .foregroundStyle(OATheme.textMuted)
                                    .padding(.horizontal, 10)
                            }
                        }
                    }
                }
            }
        }
    }

    // MARK: - Diff tab

    @ViewBuilder
    private func diffTab(_ snap: WorkspaceSnapshot) -> some View {
        if !snap.status.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            VStack(alignment: .leading, spacing: 6) {
                Text("CHANGED FILES")
                    .font(.system(size: 11, weight: .semibold))
                    .tracking(0.6)
                    .foregroundStyle(OATheme.textMuted)
                Text(snap.status)
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(OATheme.textPrimary)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(10)
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 10, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
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
    }

    // MARK: - Branches tab

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

    // MARK: - Worktrees tab

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
                        if !tree.locked && tree.path != snap.path && !tree.branch.isEmpty {
                            Button("Remove") {
                                Task { await store.removeWorktree(branch: tree.branch) }
                            }
                            .font(.system(size: 11, weight: .medium))
                            .foregroundStyle(OATheme.statusError)
                            .buttonStyle(.plain)
                            .disabled(store.workspaceActionBusy)
                        }
                    }
                    .padding(10)
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                }
            }
        }
    }

    // MARK: - PRs tab

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
            Text(snap.githubError)
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
                    VStack(alignment: .leading, spacing: 4) {
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
                        Text("\(pr.head) → \(pr.base)")
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundStyle(OATheme.textMuted)
                    }
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(OATheme.surface, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                    .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).stroke(OATheme.hairline, lineWidth: 1))
                }
            }
        }
    }
}
