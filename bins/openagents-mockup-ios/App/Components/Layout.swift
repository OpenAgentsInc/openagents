import SwiftUI

/// Step dots: ● ● ○ for the first run's "step n of 3".
struct StepDots: View {
    let step: Int
    var total = 3

    var body: some View {
        HStack(spacing: 6) {
            ForEach(1...total, id: \.self) { i in
                Circle()
                    .fill(i <= step ? Theme.Colors.textPrimary : .clear)
                    .overlay(Circle().stroke(Theme.Colors.textPrimary.opacity(i <= step ? 0 : 0.5), lineWidth: 1.5))
                    .frame(width: Theme.Size.stepDot, height: Theme.Size.stepDot)
            }
        }
        .accessibilityLabel("Step \(step) of \(total)")
    }
}

/// The top-left back control, e.g. "< Menu".
struct BackControl {
    let label: String
    let action: () -> Void
}

/// The top bar: back control, condensed title, step dots or a trailing control.
struct TopBar<Trailing: View>: View {
    var back: BackControl? = nil
    let title: String
    var step: Int? = nil
    @ViewBuilder var trailing: Trailing

    var body: some View {
        ZStack {
            Text(title).condensedTitle(Theme.Fonts.screenTitle, tracking: 1.2)
                .foregroundStyle(Theme.Colors.textPrimary)
            HStack {
                if let back {
                    Button(action: back.action) {
                        HStack(spacing: 4) {
                            Image(systemName: "chevron.left").font(.system(size: 16, weight: .bold))
                            Text(back.label).font(Theme.Fonts.bodyBold)
                        }
                        .foregroundStyle(Theme.Colors.textPrimary)
                        .frame(minHeight: 44)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(PressStyle())
                }
                Spacer()
                if let step { StepDots(step: step) }
                trailing
            }
        }
        .padding(.horizontal, Theme.Space.page)
        .frame(height: Theme.Size.topBarHeight)
        .overlay(alignment: .bottom) { Rectangle().fill(Theme.Colors.divider).frame(height: 1) }
    }
}

extension TopBar where Trailing == EmptyView {
    init(back: BackControl? = nil, title: String, step: Int? = nil) {
        self.init(back: back, title: title, step: step) { EmptyView() }
    }
}

/// The spec's "Next:" line (CHK-03).
struct NextLine: View {
    let text: String

    var body: some View {
        HStack(spacing: 6) {
            Text("Next:").font(Theme.Fonts.bodyBold).foregroundStyle(Theme.Colors.textPrimary)
            Text(text).font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .fixedSize(horizontal: false, vertical: true)
    }
}

/// Small uppercase section label: RECOMMENDED, YOUR RUNS.
struct SectionLabel: View {
    let text: String

    var body: some View {
        Text(text)
            .condensedTitle(Theme.Fonts.sectionLabel, tracking: Theme.Tracking.sectionLabel)
            .foregroundStyle(Theme.Colors.textTertiary)
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}

/// A standard screen: top bar, scrolling content, and a pinned bottom area
/// for the primary action.
struct ScreenScaffold<Content: View, Bottom: View, TopBarView: View>: View {
    @ViewBuilder var topBar: TopBarView
    @ViewBuilder var content: Content
    @ViewBuilder var bottom: Bottom

    var body: some View {
        VStack(spacing: 0) {
            topBar
            ScrollView {
                VStack(alignment: .leading, spacing: Theme.Space.m) {
                    content
                }
                .padding(.horizontal, Theme.Space.page)
                .padding(.vertical, Theme.Space.l)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .scrollIndicators(.hidden)
            VStack(spacing: Theme.Space.xs) {
                bottom
            }
            .padding(.horizontal, Theme.Space.page)
            .padding(.top, Theme.Space.s)
            .padding(.bottom, Theme.Space.xs)
            .background(
                Theme.Colors.background
                    .overlay(alignment: .top) { Rectangle().fill(Theme.Colors.divider).frame(height: 1) }
            )
        }
        .background(Theme.Colors.background.ignoresSafeArea())
        .foregroundStyle(Theme.Colors.textPrimary)
    }
}

/// A screen outside this spec (Wallet, Your computers, Identity keys…).
struct StubScreen: View {
    @Environment(MockApp.self) private var app
    let name: String

    var body: some View {
        ScreenScaffold {
            TopBar(back: BackControl(label: "Back") { app.back() }, title: name)
        } content: {
            VStack(spacing: Theme.Space.m) {
                Image(systemName: "square.dashed").font(.system(size: 54, weight: .light))
                    .foregroundStyle(Theme.Colors.textTertiary)
                Text(name).font(Theme.Fonts.title)
                Text("This screen exists in the real app but isn't part of the wireframe spec, so the mockup leaves it blank.")
                    .font(Theme.Fonts.body).foregroundStyle(Theme.Colors.textSecondary)
                    .multilineTextAlignment(.center)
            }
            .frame(maxWidth: .infinity)
            .padding(.top, 80)
        } bottom: {
            PrimaryButton(title: "Back") { app.back() }
        }
    }
}

#Preview("Layout") {
    ScreenScaffold {
        TopBar(back: BackControl(label: "Menu") {}, title: "THE GYM", step: 2)
    } content: {
        Text("Give Coder a new tool.").font(Theme.Fonts.title)
        SectionLabel(text: "Recommended")
        NextLine(text: "tap Start training.")
    } bottom: {
        PrimaryButton(title: "Start training") {}
    }
    .environment(MockApp())
}
