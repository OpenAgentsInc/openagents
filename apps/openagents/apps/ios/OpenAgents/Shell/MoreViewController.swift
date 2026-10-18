import UIKit
import UserNotifications

/// Settings & utilities: account, devices, appearance, archive, diagnostics.
final class MoreViewController: UIViewController, UICollectionViewDelegate {
    /// "Version 1.0.0 (1013)" under About.
    private static var versionText: String {
        let info = Bundle.main.infoDictionary
        let version = info?["CFBundleShortVersionString"] as? String ?? ""
        let build = info?["CFBundleVersion"] as? String ?? ""
        return build.isEmpty ? "Version \(version)" : "Version \(version) (\(build))"
    }

    private let app: AppModel
    private var collectionView: UICollectionView!
    private var dataSource: UICollectionViewDiffableDataSource<String, Row>!

    struct Row: Hashable {
        let id: String
        let title: String
        var subtitle: String?
        var symbol: String
        var tint: UIColor = Palette.secondary
        var accessory: Accessory = .disclosure
        var destructive = false

        enum Accessory: Hashable {
            case disclosure
            case none
            case check(Bool)
            case dot(Bool)
            case toggle(Bool)
        }
    }

    init(app: AppModel) {
        self.app = app
        super.init(nibName: nil, bundle: nil)
        title = "Settings"
    }

    required init?(coder: NSCoder) { fatalError() }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = Palette.background
        navigationItem.largeTitleDisplayMode = .always
        var config = UICollectionLayoutListConfiguration(appearance: .insetGrouped)
        config.backgroundColor = .clear
        config.headerMode = .supplementary
        collectionView = UICollectionView(frame: view.bounds, collectionViewLayout: UICollectionViewCompositionalLayout.list(using: config))
        collectionView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        collectionView.backgroundColor = .clear
        collectionView.delegate = self
        view.addSubview(collectionView)

        let cell = UICollectionView.CellRegistration<UICollectionViewListCell, Row> { [weak self] cell, _, row in
            var c = UIListContentConfiguration.subtitleCell()
            c.text = row.title
            c.textProperties.font = Fonts.ui(.sansMedium, 16)
            c.textProperties.color = row.destructive ? Palette.danger : Palette.text
            c.secondaryText = row.subtitle
            c.secondaryTextProperties.font = Fonts.ui(.sans, 13)
            c.secondaryTextProperties.color = Palette.secondary
            c.image = UIImage(systemName: row.symbol)
            c.imageProperties.tintColor = row.destructive ? Palette.danger : row.tint
            cell.contentConfiguration = c
            var bg = UIBackgroundConfiguration.listGroupedCell()
            bg.backgroundColor = Palette.elevated
            cell.backgroundConfiguration = bg
            switch row.accessory {
            case .disclosure: cell.accessories = [.disclosureIndicator()]
            case .none: cell.accessories = []
            case let .check(on): cell.accessories = on ? [.checkmark(options: .init(tintColor: Palette.accent))] : []
            case let .dot(online):
                let dot = UIView(frame: CGRect(x: 0, y: 0, width: 8, height: 8))
                dot.backgroundColor = online ? Palette.success : Palette.tertiary
                dot.layer.cornerRadius = 4
                cell.accessories = [.customView(configuration: .init(customView: dot, placement: .trailing()))]
            case let .toggle(on):
                let toggle = UISwitch()
                toggle.isOn = on
                toggle.onTintColor = Palette.accent
                toggle.accessibilityIdentifier = "toggle-\(row.id)"
                toggle.addAction(UIAction { [weak self, weak toggle] _ in
                    self?.toggled(row.id, toggle?.isOn ?? false)
                }, for: .valueChanged)
                cell.accessories = [.customView(configuration: .init(customView: toggle, placement: .trailing()))]
            }
            cell.accessibilityIdentifier = "settings-\(row.id)"
        }
        let header = UICollectionView.SupplementaryRegistration<UICollectionViewListCell>(elementKind: UICollectionView.elementKindSectionHeader) { [weak self] view, _, path in
            var c = UIListContentConfiguration.groupedHeader()
            c.text = self?.dataSource.snapshot().sectionIdentifiers[path.section]
            view.contentConfiguration = c
        }
        dataSource = UICollectionViewDiffableDataSource(collectionView: collectionView) { cv, path, row in
            cv.dequeueConfiguredReusableCell(using: cell, for: path, item: row)
        }
        dataSource.supplementaryViewProvider = { cv, _, path in
            cv.dequeueConfiguredReusableSupplementary(using: header, for: path)
        }
        reload()
        wallpaperObserver = NotificationCenter.default.addObserver(forName: WallpaperStore.didChange, object: nil, queue: .main) { [weak self] _ in self?.reload() }
        // Devices go on/offline and the org name backfills after opening.
        appToken = app.observe { [weak self] in self?.reload() }
        // Back from iOS Settings (notifications may have been allowed there).
        foregroundObserver = NotificationCenter.default.addObserver(forName: UIApplication.willEnterForegroundNotification, object: nil, queue: .main) { [weak self] _ in
            self?.refreshNotificationStatus()
        }
    }

    private var foregroundObserver: NSObjectProtocol?

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        if dataSource != nil { reload() }
        refreshNotificationStatus()
        Task { @MainActor [weak self] in await self?.app.refreshCloudHost() }
    }

    // MARK: Notifications

    private var notificationStatus: UNAuthorizationStatus = .notDetermined

    private func refreshNotificationStatus() {
        Task { @MainActor in
            let status = await PushNotifications.shared.authorizationStatus()
            guard status != self.notificationStatus else { return }
            self.notificationStatus = status
            self.reload()
        }
    }

    /// On when this phone wants them and iOS allows them.
    private var notificationsOn: Bool {
        PushNotifications.shared.enabled && (notificationStatus == .authorized || notificationStatus == .provisional || notificationStatus == .ephemeral)
    }

    private func notificationRows() -> [Row] {
        var rows = [Row(id: "notify:enabled", title: "Notifications", subtitle: notificationStatus == .denied ? "Turned off in iOS Settings" : "When a session finishes, needs you or fails", symbol: "bell", accessory: .toggle(notificationsOn))]
        if notificationsOn {
            rows += PushNotifications.Kind.allCases.map { kind in
                let symbol: String = switch kind {
                case .done: "checkmark.circle"
                case .input: "questionmark.bubble"
                case .failed: "exclamationmark.triangle"
                }
                return Row(id: "notify:\(kind.rawValue)", title: kind.title, symbol: symbol, accessory: .toggle(PushNotifications.shared.isOn(kind)))
            }
        }
        return rows
    }

    private func toggled(_ id: String, _ on: Bool) {
        if id == "groupByProject" {
            app.groupByProject = on
            return
        }
        let push = PushNotifications.shared
        if id == "notify:enabled" {
            guard on else {
                push.enabled = false
                return reload()
            }
            if notificationStatus == .denied {
                // Only the Settings app can turn them back on.
                let alert = UIAlertController(title: "Notifications are off", message: "Allow notifications for OpenAgents in iOS Settings.", preferredStyle: .alert)
                alert.addAction(UIAlertAction(title: "Not Now", style: .cancel) { [weak self] _ in self?.reload() })
                alert.addAction(UIAlertAction(title: "Open Settings", style: .default) { [weak self] _ in
                    if let url = URL(string: UIApplication.openSettingsURLString) { UIApplication.shared.open(url) }
                    self?.reload()
                })
                return present(alert, animated: true)
            }
            push.enabled = true
            Task { @MainActor in
                await push.requestPermission()
                self.notificationStatus = await push.authorizationStatus()
                self.reload()
            }
            return
        }
        if let kind = PushNotifications.Kind(rawValue: String(id.dropFirst("notify:".count))) {
            push.set(kind, on)
        }
    }

    private var wallpaperObserver: NSObjectProtocol?
    private var appToken: AnyObject?

    private func reload() {
        var s = NSDiffableDataSourceSnapshot<String, Row>()
        s.appendSections(["Account"])
        s.appendItems([
            Row(id: "account", title: app.accountName, subtitle: app.accountDetail, symbol: "person.crop.circle", accessory: .none),
        ])
        // The account's identity and its backup (#11996). They show once the
        // Rust core has an identity to show (#11995).
        if let identity = app.identity {
            s.appendItems([Row(id: "your-id", title: "Your ID", subtitle: identity.npub, symbol: "person.text.rectangle", accessory: .none)])
            // TODO(#11996): show these once their screens exist. The core
            // has them: backupWords, backupCheck, confirmBackup,
            // restoreFromWords, setIcloudCopy, revealNsec (#11995).
            // s.appendItems([
            //     Row(id: "backup", title: "Back up your account", subtitle: identity.wordsSaved ? "Your 12 words are saved" : nil, symbol: "key", tint: identity.wordsSaved ? Palette.secondary : Palette.warning),
            //     Row(id: "restore", title: "Restore from 12 words", symbol: "arrow.counterclockwise"),
            // ])
        }
        if case .signedIn? = app.accountSnapshot?.state {
            s.appendItems([Row(id: "project-sharing", title: "Project sharing", symbol: "person.2")])
        }
        s.appendSections(["Wallet"])
        s.appendItems([Row(id: "wallet", title: "Wallet", subtitle: "Bitcoin on this phone", symbol: "bitcoinsign.circle")])
        s.appendSections(["Devices"])
        if app.client != nil, !app.isDemo {
            s.appendItems([Row(id: "scan-code", title: "Connect a computer", subtitle: "Scan the code OpenAgents shows on your computer", symbol: "qrcode.viewfinder")])
        }
        s.appendItems(app.hostOptions.map { Row(id: "device:\($0.id)", title: $0.displayName, subtitle: app.client?.deviceChannelEnabled(deviceId: $0.id) == true ? "Device connection" : ($0.online ? "Online" : "Offline"), symbol: "desktopcomputer", accessory: .dot($0.online)) })
        if app.offersCloudHost {
            let on = app.cloudHost?.running == true
            s.appendItems([Row(id: "cloudhost", title: on ? "Stop OpenAgents Cloud" : "Start OpenAgents Cloud", subtitle: app.cloudHostDetail, symbol: "cloud", accessory: .none, destructive: on)])
        }
        if app.voice.available {
            s.appendSections(["Voice"])
            s.appendItems([Row(id: "voice", title: "Voice", subtitle: app.voice.selectedStyle.map { "Codex voice · \($0.capitalized)" } ?? "Codex voice device and style", symbol: "waveform")])
        }
        // Push notifications return after the first release (#11853).
        s.appendSections(["Appearance"])
        let style = UserDefaults.standard.integer(forKey: "appearance")
        s.appendItems([
            Row(id: "appearance:0", title: "System", symbol: "circle.lefthalf.filled", accessory: .check(style == 0)),
            Row(id: "appearance:1", title: "Light", symbol: "sun.max", accessory: .check(style == 1)),
            Row(id: "appearance:2", title: "Dark", symbol: "moon", accessory: .check(style == 2)),
        ])
        s.appendSections(["Wallpaper"])
        var wall = [Row(id: "wallpaper:choose", title: WallpaperStore.isSet ? "Change Wallpaper…" : "Choose Wallpaper…", subtitle: WallpaperStore.isSet ? WallpaperStore.name : "Shown behind new chats and the sessions list", symbol: "photo")]
        if WallpaperStore.isSet {
            let effect = WallpaperStore.effect
            wall.append(Row(id: "wallpaper:effect", title: "Effect", subtitle: "\(WallpaperStore.label(effect)) — \(WallpaperStore.detail(effect))", symbol: "wand.and.stars"))
            wall.append(Row(id: "wallpaper:remove", title: "Remove Wallpaper", symbol: "trash", accessory: .none, destructive: true))
        }
        s.appendItems(wall)
        s.appendSections(["Sessions"])
        s.appendItems([
            Row(id: "groupByProject", title: "Group by Project", subtitle: "Sessions outside your sections, under their project", symbol: "square.stack.3d.up", accessory: .toggle(app.groupByProject)),
            Row(id: "archived", title: "Archived Sessions", symbol: "archivebox"),
        ])
        s.appendSections(["About"])
        s.appendItems([Row(id: "about", title: "OpenAgents", subtitle: Self.versionText, symbol: "info.circle")])
        if app.isSignedIn, !app.isDemo {
            s.appendSections([" "])
            s.appendItems([
                Row(id: "signout", title: "Sign Out", symbol: "rectangle.portrait.and.arrow.right", accessory: .none, destructive: true),
                Row(id: "delete-account", title: "Delete Account", symbol: "trash", accessory: .none, destructive: true),
            ])
        }
        dataSource.apply(s, animatingDifferences: false)
    }

    func collectionView(_ collectionView: UICollectionView, didSelectItemAt path: IndexPath) {
        collectionView.deselectItem(at: path, animated: true)
        guard let row = dataSource.itemIdentifier(for: path) else { return }
        switch row.id {
        case "project-sharing":
            do {
                let sharing = try app.openProjectSharing()
                present(UINavigationController(rootViewController: AgentFormController(app: app, sharing: sharing)), animated: true)
            } catch {
                let alert = UIAlertController(title: "Couldn't open project sharing", message: error.localizedDescription, preferredStyle: .alert)
                alert.addAction(UIAlertAction(title: "OK", style: .default))
                present(alert, animated: true)
            }
        case "about":
            navigationController?.pushViewController(AboutViewController(version: Self.versionText), animated: true)
        case "wallet": router?.showWallet()
        case "voice": navigationController?.pushViewController(VoiceViewController(app: app), animated: true)
        case "scan-code":
            CodeAction.scan(from: self, app: app) { [weak self] in self?.reload() }
        case "cloudhost":
            guard !app.cloudHostBusy else { return }
            let start = app.cloudHost?.running != true
            Task { @MainActor [weak self] in
                guard let self else { return }
                if let message = await self.app.setCloudHost(running: start) {
                    let alert = UIAlertController(title: start ? "Couldn't start OpenAgents Cloud" : "Couldn't stop OpenAgents Cloud", message: message, preferredStyle: .alert)
                    alert.addAction(UIAlertAction(title: "OK", style: .default))
                    self.present(alert, animated: true)
                }
                self.reload()
            }
        case let id where id.hasPrefix("device:"):
            guard let client = app.client else { return }
            let device = String(id.dropFirst("device:".count))
            let rename = UIAlertAction(title: "Rename…", style: .default) { [weak self] _ in self?.renameDevice(device, current: row.title) }
            if app.isDemo {
                let alert = UIAlertController(title: row.title, message: nil, preferredStyle: .alert)
                alert.addAction(rename)
                alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
                present(alert, animated: true)
            } else if client.deviceChannelEnabled(deviceId: device) {
                let alert = UIAlertController(title: "Device connection", message: "Use the usual connection for this computer?", preferredStyle: .alert)
                alert.addAction(rename)
                alert.addAction(UIAlertAction(title: "Use usual connection", style: .default) { [weak self] _ in
                    client.disableDeviceChannel(deviceId: device)
                    self?.reload()
                })
                alert.addAction(UIAlertAction(title: "Forget pairing", style: .destructive) { [weak self] _ in
                    Task { @MainActor [weak self] in
                        do { try await client.forgetDeviceChannel(deviceId: device) }
                        catch {
                            guard let self, self.app.client === client else { return }
                            let failure = UIAlertController(title: "Couldn't remove this pairing", message: "Try again when this computer is online.", preferredStyle: .alert)
                            failure.addAction(UIAlertAction(title: "OK", style: .default))
                            self.present(failure, animated: true)
                        }
                        self?.reload()
                    }
                })
                alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
                present(alert, animated: true)
            } else {
                let alert = UIAlertController(title: "Pair this computer", message: "Copy the connect code from this computer, then paste it here. This connection lasts until you sign out or close the app.", preferredStyle: .alert)
                alert.addAction(rename)
                alert.addAction(UIAlertAction(title: "Paste connect code", style: .default) { [weak self] _ in
                    let code = UIPasteboard.general.string ?? ""
                    Task { @MainActor [weak self] in
                        do { try await client.pairDeviceChannel(deviceId: device, code: code) }
                        catch {
                            guard let self, self.app.client === client else { return }
                            let failure = UIAlertController(title: "Couldn't pair this computer", message: "Check the code and try again.", preferredStyle: .alert)
                            failure.addAction(UIAlertAction(title: "OK", style: .default))
                            self.present(failure, animated: true)
                        }
                        self?.reload()
                    }
                })
                alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
                present(alert, animated: true)
            }
        case let id where id.hasPrefix("appearance:"):
            let style = Int(id.dropFirst(11)) ?? 0
            UserDefaults.standard.set(style, forKey: "appearance")
            view.window?.overrideUserInterfaceStyle = UIUserInterfaceStyle(rawValue: style) ?? .unspecified
            reload()
        case "wallpaper:choose":
            WallpaperPicker.present(from: self)
        case "wallpaper:effect":
            let sheet = UIAlertController(title: "Wallpaper Effect", message: nil, preferredStyle: .actionSheet)
            for e in WallpaperStore.allEffects {
                let action = UIAlertAction(title: WallpaperStore.label(e), style: .default) { _ in WallpaperStore.effect = e }
                action.setValue(e == WallpaperStore.effect, forKey: "checked")
                sheet.addAction(action)
            }
            sheet.addAction(UIAlertAction(title: "Cancel", style: .cancel))
            sheet.popoverPresentationController?.sourceView = collectionView.cellForItem(at: path)
            present(sheet, animated: true)
        case "wallpaper:remove":
            WallpaperStore.remove()
        case "archived":
            navigationController?.pushViewController(FolderViewController(app: app, folder: FolderRowVM(id: "archived", name: "Archived", count: 0, symbol: "archivebox")), animated: true)
        case "your-id":
            guard let npub = app.identity?.npub else { return }
            // The public ID only; it is not a secret, so it may stay on the pasteboard.
            UIPasteboard.general.string = npub
            Toast.show("Copied your ID", in: view.window)
        case "signout":
            let alert = UIAlertController(title: "Sign out?", message: "Local drafts stay on this device.", preferredStyle: .actionSheet)
            alert.addAction(UIAlertAction(title: "Sign Out", style: .destructive) { [weak self] _ in self?.app.signOut() })
            alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
            alert.popoverPresentationController?.sourceView = collectionView.cellForItem(at: path)
            present(alert, animated: true)
        case "delete-account":
            confirmDeleteAccount(from: collectionView.cellForItem(at: path))
        default:
            break
        }
    }

    /// Delete Account (#12002): the warning says what goes and that the
    /// 12 words still hold the wallet; the person confirms before anything
    /// is sent, and again before giving up paid credit.
    private func confirmDeleteAccount(from source: UIView?) {
        let alert = UIAlertController(title: "Delete your account?", message: accountDeletionWarning(), preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        alert.addAction(UIAlertAction(title: "Delete Account", style: .destructive) { [weak self] _ in
            self?.deleteAccount(forfeitBalance: false)
        })
        present(alert, animated: true)
    }

    private func deleteAccount(forfeitBalance: Bool) {
        Task { @MainActor [weak self] in
            guard let self else { return }
            let result = await self.app.deleteAccount(forfeitBalance: forfeitBalance)
            switch result {
            case .deleted:
                break
            case .paidCredit(let message):
                let alert = UIAlertController(title: "Give up your credit?", message: message, preferredStyle: .alert)
                alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
                alert.addAction(UIAlertAction(title: "Give Up and Delete", style: .destructive) { [weak self] _ in
                    self?.deleteAccount(forfeitBalance: true)
                })
                self.present(alert, animated: true)
            case .sharedTeam(let message), .failed(let message):
                let alert = UIAlertController(title: "Couldn't delete your account", message: message, preferredStyle: .alert)
                alert.addAction(UIAlertAction(title: "OK", style: .default))
                self.present(alert, animated: true)
            }
        }
    }

    /// A nickname is a synced device field: every signed-in surface shows it.
    private func renameDevice(_ device: String, current: String) {
        let alert = UIAlertController(title: "Rename device", message: "Everywhere you're signed in shows this name.", preferredStyle: .alert)
        alert.addTextField { field in
            field.text = current
            field.placeholder = "Device name"
            field.autocapitalizationType = .words
            field.clearButtonMode = .whileEditing
            field.accessibilityIdentifier = "device-name-field"
        }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
        alert.addAction(UIAlertAction(title: "Save", style: .default) { [weak self, weak alert] _ in
            guard let self, let client = self.app.client else { return }
            let name = alert?.textFields?.first?.text?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            guard !name.isEmpty, name != current else { return }
            do {
                try client.renameDevice(deviceId: device, name: name)
            } catch {
                let failure = UIAlertController(title: "Couldn't rename this device", message: "Try again in a moment.", preferredStyle: .alert)
                failure.addAction(UIAlertAction(title: "OK", style: .default))
                self.present(failure, animated: true)
            }
            self.reload()
        })
        present(alert, animated: true)
    }
}
