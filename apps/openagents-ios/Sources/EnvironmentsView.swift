// Firm reason: 6 (iOS pure-black OLED Cloud Environments, Projects, and Execution Target selector backed by /api/chat/environments/v1 and /api/projects/v1).
import SwiftUI

struct EnvironmentsView: View {
    @ObservedObject var account: MobileAccountController
    @ObservedObject var cloudChat: MobileCloudChat
    @Environment(\.dismiss) private var dismiss

    @State private var isRefreshing = false

    var body: some View {
        NavigationStack {
            List {
                if !account.isLoggedIn {
                    Section {
                        VStack(alignment: .leading, spacing: 8) {
                            Text("Log in to OpenAgents Cloud")
                                .font(.system(size: 15, weight: .semibold))
                                .foregroundStyle(OATheme.textPrimary)
                            Text("Log in to choose execution targets, browse enrolled computers, and attach Cloud Projects.")
                                .font(.system(size: 13))
                                .foregroundStyle(OATheme.textSecondary)
                        }
                        .padding(.vertical, 6)
                        .listRowBackground(OATheme.surface)
                    }
                } else {
                    executionTargetSection
                    enrolledComputersSection
                    projectsSection
                }
            }
            .scrollContentBackground(.hidden)
            .background(OATheme.background)
            .navigationTitle("Environments & Projects")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    if account.isLoggedIn {
                        Button {
                            Task {
                                isRefreshing = true
                                await cloudChat.refreshEnvironmentsAndProjects()
                                isRefreshing = false
                            }
                        } label: {
                            if isRefreshing {
                                ProgressView().controlSize(.small)
                            } else {
                                Image(systemName: "arrow.clockwise")
                            }
                        }
                        .accessibilityIdentifier("environments.refresh")
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }
                        .accessibilityIdentifier("environments.done")
                }
            }
        }
        .preferredColorScheme(.dark)
        .task {
            if account.isLoggedIn {
                await cloudChat.refreshEnvironmentsAndProjects()
            }
        }
    }

    @ViewBuilder
    private var executionTargetSection: some View {
        Section {
            targetRow(
                id: "environments.target.auto",
                title: "Auto",
                subtitle: "Cloud Tools for read/research; enrolled computer ($0) or Cloud Sandbox for code",
                selected: cloudChat.selectedTarget == .auto
            ) {
                cloudChat.selectTarget(.auto)
            }

            targetRow(
                id: "environments.target.cloud",
                title: "OpenAgents Cloud",
                subtitle: "Instant streaming with Web Search, Web Fetch & GitHub tools",
                selected: cloudChat.selectedTarget == .cloud
            ) {
                cloudChat.selectTarget(.cloud)
            }

            targetRow(
                id: "environments.target.sandbox",
                title: "Cloud Sandbox",
                subtitle: "Isolated Linux sandbox with shell & build tools (metered)",
                selected: {
                    if case .sandbox = cloudChat.selectedTarget { return true }
                    return false
                }()
            ) {
                cloudChat.selectTarget(.sandbox(size: "small"))
            }
        } header: {
            Text("Execution Target")
        } footer: {
            if let note = cloudChat.lastPlacementNote, !note.isEmpty {
                Text("Last placement: \(note)")
            }
        }
    }

    @ViewBuilder
    private var enrolledComputersSection: some View {
        Section {
            if cloudChat.environments.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    Text("No enrolled computers yet")
                        .font(.system(size: 14, weight: .medium))
                        .foregroundStyle(OATheme.textPrimary)
                    Text("Log in to OpenAgents Desktop on your Mac or run `openagents-cli` on Linux to enroll a computer for $0 local execution through OpenAgents Cloud.")
                        .font(.system(size: 13))
                        .foregroundStyle(OATheme.textSecondary)
                }
                .padding(.vertical, 6)
                .listRowBackground(OATheme.surface)
            } else {
                ForEach(cloudChat.environments) { env in
                    let isSelected: Bool = {
                        if case .computer(let host, _) = cloudChat.selectedTarget {
                            return host == env.hostPubkey
                        }
                        return false
                    }()
                    Button {
                        cloudChat.selectTarget(.computer(hostPubkey: env.hostPubkey, name: env.displayName))
                        dismiss()
                    } label: {
                        VStack(alignment: .leading, spacing: 6) {
                            HStack(spacing: 10) {
                                Circle()
                                    .fill(env.online ? OATheme.statusReady : OATheme.textMuted)
                                    .frame(width: 8, height: 8)
                                VStack(alignment: .leading, spacing: 2) {
                                    HStack(spacing: 6) {
                                        Text(env.displayName)
                                            .font(.system(size: 14, weight: .semibold))
                                            .foregroundStyle(OATheme.textPrimary)
                                        Text(env.osLabel)
                                            .font(.system(size: 10, weight: .medium, design: .monospaced))
                                            .foregroundStyle(OATheme.textSecondary)
                                            .padding(.horizontal, 6)
                                            .padding(.vertical, 2)
                                            .background(OATheme.elevated, in: Capsule())
                                    }
                                    Text(env.online ? "Online · $0 local execution" : "Offline")
                                        .font(.system(size: 12))
                                        .foregroundStyle(env.online ? OATheme.statusReady : OATheme.textMuted)
                                }
                                Spacer()
                                if isSelected {
                                    Image(systemName: "checkmark")
                                        .font(.system(size: 13, weight: .semibold))
                                        .foregroundStyle(Color.white)
                                }
                            }
                            if !env.workspaces.isEmpty {
                                VStack(alignment: .leading, spacing: 3) {
                                    ForEach(env.workspaces.prefix(4)) { ws in
                                        HStack(spacing: 6) {
                                            Image(systemName: "folder")
                                                .font(.system(size: 10))
                                                .foregroundStyle(OATheme.textMuted)
                                            Text(ws.name)
                                                .font(.system(size: 12, design: .monospaced))
                                                .foregroundStyle(OATheme.textSecondary)
                                            if !ws.branch.isEmpty {
                                                Text(ws.branch)
                                                    .font(.system(size: 11, design: .monospaced))
                                                    .foregroundStyle(OATheme.textMuted)
                                            }
                                        }
                                    }
                                }
                                .padding(.leading, 18)
                            }
                        }
                        .padding(.vertical, 4)
                    }
                    .listRowBackground(OATheme.surface)
                }
            }
        } header: {
            Text("Enrolled Computers")
        }
    }

    @ViewBuilder
    private var projectsSection: some View {
        Section {
            Button {
                cloudChat.selectProject(nil)
            } label: {
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text("No Project (General Chat)")
                            .font(.system(size: 14, weight: .medium))
                            .foregroundStyle(OATheme.textPrimary)
                        Text("Chat without project instructions or repo context")
                            .font(.system(size: 12))
                            .foregroundStyle(OATheme.textSecondary)
                    }
                    Spacer()
                    if cloudChat.selectedProjectID == nil {
                        Image(systemName: "checkmark")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(Color.white)
                    }
                }
            }
            .listRowBackground(OATheme.surface)

            if cloudChat.projects.isEmpty {
                Text("Create Projects on openagents.com to share instructions, knowledge, and GitHub repo context across devices.")
                    .font(.system(size: 12))
                    .foregroundStyle(OATheme.textMuted)
                    .listRowBackground(OATheme.surface)
            } else {
                ForEach(cloudChat.projects) { project in
                    Button {
                        cloudChat.selectProject(project.id)
                        dismiss()
                    } label: {
                        HStack {
                            VStack(alignment: .leading, spacing: 3) {
                                Text(project.name)
                                    .font(.system(size: 14, weight: .semibold))
                                    .foregroundStyle(OATheme.textPrimary)
                                if !project.repoCanonical.isEmpty {
                                    Text(project.repoCanonical)
                                        .font(.system(size: 11, design: .monospaced))
                                        .foregroundStyle(OATheme.textSecondary)
                                } else if !project.description.isEmpty {
                                    Text(project.description)
                                        .font(.system(size: 12))
                                        .foregroundStyle(OATheme.textSecondary)
                                        .lineLimit(2)
                                }
                            }
                            Spacer()
                            if cloudChat.selectedProjectID == project.id {
                                Image(systemName: "checkmark")
                                    .font(.system(size: 13, weight: .semibold))
                                    .foregroundStyle(Color.white)
                            }
                        }
                        .padding(.vertical, 2)
                    }
                    .listRowBackground(OATheme.surface)
                }
            }
        } header: {
            Text("Cloud Projects")
        }
    }

    private func targetRow(id: String, title: String, subtitle: String, selected: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 10) {
                VStack(alignment: .leading, spacing: 3) {
                    Text(title)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(OATheme.textPrimary)
                    Text(subtitle)
                        .font(.system(size: 12))
                        .foregroundStyle(OATheme.textSecondary)
                }
                Spacer()
                if selected {
                    Image(systemName: "checkmark")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(Color.white)
                }
            }
            .padding(.vertical, 2)
        }
        .accessibilityIdentifier(id)
        .listRowBackground(OATheme.surface)
    }
}
