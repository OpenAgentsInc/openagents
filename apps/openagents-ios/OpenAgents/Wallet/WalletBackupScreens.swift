import UIKit

/// The wallet screen's backup card (docs/auth/wallets-and-custody.md, "Recovery when the device is
/// lost"; #13217). It asks the moment the wallet first holds money: one tap backs the wallet up
/// with the account's passkey. Then it says the wallet is backed up, or that the account's backup
/// isn't this wallet's or no longer works. verse/wallet-backup.coax decides which
/// (verse_backup_card); WalletBackupFlow does the work.
@MainActor
final class WalletBackupCardView: UIView {
    /// "Not now" hides the offer until the app starts again.
    static var notNow = false

    private let flow: WalletBackupFlow
    /// Presents a sheet from the wallet screen.
    var present: ((UIViewController) -> Void)?
    /// Opens the wallet's safety screen (the daily limit and the backup).
    var openSafety: (() -> Void)?
    /// The card changed size.
    var changed: (() -> Void)?

    private(set) var phase: Int64 = 0
    private var balance: UInt64 = 0
    private(set) var summary: WalletBackup.Summary?
    private var busy = false

    private let heading = Look.label("", Look.title(15))
    let about = Look.label("", .preferredFont(forTextStyle: .subheadline), Look.dim)
    let note = Look.label("", .preferredFont(forTextStyle: .footnote), Look.warn)
    let action = Look.button("Back up", "backup.action", primary: true)
    private let later = Look.button("Not now", "backup.later")
    private lazy var row = UIStackView(arrangedSubviews: [action, later])
    /// Always there while logged in: the daily limit and what the last 24 hours counted.
    let limit = Look.button("Daily limit", "wallet.limit")
    private let column = UIStackView()
    private let card = UIView()

    init(flow: WalletBackupFlow) {
        self.flow = flow
        super.init(frame: .zero)
        accessibilityIdentifier = "backup.card"
        card.backgroundColor = UIColor(white: 1, alpha: 0.06)
        card.layer.cornerRadius = 12; card.layer.borderWidth = 1; card.layer.borderColor = Look.line.cgColor
        column.axis = .vertical; column.spacing = 10; column.translatesAutoresizingMaskIntoConstraints = false
        card.addSubview(column)
        NSLayoutConstraint.activate([
            column.topAnchor.constraint(equalTo: card.topAnchor, constant: 16), column.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -16),
            column.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 16), column.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -16),
        ])
        row.spacing = 12; row.distribution = .fillEqually
        heading.accessibilityIdentifier = "backup.heading"; about.accessibilityIdentifier = "backup.about"; note.accessibilityIdentifier = "backup.note"
        [heading, about, note, row].forEach(column.addArrangedSubview)
        let outer = Look.stack([card, limit], spacing: 10); outer.translatesAutoresizingMaskIntoConstraints = false
        addSubview(outer)
        NSLayoutConstraint.activate([outer.topAnchor.constraint(equalTo: topAnchor), outer.bottomAnchor.constraint(equalTo: bottomAnchor),
                                     outer.leadingAnchor.constraint(equalTo: leadingAnchor), outer.trailingAnchor.constraint(equalTo: trailingAnchor)])
        action.addAction(UIAction { [weak self] _ in self?.act() }, for: .touchUpInside)
        later.addAction(UIAction { [weak self] _ in Self.notNow = true; self?.show() }, for: .touchUpInside)
        limit.addAction(UIAction { [weak self] _ in self?.openSafety?() }, for: .touchUpInside)
        show()
    }
    required init?(coder: NSCoder) { fatalError() }

    /// The wallet's balance changed, or the screen opened: ask the account and show what is true.
    func load(balance: UInt64) async {
        self.balance = balance
        if flow.client.isLoggedIn { summary = try? await flow.client.summary() } else { summary = nil }
        if let daily = summary?.limit.daily { flow.ledger.limit = daily }
        show()
    }

    private func show() {
        let backup = summary?.backup
        phase = verse_backup_card(Int64(min(balance, UInt64(Int64.max))), flow.client.isLoggedIn, backup != nil, backup?.counts ?? false,
                                  backup != nil && backup?.vault == flow.memory.vault, Self.notNow)
        card.isHidden = phase == 0
        later.isHidden = !(phase == 1 || phase == 2)
        note.isHidden = (note.text ?? "").isEmpty
        switch phase {
        case 1, 2:
            heading.text = "Back up your wallet"
            about.text = "Your wallet has \(WalletBackup.sats(Int64(balance))) sats. If you lose this phone, you need a backup to get them back."
            action.configuration?.title = phase == 1 ? "Back up" : "Log in to back up"
        case 3:
            heading.text = "Backed up"
            about.text = "Your passkey or your recovery code brings this wallet back on a new phone. We can't open the backup."
            action.configuration?.title = "Wallet safety"
        case 4:
            heading.text = "This wallet isn't backed up"
            about.text = "Your account has a backup that this phone didn't make. Back up this wallet to replace it."
            action.configuration?.title = "Back up this wallet"
        case 5:
            heading.text = "Back up again"
            about.text = "Your backup's passkey was removed from your account. Only your recovery code opens it now. Back up again to bring your wallet back with a passkey."
            action.configuration?.title = "Back up again"
        default: break
        }
        if flow.client.isLoggedIn, let l = summary?.limit {
            limit.isHidden = false
            limit.configuration?.title = "Daily limit \(WalletBackup.sats(l.daily)) sats"
            limit.configuration?.subtitle = "\(WalletBackup.sats(l.spent)) sats in the last 24 hours" + (l.raiseTo.map { " · going up to \(WalletBackup.sats($0))" } ?? "")
        } else { limit.isHidden = true }
        isHidden = card.isHidden && limit.isHidden
        changed?()
    }

    private func act() {
        guard !busy else { return }
        switch phase {
        case 2:
            let nav = LoginSheet(rootViewController: SignInViewController())
            nav.overrideUserInterfaceStyle = .dark
            nav.closed = { [weak self] in Task { await self?.load(balance: self?.balance ?? 0) } }
            present?(nav)
        case 3: openSafety?()
        case 1, 4, 5: backUp()
        default: break
        }
    }
    /// One tap: the passkey prompt, then the recovery code to write down.
    func backUp() {
        busy = true; action.isEnabled = false; action.configuration?.showsActivityIndicator = true
        note.text = nil
        Task { @MainActor in
            defer { busy = false; action.isEnabled = true; action.configuration?.showsActivityIndicator = false }
            do {
                let words = try await flow.backUp()
                UINotificationFeedbackGenerator().notificationOccurred(.success)
                let sheet = UINavigationController(rootViewController: RecoveryCodeViewController(words: words))
                sheet.overrideUserInterfaceStyle = .dark; sheet.isModalInPresentation = true
                present?(sheet)
            } catch {
                note.text = error.localizedDescription
            }
            await load(balance: balance)
        }
    }
}

/// The recovery code, shown once: 24 words made on this phone. They aren't stored here or sent
/// anywhere, so this screen is the only time they can be written down.
final class RecoveryCodeViewController: WalletForm {
    private let words: String
    init(words: String) { self.words = words; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { fatalError() }
    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Your recovery code"
        let about = Look.label("Your wallet is backed up. Write these 24 words down and keep them somewhere safe. If you lose your passkey, they are the only way to open the backup.", .preferredFont(forTextStyle: .body))
        let grid = Look.stack([], spacing: 6)
        let list = words.split(separator: " ")
        for r in stride(from: 0, to: list.count, by: 2) {
            let line = UIStackView(); line.distribution = .fillEqually; line.spacing = 12
            for i in r..<min(r + 2, list.count) {
                let l = Look.label("\(i + 1). \(list[i])", .monospacedSystemFont(ofSize: 17, weight: .medium))
                l.accessibilityIdentifier = "code.word.\(i + 1)"
                line.addArrangedSubview(l)
            }
            grid.addArrangedSubview(line)
        }
        grid.accessibilityIdentifier = "code.words"
        let once = Look.label("We can't show these words again: they were made on this phone and we never get them. We never ask for them on our website. Anyone who has them and your account can open your wallet.", .preferredFont(forTextStyle: .footnote), Look.warn)
        let done = Look.button("I wrote them down", "code.done", primary: true)
        done.addAction(UIAction { [weak self] _ in self?.dismiss(animated: true) }, for: .touchUpInside)
        [about, grid, once, done].forEach(column.addArrangedSubview)
        column.setCustomSpacing(22, after: grid)
    }
}

/// Bringing the wallet back with the recovery code, for a phone whose passkey can't open the backup.
final class RestoreCodeViewController: WalletForm {
    private let flow: WalletBackupFlow
    private let field = UITextView()
    private let restore = Look.button("Bring my wallet back", "restore.go", primary: true)
    /// The wallet is on this phone again.
    var restored: (() -> Void)?
    init(flow: WalletBackupFlow) { self.flow = flow; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { fatalError() }
    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Recovery code"
        navigationItem.leftBarButtonItem = UIBarButtonItem(systemItem: .cancel, primaryAction: UIAction { [weak self] _ in self?.dismiss(animated: true) })
        let about = Look.label("Type the 24 words of your wallet's recovery code. They open your backup on this phone and are never sent to us.", .preferredFont(forTextStyle: .body))
        field.backgroundColor = UIColor(white: 1, alpha: 0.07); field.textColor = Look.ink; field.font = .monospacedSystemFont(ofSize: 17, weight: .regular)
        field.layer.cornerRadius = 8; field.layer.borderColor = Look.line.cgColor; field.layer.borderWidth = 1
        field.autocapitalizationType = .none; field.autocorrectionType = .no; field.spellCheckingType = .no
        field.heightAnchor.constraint(equalToConstant: 160).isActive = true
        field.accessibilityIdentifier = "restore.words"
        restore.addAction(UIAction { [weak self] _ in self?.go() }, for: .touchUpInside)
        [about, field, restore].forEach(column.addArrangedSubview)
    }
    private func go() {
        let typed = field.text ?? ""
        field.resignFirstResponder()
        busy(restore) { [self] in
            do {
                // Reading the backup needs a fresh proof that it is you: the passkey, when the account has one.
                do { _ = try await flow.restore(.words(typed)) ? done() : fail("This account has no wallet backup.") }
                catch WalletBackup.Failure.stepUp {
                    _ = try await flow.prove()
                    _ = try await flow.restore(.words(typed)) ? done() : fail("This account has no wallet backup.")
                }
            } catch { fail(error) }
        }
    }
    private func done() {
        UINotificationFeedbackGenerator().notificationOccurred(.success)
        restored?()
        dismiss(animated: true)
    }
}

/// Wallet safety: the daily limit (shown, changeable), the backup and its slots, and the opt-in
/// to use the wallet's words as a Nostr identity. Every change goes through the server's rules
/// (coax/web/api/wallet-policy.coax); a raise or a change to the backup asks for the passkey.
final class WalletSafetyViewController: WalletForm {
    private let flow: WalletBackupFlow
    private var summary: WalletBackup.Summary?
    private let limitLine = Look.label("", Look.title(20))
    private let spentLine = Look.label("", .preferredFont(forTextStyle: .subheadline), Look.dim)
    private let raiseLine = Look.label("", .preferredFont(forTextStyle: .subheadline), Look.warn)
    private let cancelRaise = Look.button("Cancel the raise", "limit.cancel")
    private let field = Look.field("New daily limit in sats", "limit.field")
    private let set = Look.button("Set limit", "limit.set", primary: true)
    private let limitNote = Look.label("", .preferredFont(forTextStyle: .footnote), Look.ink)
    private let backupLine = Look.label("", .preferredFont(forTextStyle: .body))
    private let slots = Look.stack([], spacing: 4)
    private let again = Look.button("Back up again", "backup.again")
    private let remove = Look.button("Delete backup", "backup.delete")
    private let withCode = Look.button("Bring a wallet back with a recovery code", "restore.open")
    private let oneSeed = UISwitch()
    private let oneSeedNote = Look.label("", .preferredFont(forTextStyle: .footnote), Look.dim)
    /// The wallet came back, or the backup changed.
    var changed: (() -> Void)?

    init(flow: WalletBackupFlow) { self.flow = flow; super.init(nibName: nil, bundle: nil) }
    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = "Wallet safety"
        limitLine.accessibilityIdentifier = "limit.value"; spentLine.accessibilityIdentifier = "limit.spent"; raiseLine.accessibilityIdentifier = "limit.raise"
        limitNote.accessibilityIdentifier = "limit.note"; backupLine.accessibilityIdentifier = "backup.state"
        field.keyboardType = .numberPad
        let how = Look.label("Lowering counts at once. Raising takes 24 hours, and every way you log in is told, so you can cancel a raise you didn't ask for.", .preferredFont(forTextStyle: .footnote), Look.dim)
        let honest = Look.label("Your wallet's keys are on this phone. The app checks this limit before it pays and asks for your passkey to pay more. We can't stop a payment made with your wallet's words somewhere else.", .preferredFont(forTextStyle: .footnote), Look.dim)
        let backupHead = Look.label("Backup", Look.title(15))
        let backupHow = Look.label("Only your passkey or your recovery code opens the backup. We can't, and we never ask for the recovery code on our website.", .preferredFont(forTextStyle: .footnote), Look.dim)
        let seedHead = Look.label("Nostr identity", Look.title(15))
        let seedTitle = Look.label("Use my wallet words for my Nostr identity")
        let seedRow = UIStackView(arrangedSubviews: [seedTitle, oneSeed]); seedRow.spacing = 12; seedRow.alignment = .center
        oneSeed.accessibilityIdentifier = "oneseed.switch"
        oneSeed.addAction(UIAction { [weak self] _ in self?.toggleOneSeed() }, for: .valueChanged)
        set.addAction(UIAction { [weak self] _ in self?.setLimit() }, for: .touchUpInside)
        cancelRaise.addAction(UIAction { [weak self] _ in self?.cancel() }, for: .touchUpInside)
        again.addAction(UIAction { [weak self] _ in self?.backUp() }, for: .touchUpInside)
        remove.addAction(UIAction { [weak self] _ in self?.deleteBackup() }, for: .touchUpInside)
        withCode.addAction(UIAction { [weak self] _ in self?.openRestore() }, for: .touchUpInside)
        [Look.label("Daily limit", Look.title(15)), limitLine, spentLine, raiseLine, cancelRaise, field, set, limitNote, how, honest,
         backupHead, backupLine, slots, again, remove, withCode, backupHow, seedHead, seedRow, oneSeedNote].forEach(column.addArrangedSubview)
        column.setCustomSpacing(28, after: honest); column.setCustomSpacing(28, after: backupHow)
        render()
        Task { await load() }
    }

    private func load() async {
        do { summary = try await flow.client.summary() } catch { limitNote.text = error.localizedDescription }
        if let daily = summary?.limit.daily { flow.ledger.limit = daily }
        render()
    }
    private func render() {
        let l = summary?.limit ?? WalletBackup.Limit()
        limitLine.text = "\(WalletBackup.sats(l.daily)) sats a day"
        spentLine.text = "\(WalletBackup.sats(l.spent)) sats counted in the last 24 hours"
        if let to = l.raiseTo, let at = l.raiseAt {
            let f = DateFormatter(); f.dateStyle = .medium; f.timeStyle = .short
            raiseLine.text = "Goes up to \(WalletBackup.sats(to)) sats on \(f.string(from: at))."
        } else { raiseLine.text = nil }
        raiseLine.isHidden = l.raiseTo == nil; cancelRaise.isHidden = l.raiseTo == nil
        limitNote.isHidden = (limitNote.text ?? "").isEmpty
        slots.arrangedSubviews.forEach { $0.removeFromSuperview() }
        let hasWallet = flow.seeds.read() != nil
        if let b = summary?.backup {
            let mine = b.vault == flow.memory.vault
            backupLine.text = !b.counts ? "Your backup's passkey was removed from your account. Only your recovery code opens it now."
                : mine ? "This wallet is backed up." : hasWallet ? "Your account has a backup that this phone didn't make." : "Your account has a wallet backup."
            backupLine.textColor = b.counts && mine ? Look.good : Look.warn
            for s in b.slots { slots.addArrangedSubview(Look.label(WalletBackup.slotLine(s), .preferredFont(forTextStyle: .subheadline), Look.dim)) }
        } else {
            backupLine.text = hasWallet ? "This wallet isn't backed up." : "No backup."
            backupLine.textColor = Look.warn
        }
        again.configuration?.title = summary?.backup == nil ? "Back up" : "Back up again"
        again.isHidden = !hasWallet; remove.isHidden = summary?.backup == nil
        withCode.isHidden = hasWallet || summary?.backup == nil
        oneSeed.isEnabled = hasWallet
        oneSeed.isOn = flow.memory.oneSeedKey != nil
        oneSeedNote.text = flow.memory.oneSeedKey != nil
            ? "On. Your wallet's 12 or 24 words are also a way to log in to your account. Anyone who gets them has your money and your identity."
            : "Off. Your wallet's words and your login stay separate."
    }

    private func setLimit() {
        guard let want = Int64((field.text ?? "").filter { $0.isNumber }), (field.text ?? "").contains(where: { $0.isNumber }) else { limitNote.text = "Type the new limit in sats."; render(); return }
        let current = summary?.limit.daily ?? 20_000
        field.resignFirstResponder()
        busy(set) { [self] in
            do {
                do { limitNote.text = try await flow.client.setLimit(want, current: current) }
                catch WalletBackup.Failure.stepUp {
                    _ = try await flow.prove()
                    limitNote.text = try await flow.client.setLimit(want, current: current)
                }
                field.text = ""
            } catch { limitNote.text = error.localizedDescription }
            await load()
        }
    }
    private func cancel() {
        busy(cancelRaise) { [self] in
            do { try await flow.client.cancelRaise(); limitNote.text = "The raise was cancelled." } catch { limitNote.text = error.localizedDescription }
            await load()
        }
    }
    private func backUp() {
        busy(again) { [self] in
            do {
                let words = try await flow.backUp()
                let sheet = UINavigationController(rootViewController: RecoveryCodeViewController(words: words))
                sheet.overrideUserInterfaceStyle = .dark; sheet.isModalInPresentation = true
                present(sheet, animated: true)
                changed?()
            } catch { fail(error) }
            await load()
        }
    }
    private func deleteBackup() {
        let ask = UIAlertController(title: "Delete your backup?", message: "If you lose this phone after that, nothing brings your wallet back.", preferredStyle: .alert)
        ask.addAction(UIAlertAction(title: "Keep it", style: .cancel))
        ask.addAction(UIAlertAction(title: "Delete", style: .destructive) { [weak self] _ in self?.reallyDelete() })
        present(ask, animated: true)
    }
    private func reallyDelete() {
        busy(remove) { [self] in
            do {
                do { try await flow.client.deleteBackup() }
                catch WalletBackup.Failure.stepUp { _ = try await flow.prove(); try await flow.client.deleteBackup() }
                changed?()
            } catch { fail(error) }
            await load()
        }
    }
    private func openRestore() {
        let screen = RestoreCodeViewController(flow: flow)
        screen.restored = { [weak self] in self?.changed?(); Task { await self?.load() } }
        let sheet = UINavigationController(rootViewController: screen); sheet.overrideUserInterfaceStyle = .dark
        present(sheet, animated: true)
    }
    private func toggleOneSeed() {
        let on = oneSeed.isOn
        let run = { [self] in
            oneSeed.isEnabled = false
            Task { @MainActor in
                do { if on { try await flow.useWalletWordsForNostr() } else { try await flow.stopUsingWalletWordsForNostr() } }
                catch { fail(error) }
                render()
            }
        }
        // The warning, once, before the wallet's words also become a way in.
        guard on, !flow.memory.warnedOneSeed else { run(); return }
        let warn = UIAlertController(title: "One set of words for everything",
                                     message: "Your Nostr identity will come from your wallet's words. One backup then covers both, and anyone who gets those words gets your money and your identity. You can turn this off later.",
                                     preferredStyle: .alert)
        warn.addAction(UIAlertAction(title: "Keep them separate", style: .cancel) { [weak self] _ in self?.render() })
        warn.addAction(UIAlertAction(title: "Use my wallet words", style: .default) { [weak self] _ in self?.flow.memory.warnedOneSeed = true; run() })
        present(warn, animated: true)
    }
}
