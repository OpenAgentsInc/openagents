// Firm reason: 6 (system UI: Liquid Glass and popovers are the platform's own; components/gpu-components.md).
// The system controls over the GPU chat page (GitHub #13645): the menu button at the top left,
// a round Liquid Glass button that rides with the page while the sidebar pushes it; Back in the same
// place while the login page shows (the page keeps login.back's row and paints nothing); and the chips'
// picker, a SwiftUI popover anchored to the chip that opened it. The page (chat.coax, chat_native)
// paints neither and keeps their rows: what each row says, whether it is chosen and whether it can be
// chosen are the committed rows of the environment area's picker (apps/shared/app/environment/
// view.coax es_picker). A press goes back to the page by the row's ID (oc_activate), as a tap does.
// Nothing here decides what can be chosen.
import SwiftUI
import UIKit

/// A round button at the top left: native Liquid Glass on iOS 26 and later, a material blur before.
/// The menu button names the sidebar's state; Back (`back: true`) says Back.
// UIButton supplies real touch, keyboard and accessibility primary activation.
// A bare UIControl does not turn a finger tap into primaryActionTriggered.
@MainActor final class GlassMenuButton: UIButton {
    private let glyph = UIImageView()
    private var ground: UIVisualEffectView
    private let back: Bool
    var open = false { didSet { if open != oldValue { label() } } }

    init(back: Bool = false) {
        self.back = back
        #if compiler(>=6.2)
        if #available(iOS 26.0, *) {
            let effect = UIGlassEffect(style: .regular); effect.isInteractive = true
            ground = UIVisualEffectView(effect: effect)
        } else { ground = UIVisualEffectView(effect: UIBlurEffect(style: .systemUltraThinMaterialDark)) }
        #else
        ground = UIVisualEffectView(effect: UIBlurEffect(style: .systemUltraThinMaterialDark))
        #endif
        super.init(frame: .zero)
        ground.isUserInteractionEnabled = false; ground.clipsToBounds = true
        addSubview(ground)
        glyph.image = UIImage(systemName: back ? "chevron.left" : "sidebar.left", withConfiguration: UIImage.SymbolConfiguration(pointSize: 17, weight: .medium))
        glyph.tintColor = .white; glyph.contentMode = .center; glyph.isUserInteractionEnabled = false
        ground.contentView.addSubview(glyph)
        isAccessibilityElement = true; accessibilityIdentifier = back ? "login.back" : "nav.menu"; accessibilityTraits = .button
        label()
    }
    required init?(coder: NSCoder) { fatalError() }
    private func label() { accessibilityLabel = back ? "Back" : open ? "Close sidebar" : "Open sidebar" }
    override func layoutSubviews() {
        super.layoutSubviews()
        ground.frame = bounds; ground.layer.cornerRadius = min(bounds.width, bounds.height) / 2
        ground.layer.cornerCurve = .circular; glyph.frame = ground.contentView.bounds
    }
    override var isHighlighted: Bool { didSet { glyph.alpha = isHighlighted ? 0.6 : 1 } }
}

/// What the open picker shows, read from the page's committed rows.
@MainActor final class GlassPickerModel: ObservableObject {
    struct Row: Identifiable, Equatable { let id: String; let title: String; let detail: String; let checked: Bool; let enabled: Bool }
    struct Content: Equatable { var title = ""; var rows: [Row] = []; var notes: [String] = []; var busy = false }
    @Published var open = false
    @Published var content = Content()
    /// A row's ID, back to the page (the same press a tap on the row makes).
    var press: (String) -> Void = { _ in }
}

private struct GlassPickerAnchor: View {
    @ObservedObject var model: GlassPickerModel
    var body: some View {
        Color.clear.popover(isPresented: Binding(get: { model.open }, set: { shown in
            // the popover went away by itself (a tap beside it): the page's picker closes too
            if !shown, model.open { model.open = false; model.press("env.close") }
        })) {
            GlassPickerList(model: model).presentationCompactAdaptation(.popover)
        }
    }
}

private struct GlassPickerList: View {
    @ObservedObject var model: GlassPickerModel
    private var c: GlassPickerModel.Content { model.content }
    var body: some View {
        let list = VStack(alignment: .leading, spacing: 0) {
            ForEach(c.rows) { row in
                Button { model.press(row.id) } label: {
                    HStack(alignment: .center, spacing: 12) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(row.title).font(.body).foregroundStyle(row.enabled ? .primary : .tertiary)
                            if !row.detail.isEmpty { Text(row.detail).font(.footnote).foregroundStyle(row.enabled ? .secondary : .tertiary) }
                        }
                        Spacer(minLength: 8)
                        Image(systemName: "checkmark").font(.body.weight(.semibold)).opacity(row.checked ? 1 : 0)
                    }
                    .padding(.horizontal, 16).padding(.vertical, 10).contentShape(Rectangle())
                }
                .buttonStyle(.plain).disabled(!row.enabled)
                .accessibilityIdentifier(row.id)
                .accessibilityAddTraits(row.checked ? .isSelected : [])
            }
            ForEach(c.notes, id: \.self) { note in
                Text(note).font(.footnote).foregroundStyle(.secondary).padding(.horizontal, 16).padding(.vertical, 10)
                    .accessibilityIdentifier("env.note")
            }
        }
        return VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text(c.title).font(.headline)
                Spacer()
                Button { model.press("env.refresh") } label: {
                    if c.busy { ProgressView() } else { Image(systemName: "arrow.clockwise") }
                }
                .disabled(c.busy).accessibilityLabel("Refresh").accessibilityIdentifier("env.refresh")
            }
            .padding(.horizontal, 16).padding(.top, 14).padding(.bottom, 6)
            if c.rows.count + c.notes.count > 6 { ScrollView { list }.frame(height: 360) } else { list.padding(.bottom, 8) }
        }
        .frame(width: 300).tint(.primary)
    }
}

/// Both controls, kept in step with the page once a frame (sync).
@MainActor final class GlassChrome {
    let menu = GlassMenuButton()
    let back = GlassMenuButton(back: true)
    private let model = GlassPickerModel()
    private let anchor: UIHostingController<GlassPickerAnchor>
    private weak var host: UIViewController?
    private var sheet: Int32 = 0
    private var stamp: Int32 = -1
    /// A press by row ID into the page; true when it did something.
    var activate: (String) -> Bool = { _ in false }

    init(on host: UIViewController) {
        self.host = host
        anchor = UIHostingController(rootView: GlassPickerAnchor(model: model))
        anchor.view.backgroundColor = .clear; anchor.view.isUserInteractionEnabled = false
        anchor.view.isAccessibilityElement = false
        anchor.safeAreaRegions = []
        host.addChild(anchor); host.view.addSubview(anchor.view); anchor.didMove(toParent: host)
        host.view.addSubview(menu)
        back.isHidden = true; host.view.addSubview(back)
        back.addAction(UIAction { [weak self] _ in _ = self?.activate("login.back") }, for: .primaryActionTriggered)
        menu.addAction(UIAction { [weak self] _ in
            guard let self else { return }
            // closed, the page takes nav.menu; open, the press lands on the sidebar's scrim
            if !self.activate("nav.menu") { _ = self.activate("drawer.scrim") }
        }, for: .primaryActionTriggered)
        model.press = { [weak self] id in _ = self?.activate(id) }
    }

    /// After each frame: place the menu button, and open, fill or close the popover.
    func sync(_ owner: UnsafeMutableRawPointer, rowsStamp: Int32) {
        var m = [Float](repeating: 0, count: 6)
        if oc_menu(owner, &m) != 0 {
            let frame = CGRect(x: CGFloat(m[0] + m[3]), y: CGFloat(m[1]), width: CGFloat(m[2]), height: CGFloat(m[2]))
            if menu.frame != frame { menu.frame = frame }
            menu.open = m[4] != 0; menu.isHidden = m[5] != 0
            // the login page shows: Back stands where the menu button was
            let at = CGRect(x: CGFloat(m[0]), y: CGFloat(m[1]), width: CGFloat(m[2]), height: CGFloat(m[2]))
            if back.frame != at { back.frame = at }
            back.isHidden = m[5] == 0
        } else { menu.isHidden = true; back.isHidden = true }
        let now = oc_sheet(owner)
        if now != 0 {
            var b = [Float](repeating: 0, count: 4)
            if oc_find(owner, now == 1 ? "composer.environment" : "composer.repo", &b) != 0 {
                let frame = CGRect(x: CGFloat(b[0]), y: CGFloat(b[1]), width: CGFloat(b[2]), height: CGFloat(b[3]))
                if anchor.view.frame != frame { anchor.view.frame = frame }
            }
            if rowsStamp != stamp || now != sheet { stamp = rowsStamp; let c = read(owner); if c != model.content { model.content = c } }
            if !model.open { model.open = true }
        } else if model.open, sheet != 0 { model.open = false }
        sheet = now
    }

    #if DEBUG
    /// Debug builds: `-OAGlassSample 1` opens the popover over the composer with example rows, so its
    /// look can be captured logged out. The rows go nowhere: a press only closes it.
    func sample(_ owner: UnsafeMutableRawPointer) {
        guard UserDefaults.standard.bool(forKey: "OAGlassSample") else { return }
        var b = [Float](repeating: 0, count: 4)
        guard oc_find(owner, "chat.composer", &b) != 0 else { return }
        anchor.view.frame = CGRect(x: CGFloat(b[0]) + 8, y: CGFloat(b[1]), width: 120, height: 32)
        model.content = .init(title: "Where it runs", rows: [
            .init(id: "env.default", title: "Default", detail: "We choose where each task runs", checked: true, enabled: true),
            .init(id: "env.c.sample1", title: "coderos", detail: "Ready", checked: false, enabled: true),
            .init(id: "env.c.sample2", title: "mac", detail: "Not answering", checked: false, enabled: false)])
        model.press = { [weak self] _ in self?.model.open = false }
        model.open = true
    }
    #endif

    /// The picker's committed rows: its panel's title, its choices, its notes and whether it is checking.
    private func read(_ owner: UnsafeMutableRawPointer) -> GlassPickerModel.Content {
        var c = GlassPickerModel.Content()
        for i in 0..<oc_rows(owner) {
            var row = [Float](repeating: 0, count: 8), id = [CChar](repeating: 0, count: 64), label = [CChar](repeating: 0, count: 256), value = [CChar](repeating: 0, count: 256)
            guard oc_row(owner, i, &row, &id, Int32(id.count), &label, Int32(label.count)) != 0 else { continue }
            let name = String(cString: id), words = String(cString: label), role = Int(row[4]), state = Int(row[5])
            guard name.hasPrefix("env.") else { continue }
            switch role {
            case 4: c.title = words                                   // the panel
            case 1:                                                   // a choice
                _ = oc_row_value(owner, i, &value, Int32(value.count))
                c.rows.append(.init(id: name, title: words, detail: String(cString: value), checked: state & 32 != 0, enabled: state & 1 == 0))
            case 8: c.notes.append(words)                             // a note
            default: if name == "env.refresh" { c.busy = state & 2 != 0 }
            }
        }
        return c
    }

    /// The page's own rows that these controls stand for in VoiceOver.
    static func covers(_ id: String) -> Bool { id == "nav.menu" || id == "login.back" || id.hasPrefix("env.") }
}
