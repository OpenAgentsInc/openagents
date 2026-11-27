// Firm reason: 6 (iOS full-screen pure-black OLED OpenAgents Login Gate shown exclusively until signed in, with Google OAuth, Apple Sign In, and Email + 6-digit code verification).
import SwiftUI

struct LoginGateView: View {
    @ObservedObject var account: MobileAccountController

    @State private var activeProvider: String?
    @State private var showEmailSheet: Bool = false

    var body: some View {
        ZStack {
            OATheme.background.ignoresSafeArea()

            VStack(spacing: 0) {
                // Top centered OpenAgents brand header
                HStack(spacing: 9) {
                    OpenAgentsPowerMark(size: 22, color: Color(white: 0.93))
                    Text("OpenAgents")
                        .font(.system(size: 22, weight: .semibold))
                        .tracking(-0.4)
                        .foregroundStyle(Color(white: 0.93))
                }
                .padding(.top, 20)
                .accessibilityElement(children: .combine)
                .accessibilityIdentifier("login.brand")

                Spacer(minLength: 32)

                // Centered minimalist OpenAgents monochrome hero
                VStack(spacing: 28) {
                    OpenAgentsHeroEmblem(size: 112)
                        .accessibilityHidden(true)

                    Text("Your agents, everywhere.")
                        .font(.system(size: 27, weight: .medium))
                        .tracking(-0.5)
                        .multilineTextAlignment(.center)
                        .foregroundStyle(Color(white: 0.93))
                        .padding(.horizontal, 24)
                        .accessibilityIdentifier("login.headline")
                }

                Spacer(minLength: 36)

                // Bottom-anchored thumb-friendly auth stack
                VStack(spacing: 12) {
                    if let err = account.errorMessage, !err.isEmpty, !showEmailSheet {
                        HStack(alignment: .top, spacing: 8) {
                            Image(systemName: "exclamationmark.circle.fill")
                                .font(.system(size: 14))
                                .foregroundStyle(OATheme.statusError)
                                .padding(.top, 1)
                            Text(err)
                                .font(.system(size: 13))
                                .foregroundStyle(Color(red: 0.98, green: 0.72, blue: 0.71))
                                .fixedSize(horizontal: false, vertical: true)
                            Spacer(minLength: 4)
                            Button {
                                account.errorMessage = nil
                            } label: {
                                Image(systemName: "xmark")
                                    .font(.system(size: 11, weight: .semibold))
                                    .foregroundStyle(OATheme.textMuted)
                            }
                            .accessibilityLabel("Dismiss error")
                        }
                        .padding(.horizontal, 14)
                        .padding(.vertical, 10)
                        .background(
                            RoundedRectangle(cornerRadius: 14, style: .continuous)
                                .fill(Color(red: 0.12, green: 0.04, blue: 0.04))
                                .overlay(
                                    RoundedRectangle(cornerRadius: 14, style: .continuous)
                                        .strokeBorder(Color(red: 0.36, green: 0.12, blue: 0.12), lineWidth: 1)
                                )
                        )
                        .padding(.bottom, 4)
                        .accessibilityIdentifier("login.error")
                    }

                    // 1. Continue with Google (solid white pill + real multicolor Google G)
                    Button {
                        startOAuth(provider: "GoogleOAuth")
                    } label: {
                        HStack(spacing: 10) {
                            if activeProvider == "GoogleOAuth" {
                                ProgressView()
                                    .tint(.black)
                                    .controlSize(.small)
                            } else {
                                GoogleGMark(size: 20)
                            }
                            Text("Continue with Google")
                                .font(.system(size: 16, weight: .semibold))
                                .foregroundStyle(Color.black)
                        }
                        .frame(maxWidth: .infinity)
                        .frame(height: 52)
                        .background(Color.white, in: Capsule())
                    }
                    .buttonStyle(LoginPillButtonStyle())
                    .disabled(account.isBusy)
                    .accessibilityIdentifier("login.google")

                    // 2. Continue with Apple (solid white pill + actual Apple logo)
                    Button {
                        startOAuth(provider: "AppleOAuth")
                    } label: {
                        HStack(spacing: 10) {
                            if activeProvider == "AppleOAuth" {
                                ProgressView()
                                    .tint(.black)
                                    .controlSize(.small)
                            } else {
                                AppleLogoMark(size: 18, color: .black)
                            }
                            Text("Continue with Apple")
                                .font(.system(size: 16, weight: .semibold))
                                .foregroundStyle(Color.black)
                        }
                        .frame(maxWidth: .infinity)
                        .frame(height: 52)
                        .background(Color.white, in: Capsule())
                    }
                    .buttonStyle(LoginPillButtonStyle())
                    .disabled(account.isBusy)
                    .accessibilityIdentifier("login.apple")

                    // 3. Personal or work email (dark bordered pill)
                    Button {
                        account.errorMessage = nil
                        account.statusMessage = nil
                        showEmailSheet = true
                    } label: {
                        Text("Personal or work email")
                            .font(.system(size: 16, weight: .medium))
                            .foregroundStyle(Color(white: 0.68))
                            .frame(maxWidth: .infinity)
                            .frame(height: 52)
                            .background(
                                Capsule()
                                    .fill(Color(red: 0.055, green: 0.055, blue: 0.055))
                                    .overlay(
                                        Capsule()
                                            .strokeBorder(Color(red: 0.173, green: 0.173, blue: 0.173), lineWidth: 1)
                                    )
                            )
                    }
                    .buttonStyle(LoginPillButtonStyle())
                    .disabled(account.isBusy)
                    .accessibilityIdentifier("login.email")

                    // Quiet legal footnote with underlined links
                    legalFootnote
                        .padding(.top, 8)
                }
                .padding(.horizontal, 20)
                .padding(.bottom, 16)
            }
            .frame(maxWidth: 460)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .preferredColorScheme(.dark)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("login.gate")
        .sheet(isPresented: $showEmailSheet) {
            EmailSignInSheet(account: account)
                .preferredColorScheme(.dark)
        }
        .onChange(of: account.isLoggedIn) { _, loggedIn in
            if loggedIn {
                showEmailSheet = false
            }
        }
        .onAppear {
            #if DEBUG
            if UserDefaults.standard.bool(forKey: "OALoginEmailSheet") {
                showEmailSheet = true
            }
            #endif
        }
    }

    private func startOAuth(provider: String) {
        activeProvider = provider
        Task {
            await account.signInWithWorkOS(provider: provider)
            activeProvider = nil
        }
    }

    private var legalFootnote: some View {
        Text(Self.makeLegalAttributedString())
            .font(.system(size: 12))
            .multilineTextAlignment(.center)
            .lineSpacing(2.5)
            .tint(Color(white: 0.62))
            .padding(.horizontal, 12)
            .accessibilityIdentifier("login.legal")
    }

    private static func makeLegalAttributedString() -> AttributedString {
        var result = AttributedString("By continuing, you agree to our ")
        result.foregroundColor = Color(white: 0.45)

        var terms = AttributedString("Terms of Service")
        terms.link = URL(string: "https://openagents.com/terms")
        terms.underlineStyle = .single
        terms.foregroundColor = Color(white: 0.62)

        var mid = AttributedString(" and ")
        mid.foregroundColor = Color(white: 0.45)

        var privacy = AttributedString("Privacy Policy")
        privacy.link = URL(string: "https://openagents.com/privacy")
        privacy.underlineStyle = .single
        privacy.foregroundColor = Color(white: 0.62)

        var end = AttributedString(".")
        end.foregroundColor = Color(white: 0.45)

        result.append(terms)
        result.append(mid)
        result.append(privacy)
        result.append(end)
        return result
    }
}

// MARK: - Email + 6-Digit Verification Code Sheet

private struct EmailSignInSheet: View {
    @ObservedObject var account: MobileAccountController
    @Environment(\.dismiss) private var dismiss

    @State private var emailDraft = ""
    @State private var codeDraft = ""
    @State private var passwordDraft = ""
    @State private var showPasswordMode = false
    @FocusState private var emailFocused: Bool
    @FocusState private var codeFocused: Bool

    var body: some View {
        NavigationStack {
            ZStack {
                OATheme.background.ignoresSafeArea()

                ScrollView {
                    VStack(alignment: .leading, spacing: 16) {
                        VStack(alignment: .leading, spacing: 6) {
                            Text(headerTitle)
                                .font(.system(size: 22, weight: .semibold))
                                .foregroundStyle(Color(white: 0.93))
                            Text(headerSubtitle)
                                .font(.system(size: 14))
                                .foregroundStyle(OATheme.textSecondary)
                        }
                        .padding(.top, 8)

                        if let err = account.errorMessage, !err.isEmpty {
                            Text(err)
                                .font(.system(size: 13))
                                .foregroundStyle(OATheme.statusError)
                                .padding(12)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(
                                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                                        .fill(Color(red: 0.12, green: 0.04, blue: 0.04))
                                        .overlay(
                                            RoundedRectangle(cornerRadius: 12, style: .continuous)
                                                .strokeBorder(Color(red: 0.36, green: 0.12, blue: 0.12), lineWidth: 1)
                                        )
                                )
                        } else if let msg = account.statusMessage, !msg.isEmpty {
                            Text(msg)
                                .font(.system(size: 13))
                                .foregroundStyle(OATheme.statusReady)
                                .padding(12)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(
                                    RoundedRectangle(cornerRadius: 12, style: .continuous)
                                        .fill(Color(red: 0.04, green: 0.11, blue: 0.06))
                                        .overlay(
                                            RoundedRectangle(cornerRadius: 12, style: .continuous)
                                                .strokeBorder(Color(red: 0.12, green: 0.30, blue: 0.18), lineWidth: 1)
                                        )
                                )
                        }

                        if !account.pendingOrgs.isEmpty {
                            orgSelectionStep
                        } else if account.awaitingEmailCode {
                            codeVerificationStep
                        } else {
                            emailEntryStep
                        }
                    }
                    .padding(.horizontal, 20)
                    .padding(.vertical, 12)
                }
            }
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .principal) {
                    HStack(spacing: 6) {
                        OpenAgentsPowerMark(size: 16, color: Color(white: 0.93))
                        Text("OpenAgents")
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundStyle(Color(white: 0.93))
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Close") {
                        dismiss()
                    }
                    .foregroundStyle(Color(white: 0.85))
                    .accessibilityIdentifier("login.email.close")
                }
            }
        }
        .preferredColorScheme(.dark)
        .onAppear {
            if account.awaitingEmailCode {
                codeFocused = true
            } else {
                emailFocused = true
            }
        }
        .onChange(of: account.awaitingEmailCode) { _, waiting in
            if waiting {
                codeDraft = ""
                codeFocused = true
            }
        }
        .onChange(of: account.isLoggedIn) { _, loggedIn in
            if loggedIn {
                dismiss()
            }
        }
    }

    private var headerTitle: String {
        if !account.pendingOrgs.isEmpty { return "Choose organization" }
        if account.awaitingEmailCode { return "Check your email" }
        return "Log in with email"
    }

    private var headerSubtitle: String {
        if !account.pendingOrgs.isEmpty {
            return "Select the workspace organization to continue."
        }
        if account.awaitingEmailCode {
            return "Enter the 6-digit verification code sent to \(account.pendingEmailAddress)."
        }
        return "We\u{2019}ll send a 6-digit verification code to your inbox."
    }

    @ViewBuilder
    private var emailEntryStep: some View {
        VStack(spacing: 12) {
            TextField(
                text: $emailDraft,
                prompt: Text(verbatim: "you@example.com").foregroundStyle(OATheme.textMuted)
            ) {
                EmptyView()
            }
            .font(.system(size: 15))
            .foregroundStyle(OATheme.textPrimary)
            .keyboardType(.emailAddress)
            .textContentType(.emailAddress)
            .autocorrectionDisabled()
            .textInputAutocapitalization(.never)
            .submitLabel(showPasswordMode ? .next : .continue)
            .focused($emailFocused)
            .onSubmit {
                if !showPasswordMode {
                    Task { await account.sendEmailCode(to: emailDraft) }
                }
            }
            .padding(.horizontal, 16)
            .frame(height: 50)
            .background(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .fill(Color(red: 0.055, green: 0.055, blue: 0.055))
                    .overlay(
                        RoundedRectangle(cornerRadius: 14, style: .continuous)
                            .strokeBorder(Color(red: 0.173, green: 0.173, blue: 0.173), lineWidth: 1)
                    )
            )
            .accessibilityIdentifier("login.email.input")

            if showPasswordMode {
                SecureField(
                    text: $passwordDraft,
                    prompt: Text("Password").foregroundStyle(OATheme.textMuted)
                ) {
                    EmptyView()
                }
                .font(.system(size: 15))
                .foregroundStyle(OATheme.textPrimary)
                .textContentType(.password)
                .submitLabel(.go)
                .onSubmit {
                    Task { await account.passwordSignIn(email: emailDraft, password: passwordDraft, register: false) }
                }
                .padding(.horizontal, 16)
                .frame(height: 50)
                .background(
                    RoundedRectangle(cornerRadius: 14, style: .continuous)
                        .fill(Color(red: 0.055, green: 0.055, blue: 0.055))
                        .overlay(
                            RoundedRectangle(cornerRadius: 14, style: .continuous)
                                .strokeBorder(Color(red: 0.173, green: 0.173, blue: 0.173), lineWidth: 1)
                        )
                )
                .accessibilityIdentifier("login.password.input")

                Button {
                    Task { await account.passwordSignIn(email: emailDraft, password: passwordDraft, register: false) }
                } label: {
                    HStack(spacing: 8) {
                        if account.isBusy { ProgressView().tint(.black).controlSize(.small) }
                        Text("Log in")
                            .font(.system(size: 16, weight: .semibold))
                            .foregroundStyle(Color.black)
                    }
                    .frame(maxWidth: .infinity)
                    .frame(height: 50)
                    .background(
                        emailDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || passwordDraft.isEmpty
                            ? Color(white: 0.35)
                            : Color.white,
                        in: Capsule()
                    )
                }
                .disabled(account.isBusy || emailDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || passwordDraft.isEmpty)
                .accessibilityIdentifier("login.password.submit")
            } else {
                Button {
                    Task { await account.sendEmailCode(to: emailDraft) }
                } label: {
                    HStack(spacing: 8) {
                        if account.isBusy { ProgressView().tint(.black).controlSize(.small) }
                        Text("Send 6-Digit Code")
                            .font(.system(size: 16, weight: .semibold))
                            .foregroundStyle(Color.black)
                    }
                    .frame(maxWidth: .infinity)
                    .frame(height: 50)
                    .background(
                        emailDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                            ? Color(white: 0.35)
                            : Color.white,
                        in: Capsule()
                    )
                }
                .disabled(account.isBusy || emailDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                .accessibilityIdentifier("login.email.send")
            }

            if account.workosEnabled {
                Button(showPasswordMode ? "Use 6-digit email code instead" : "Log in with password instead") {
                    showPasswordMode.toggle()
                }
                .font(.system(size: 13.5, weight: .medium))
                .foregroundStyle(OATheme.textSecondary)
                .padding(.top, 4)
            }
        }
    }

    @ViewBuilder
    private var codeVerificationStep: some View {
        VStack(spacing: 12) {
            TextField(
                text: $codeDraft,
                prompt: Text("6-digit code").foregroundStyle(OATheme.textMuted)
            ) {
                EmptyView()
            }
            .font(.system(size: 20, weight: .medium, design: .monospaced))
            .foregroundStyle(OATheme.textPrimary)
            .keyboardType(.numberPad)
            .textContentType(.oneTimeCode)
            .focused($codeFocused)
            .padding(.horizontal, 16)
            .frame(height: 52)
            .background(
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .fill(Color(red: 0.055, green: 0.055, blue: 0.055))
                    .overlay(
                        RoundedRectangle(cornerRadius: 14, style: .continuous)
                            .strokeBorder(Color(red: 0.173, green: 0.173, blue: 0.173), lineWidth: 1)
                    )
            )
            .accessibilityIdentifier("login.code.input")

            Button {
                Task { await account.verifyEmailCode(codeDraft) }
            } label: {
                HStack(spacing: 8) {
                    if account.isBusy { ProgressView().tint(.black).controlSize(.small) }
                    Text("Verify & Log in")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundStyle(Color.black)
                }
                .frame(maxWidth: .infinity)
                .frame(height: 50)
                .background(
                    codeDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                        ? Color(white: 0.35)
                        : Color.white,
                    in: Capsule()
                )
            }
            .disabled(account.isBusy || codeDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            .accessibilityIdentifier("login.code.verify")

            HStack(spacing: 20) {
                Button("Resend code") {
                    Task { await account.sendEmailCode(to: account.pendingEmailAddress) }
                }
                .font(.system(size: 13.5, weight: .medium))
                .foregroundStyle(OATheme.textSecondary)
                .disabled(account.isBusy)

                Button("Use another email") {
                    codeDraft = ""
                    account.cancelEmailFlow()
                }
                .font(.system(size: 13.5, weight: .medium))
                .foregroundStyle(OATheme.textSecondary)
                .disabled(account.isBusy)
            }
            .padding(.top, 6)
        }
    }

    @ViewBuilder
    private var orgSelectionStep: some View {
        VStack(spacing: 10) {
            ForEach(account.pendingOrgs, id: \.id) { org in
                Button {
                    Task { await account.selectOrganization(org.id) }
                } label: {
                    HStack {
                        Text(org.name)
                            .font(.system(size: 15, weight: .medium))
                            .foregroundStyle(OATheme.textPrimary)
                        Spacer()
                        Image(systemName: "chevron.right")
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(OATheme.textMuted)
                    }
                    .padding(.horizontal, 16)
                    .frame(height: 50)
                    .background(
                        RoundedRectangle(cornerRadius: 14, style: .continuous)
                            .fill(OATheme.surface)
                            .overlay(
                                RoundedRectangle(cornerRadius: 14, style: .continuous)
                                    .strokeBorder(OATheme.hairline, lineWidth: 1)
                            )
                    )
                }
                .disabled(account.isBusy)
            }

            Button("Start over") {
                account.cancelEmailFlow()
            }
            .font(.system(size: 13.5, weight: .medium))
            .foregroundStyle(OATheme.textSecondary)
            .padding(.top, 4)
        }
    }
}

// MARK: - Press Feedback & Crisp Vector Brand Marks

private struct LoginPillButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed ? 0.985 : 1.0)
            .opacity(configuration.isPressed ? 0.88 : 1.0)
            .animation(.easeOut(duration: 0.12), value: configuration.isPressed)
    }
}

/// Official OpenAgents monochrome power symbol matching `components/icons/psionic-mark.svg`.
struct OpenAgentsPowerMark: View {
    var size: CGFloat = 22
    var color: Color = Color(white: 0.93)

    var body: some View {
        Canvas { ctx, sz in
            let s = min(sz.width, sz.height)
            let cx = sz.width / 2
            let cy = sz.height / 2
            let strokeW = s * (42.0 / 386.0)
            let radius = s * (160.0 / 386.0)

            // Open power arc (~305 degrees around the bottom, leaving top open for the vertical stem)
            var arc = Path()
            arc.addArc(
                center: CGPoint(x: cx, y: cy),
                radius: radius,
                startAngle: .degrees(-56),
                endAngle: .degrees(236),
                clockwise: false
            )
            ctx.stroke(
                arc,
                with: .color(color),
                style: StrokeStyle(lineWidth: strokeW, lineCap: .round)
            )

            // Vertical stem centered at top
            var stem = Path()
            let stemTop = cy - s * (172.0 / 386.0)
            let stemBottom = cy - s * (27.5 / 386.0)
            stem.move(to: CGPoint(x: cx, y: stemTop))
            stem.addLine(to: CGPoint(x: cx, y: stemBottom))
            ctx.stroke(
                stem,
                with: .color(color),
                style: StrokeStyle(lineWidth: strokeW, lineCap: .round)
            )
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}

/// Minimalist monochrome OpenAgents orbital hero emblem for the full-screen Login Gate.
private struct OpenAgentsHeroEmblem: View {
    var size: CGFloat = 112

    var body: some View {
        ZStack {
            // Subtle outer orbital halo with two crisp geometric ticks
            Canvas { ctx, sz in
                let s = min(sz.width, sz.height)
                let center = CGPoint(x: sz.width / 2, y: sz.height / 2)
                let outerR = s * 0.47

                var halo = Path()
                halo.addArc(center: center, radius: outerR, startAngle: .degrees(20), endAngle: .degrees(160), clockwise: false)
                ctx.stroke(halo, with: .color(Color(white: 0.93).opacity(0.20)), style: StrokeStyle(lineWidth: 1.25, lineCap: .round))

                var halo2 = Path()
                halo2.addArc(center: center, radius: outerR, startAngle: .degrees(200), endAngle: .degrees(340), clockwise: false)
                ctx.stroke(halo2, with: .color(Color(white: 0.93).opacity(0.20)), style: StrokeStyle(lineWidth: 1.25, lineCap: .round))

                // Quiet orbital nodes on left/right equator
                for deg in [0.0, 180.0] {
                    let rad = deg * .pi / 180.0
                    let pt = CGPoint(x: center.x + cos(rad) * outerR, y: center.y + sin(rad) * outerR)
                    let dotR: CGFloat = 2.5
                    ctx.fill(
                        Path(ellipseIn: CGRect(x: pt.x - dotR, y: pt.y - dotR, width: dotR * 2, height: dotR * 2)),
                        with: .color(Color(white: 0.93).opacity(0.55))
                    )
                }
            }

            OpenAgentsPowerMark(size: size * 0.62, color: Color(white: 0.94))
        }
        .frame(width: size, height: size)
    }
}

/// Real Apple logo vector matching Apple's official SVG silhouette (`viewBox 0 0 814 1000`).
struct AppleLogoMark: View {
    var size: CGFloat = 18
    var color: Color = .black

    var body: some View {
        Image(systemName: "apple.logo")
            .resizable()
            .aspectRatio(contentMode: .fit)
            .frame(width: size, height: size)
            .foregroundStyle(color)
            .accessibilityHidden(true)
    }
}

/// Real 4-color Google "G" vector mark (`viewBox 0 0 24 24`, `translate(1.575 2) scale(1.25)`).
struct GoogleGMark: View {
    var size: CGFloat = 20

    var body: some View {
        Canvas { ctx, sz in
            let s = min(sz.width, sz.height) / 24.0
            let ox = (sz.width - 24.0 * s) / 2.0
            let oy = (sz.height - 24.0 * s) / 2.0
            func pt(_ x: CGFloat, _ y: CGFloat) -> CGPoint {
                CGPoint(x: ox + (1.575 + x * 1.25) * s, y: oy + (2.0 + y * 1.25) * s)
            }

            // Blue (#4285F4)
            var blue = Path()
            blue.move(to: pt(16.18, 8.1818))
            blue.addCurve(to: pt(16.0345, 6.54544), control1: pt(16.18, 7.61453), control2: pt(16.1291, 7.06908))
            blue.addLine(to: pt(8.5, 6.54544))
            blue.addLine(to: pt(8.5, 9.63999))
            blue.addLine(to: pt(12.8055, 9.63999))
            blue.addCurve(to: pt(11.2091, 12.0545), control1: pt(12.62, 10.64), control2: pt(12.0564, 11.4873))
            blue.addLine(to: pt(11.2091, 14.0618))
            blue.addLine(to: pt(13.7945, 14.0618))
            blue.addCurve(to: pt(16.18, 8.1818), control1: pt(15.3073, 12.6691), control2: pt(16.18, 10.6182))
            blue.closeSubpath()
            ctx.fill(blue, with: .color(Color(red: 0x42 / 255.0, green: 0x85 / 255.0, blue: 0xF4 / 255.0)))

            // Green (#34A853)
            var green = Path()
            green.move(to: pt(8.49992, 16.0))
            green.addCurve(to: pt(13.7945, 14.0618), control1: pt(10.6599, 16.0), control2: pt(12.4708, 15.2836))
            green.addLine(to: pt(11.209, 12.0545))
            green.addCurve(to: pt(8.49992, 12.8182), control1: pt(10.4926, 12.5345), control2: pt(9.57629, 12.8182))
            green.addCurve(to: pt(4.02356, 9.51999), control1: pt(6.41629, 12.8182), control2: pt(4.65265, 11.4109))
            green.addLine(to: pt(1.35083, 9.51999))
            green.addLine(to: pt(1.35083, 11.5927))
            green.addCurve(to: pt(8.49992, 16.0), control1: pt(2.66719, 14.2073), control2: pt(5.37265, 16.0))
            green.closeSubpath()
            ctx.fill(green, with: .color(Color(red: 0x34 / 255.0, green: 0xA8 / 255.0, blue: 0x53 / 255.0)))

            // Yellow (#FBBC05)
            var yellow = Path()
            yellow.move(to: pt(4.02364, 9.52001))
            yellow.addCurve(to: pt(3.77273, 8.00001), control1: pt(3.86364, 9.04001), control2: pt(3.77273, 8.52729))
            yellow.addCurve(to: pt(4.02364, 6.48001), control1: pt(3.77273, 7.47274), control2: pt(3.86364, 6.96001))
            yellow.addLine(to: pt(4.02364, 4.40729))
            yellow.addLine(to: pt(1.35091, 4.40729))
            yellow.addCurve(to: pt(0.5, 8.00001), control1: pt(0.809091, 5.48729), control2: pt(0.5, 6.70911))
            yellow.addCurve(to: pt(1.35091, 11.5927), control1: pt(0.5, 9.29092), control2: pt(0.809091, 10.5127))
            yellow.addLine(to: pt(4.02364, 9.52001))
            yellow.closeSubpath()
            ctx.fill(yellow, with: .color(Color(red: 0xFB / 255.0, green: 0xBC / 255.0, blue: 0x05 / 255.0)))

            // Red (#EA4335)
            var red = Path()
            red.move(to: pt(8.49992, 3.18182))
            red.addCurve(to: pt(11.5581, 4.37818), control1: pt(9.67447, 3.18182), control2: pt(10.729, 3.58545))
            red.addLine(to: pt(13.8526, 2.08364))
            red.addCurve(to: pt(8.49992, 0.0), control1: pt(12.4672, 0.792727), control2: pt(10.6563, 0.0))
            red.addCurve(to: pt(1.35083, 4.40727), control1: pt(5.37265, 0.0), control2: pt(2.66719, 1.79273))
            red.addLine(to: pt(4.02356, 6.48))
            red.addCurve(to: pt(8.49992, 3.18182), control1: pt(4.65265, 4.58909), control2: pt(6.41629, 3.18182))
            red.closeSubpath()
            ctx.fill(red, with: .color(Color(red: 0xEA / 255.0, green: 0x43 / 255.0, blue: 0x35 / 255.0)))
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}
