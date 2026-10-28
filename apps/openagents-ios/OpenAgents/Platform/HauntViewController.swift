import UIKit
import QuartzCore
import UniformTypeIdentifiers

private final class HauntSurface: UIView {
    override class var layerClass: AnyClass { CAMetalLayer.self }
    var metalLayer: CAMetalLayer { layer as! CAMetalLayer }
}

/// Full-screen child world, never a modal world/web page. Coax owns gameplay;
/// native controls bind the engine's state/actions and platform effects.
final class HauntViewController: UIViewController, UIDocumentPickerDelegate {
    private var surface: HauntSurface { view as! HauntSurface }
    private var renderer: HomeRenderer?
    private var assets: HauntAssets?
    private var clock: CADisplayLink?
    private var handle: Int64 = 0
    private var lastTick: Double?
    private var epoch: Double?
    private var player = verse_player_place(7, 0, 2, 0)
    private var sticks = verse_sticks_reset()
    private var fingers: [ObjectIdentifier: Int64] = [:]
    private var nextFinger: Int64 = 0
    private var began: [ObjectIdentifier: CGPoint] = [:]
    private var size = CGSize.zero
    private let titleLabel = UILabel(), notice = UILabel(), goal = UILabel()
    private let interact = UIButton(type: .custom)
    private let hotbar = EngineHotbar()
    private var worldElement: UIAccessibilityElement?
    var closed: (() -> Void)?
    private var lastEye = coax_t_HauntPoint(), lastAt = coax_t_HauntPoint()
    override var prefersStatusBarHidden: Bool { true }
    override var prefersHomeIndicatorAutoHidden: Bool { true }
    override func loadView() { view = HauntSurface(); view.isMultipleTouchEnabled = true; view.backgroundColor = .black }
    override func viewDidLoad() {
        super.viewDidLoad()
        do {
            renderer = try HomeRenderer(layer: surface.metalLayer)
            guard let renderer else { return }
            assets = try HauntAssets(device: renderer.device, queue: renderer.commandQueue)
            var byte: UInt8 = 0; handle = mr_create(&byte, 1, 1)
            player.phi = .pi / 2
            setUpHUD()
            let pinch = UIPinchGestureRecognizer(target: self, action: #selector(zoom(_:))); pinch.cancelsTouchesInView = true; view.addGestureRecognizer(pinch)
            NotificationCenter.default.addObserver(self, selector: #selector(suspend), name: UIApplication.willResignActiveNotification, object: nil)
            NotificationCenter.default.addObserver(self, selector: #selector(resume), name: UIApplication.didBecomeActiveNotification, object: nil)
            resume()
        } catch { showFailure(error.localizedDescription) }
    }
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        let k = view.window?.screen.scale ?? UIScreen.main.scale
        surface.metalLayer.contentsScale = k
        let next = CGSize(width: view.bounds.width*k, height: view.bounds.height*k)
        if next != size { size = next; surface.metalLayer.drawableSize = next; renderer?.resize(next) }
        hotbar.frame = view.bounds; hotbar.bottomInset = view.safeAreaInsets.bottom
        titleLabel.frame = CGRect(x: 18, y: view.safeAreaInsets.top + 10, width: view.bounds.width - 36, height: 25)
        goal.frame = CGRect(x: 18, y: titleLabel.frame.maxY + 5, width: 210, height: 22)
        notice.frame = CGRect(x: 24, y: view.safeAreaInsets.top + 75, width: view.bounds.width - 48, height: 80)
        interact.frame = CGRect(x: max(20,view.bounds.midX - 120), y: view.bounds.height - view.safeAreaInsets.bottom - 118, width: 240, height: 44)
        worldElement?.accessibilityFrameInContainerSpace = view.bounds.insetBy(dx: 0, dy: 140)
    }
    private func setUpHUD() {
        for label in [titleLabel, notice, goal] { label.textColor = .white; label.font = UIFont(name: "Michroma", size: label === titleLabel ? 12 : 10); label.numberOfLines = label === notice ? 3 : 1; label.isUserInteractionEnabled = false; view.addSubview(label) }
        notice.textAlignment = .center
        titleLabel.text = "HAUNTED HOUSE"; titleLabel.accessibilityIdentifier = "haunt.title"
        goal.accessibilityIdentifier = "haunt.quest"
        interact.setTitleColor(.white, for: .normal); interact.titleLabel?.font = UIFont(name: "Michroma", size: 11)
        interact.backgroundColor = UIColor(white: 0.02, alpha: 0.7); interact.layer.cornerRadius = 10; interact.layer.borderWidth = 1; interact.layer.borderColor = UIColor(white: 0.7, alpha: 0.4).cgColor
        interact.accessibilityIdentifier = "haunt.interact"
        interact.addAction(UIAction { [weak self] _ in self?.activateNearby() }, for: .touchUpInside)
        view.addSubview(interact)
        hotbar.setActions([
            .init(id: "haunt.edit", label: "Edit", symbol: "pencil.tip.crop.circle", invoke: { [weak self] in self?.openEdit() }),
            .init(id: "haunt.talk", label: "Talk", symbol: "bubble.left", invoke: { [weak self] in self?.openTalk() }),
            .init(id: "haunt.view", label: "View", symbol: "eye", invoke: { [weak self] in self?.cycleView() }),
            .init(id: "haunt.guide", label: "Quest", symbol: "sparkle", invoke: { [weak self] in self?.openQuest() }),
            .init(id: "haunt.map", label: "Map", symbol: "map", invoke: { [weak self] in self?.openMap() }),
            .init(id: "haunt.exit", label: "Grid", symbol: "circle.hexagongrid", invoke: { [weak self] in self?.leave() })
        ])
        view.addSubview(hotbar)
        let world = UIAccessibilityElement(accessibilityContainer: view); world.accessibilityIdentifier = "haunt.world"; world.accessibilityLabel = "Haunted House"; world.accessibilityTraits = .allowsDirectInteraction
        worldElement = world
        view.accessibilityElements = [titleLabel, goal, world, interact, hotbar as Any]
    }
    @objc private func resume() {
        guard renderer != nil, clock == nil, assets != nil else { return }
        let display = CADisplayLink(target: self, selector: #selector(tick(_:)))
        display.preferredFrameRateRange = CAFrameRateRange(minimum: 30, maximum: 60, preferred: 60)
        display.add(to: .main, forMode: .common); clock = display; assets?.startAudio(); lastTick = nil
    }
    @objc private func suspend() { clock?.invalidate(); clock = nil; sticks = verse_sticks_reset(); fingers = [:]; began = [:]; lastTick = nil; assets?.stopAudio(); assets?.save() }
    @objc private func zoom(_ gesture: UIPinchGestureRecognizer) { player = verse_player_zoom(player, Float((1 - gesture.scale)*4)); gesture.scale = 1; sticks = verse_sticks_reset() }
    @objc private func tick(_ link: CADisplayLink) {
        guard let assets, size.width > 0, size.height > 0 else { return }
        let now = link.timestamp; if epoch == nil { epoch = now }; let time = now - (epoch ?? now)
        let dt = Float(min(max(now - (lastTick ?? now), 0), 0.1)); lastTick = now
        let was = player.pos
        player = verse_player_input(player, 0, 0, sticks.pad_x, sticks.pad_y, sticks.run, sticks.jump, sticks.dyaw, sticks.dpitch)
        sticks = verse_sticks_used(sticks)
        player = assets.wallBytes.withUnsafeMutableBufferPointer { wall in
            verse_player_advance_among(player, dt, coax_v_f32(data: nil, count: 0), coax_v_f32(data: wall.baseAddress, count: Int64(assets.wallCount*6)))
        }
        let constrained = verse_haunt_constrain(assets.state, hauntPoint(was), hauntPoint(player.pos))
        player = verse_player_constrain(player, constrained.x, constrained.z)
        let eye = verse_player_view_eye(player, assets.state.camera), at = verse_player_view_at(player, assets.state.camera)
        let feet = verse_player_feet(player)
        lastEye = hauntPoint(eye); lastAt = hauntPoint(at)
        let distance = hypot(feet.x-was.x, feet.z-was.z)
        assets.state.facing = verse_player_face(player)
        assets.state = verse_haunt_advance(assets.state, time, hauntPoint(feet), lastEye, player.theta, distance, player.on_ground)
        assets.playSounds()
        var matrix = [Float](repeating: 0, count: 16)
        _ = matrix.withUnsafeMutableBufferPointer { verse_grid_view(eye.x, eye.y, eye.z, at.x, at.y, at.z, Float(size.width), Float(size.height), coax_v_f32(data: $0.baseAddress, count: 16)) }
        mr_clear_frame(handle)
        verse_grid_controls(handle, sticks, Float(size.width), Float(size.height), Float(surface.metalLayer.contentsScale))
        renderer?.draw(handle: handle, time: time, haunt: assets, houseView: matrix)
        let area = verse_haunt_area(assets.state, hauntPoint(feet))
        let name = area >= 0 ? hauntText(assets.state.areas.data![Int(area)].name) : "Haunted House"
        worldElement?.accessibilityValue = String(format: "%@ %.1f %.1f · %lld objects", name, feet.x, feet.z, assets.state.entity_count)
        titleLabel.text = name.uppercased()
        goal.text = hauntText(verse_haunt_goal_name(assets.state))
        notice.text = time < assets.state.toast_until ? hauntText(assets.state.toast) : nil
        let person = verse_haunt_near(assets.state), door = verse_haunt_door(assets.state)
        let target = assets.state.editing && assets.state.selected >= 0 ? hauntText(assets.state.entities.data![Int(assets.state.selected)].name) : person >= 0 ? hauntText(assets.state.people.data![Int(person)].name) : door >= 0 ? "Door" : ""
        interact.setTitle(target, for: .normal); interact.isHidden = target.isEmpty
        if assets.state.dirty && fingers.isEmpty && Int(time*10)%20 == 0 { assets.save() }
    }
    private func route(_ touches: Set<UITouch>, _ phase: Int64) {
        let scale = surface.metalLayer.contentsScale
        for touch in touches {
            let id = ObjectIdentifier(touch), p = touch.location(in: view)
            if phase == 0 { fingers[id] = nextFinger; nextFinger += 1; began[id] = p }
            guard let slot = fingers[id] else { continue }
            sticks = verse_sticks_world_touch(sticks, slot, phase, Float(p.x*scale), Float(p.y*scale), Float(size.width), Float(size.height), Float(scale), touch.timestamp)
            if phase == 2, let start = began[id], hypot(start.x-p.x,start.y-p.y) < 8, assets?.state.editing == true {
                pick(p); sticks.jump = false
            }
            if phase >= 2 { fingers[id] = nil; began[id] = nil }
        }
    }
    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent?) { route(touches, 0) }
    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent?) { route(touches, 1) }
    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent?) { route(touches, 2) }
    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent?) { route(touches, 3) }
    private func pick(_ point: CGPoint) {
        guard let a = assets else { return }
        let direction = verse_haunt_ray(lastEye,lastAt,Float(point.x),Float(point.y),Float(view.bounds.width),Float(view.bounds.height))
        a.state = verse_haunt_select(a.state,verse_haunt_pick(a.state,lastEye,direction))
        if a.state.selected >= 0 { editSelected() }
    }
    private func activateNearby() {
        guard let a = assets else { return }
        if a.state.editing && a.state.selected >= 0 { editSelected(); return }
        let person = verse_haunt_near(a.state)
        if person >= 0 { a.state = verse_haunt_talk(a.state, person, 0); openTalk(); return }
        let door = verse_haunt_door(a.state)
        if door >= 0 { a.remember(); a.state = verse_haunt_edit(a.state,door,9,0); a.rebuildWalls(); a.save() }
    }
    private func cycleView() { guard let a = assets else { return }; a.state = verse_haunt_view(a.state); player.phi = a.state.camera == 0 ? .pi/2 : 1.107; UIImpactFeedbackGenerator(style: .light).impactOccurred() }
    private func leave() { suspend(); closed?() }
    private func showFailure(_ message: String) {
        let alert = UIAlertController(title: "Haunted House", message: message, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "Grid", style: .default) { [weak self] _ in self?.leave() }); present(alert, animated: true)
    }
    private func menu(_ title: String, _ rows: [SceneRow]) {
        sticks = verse_sticks_reset(); fingers = [:]; began = [:]
        let panel = SceneList(title: title, rows: rows)
        let nav = UINavigationController(rootViewController: panel); nav.overrideUserInterfaceStyle = .dark
        if let sheet = nav.sheetPresentationController { sheet.detents = [.medium(), .large()]; sheet.prefersGrabberVisible = true }
        present(nav, animated: true)
    }
    private func openEdit() {
        guard let a = assets else { return }; a.state = verse_haunt_edit_mode(a.state,true)
        menu("Edit", [
            SceneRow("Objects", "", { [weak self] in self?.objectList() }),
            SceneRow("Add", "", { [weak self] in self?.catalogue() }),
            SceneRow("Undo", "", { a.undo() }), SceneRow("Redo", "", { a.redo() }),
            SceneRow("Characters", "", { [weak self] in self?.peopleList() }),
            SceneRow("Zones", "", { [weak self] in self?.zonesList() }),
            SceneRow("World", "", { [weak self] in self?.worldMenu() }),
            SceneRow("Done", "", { a.state = verse_haunt_edit_mode(a.state,false); a.save() })
        ])
    }
    private func objectList() {
        guard let a = assets else { return }
        let rows = (0..<Int(a.state.entity_count)).filter { !a.state.entities.data![$0].fixed || a.state.entities.data![$0].motion == 5 }.map { i in
            let e = a.state.entities.data![i]
            return SceneRow(hauntText(e.name), hauntText(e.id), { [weak self] in a.state = verse_haunt_select(a.state,Int64(i)); self?.editSelected() })
        }
        menu("Objects", rows)
    }
    private func editSelected() {
        guard let a = assets, a.state.selected >= 0 else { return }
        let i = a.state.selected, e = a.state.entities.data![Int(i)]
        func edit(_ op: Int64, _ value: Float) { a.remember(); a.state = verse_haunt_edit(a.state,i,op,value); a.rebuildWalls(); a.save() }
        menu(hauntText(e.name), [SceneRow("←", "", { edit(0,-0.5) }), SceneRow("→", "", { edit(0,0.5) }), SceneRow("Forward", "", { edit(2,0.5) }), SceneRow("Back", "", { edit(2,-0.5) }), SceneRow("Up", "", { edit(1,0.25) }), SceneRow("Down", "", { edit(1,-0.25) }), SceneRow("Turn", "", { edit(3,.pi/4) }), SceneRow("Larger", "", { edit(4,1.2) }), SceneRow("Smaller", "", { edit(4,1/1.2) }), SceneRow("Warm", "", { edit(5,0) }), SceneRow("Cool", "", { edit(8,0) }), SceneRow("Hide / show", "", { edit(6,0) }), SceneRow("Remove", "", { edit(7,0) })])
    }
    private func catalogue() {
        guard let a = assets else { return }
        menu("Add", (0..<Int(a.state.mesh_count)).map { i in
            let m = a.state.meshes.data![i]
            return SceneRow(hauntText(m.name), "", {
                a.remember(); let point = verse_haunt_place_point(a.state)
                a.state = verse_haunt_add(a.state,Int64(i),a.retain("placed-" + UUID().uuidString),m.name,point.x,point.z)
                a.rebuildWalls(); a.save()
            })
        })
    }
    private func peopleList() {
        guard let a = assets else { return }
        menu("Characters", [SceneRow("New", "", { [weak self] in self?.newCharacter() })] + (0..<Int(a.state.people_count)).map { i in SceneRow(hauntText(a.state.people.data![i].name), "", { [weak self] in self?.characterMenu(Int64(i)) }) })
    }
    private func newCharacter() {
        guard let a = assets else { return }
        let alert = UIAlertController(title: "Character", message: nil, preferredStyle: .alert)
        alert.addTextField { $0.placeholder = "Name" }
        for (title, figure) in [("Tunic","Custom_Fig_Guest0"),("Dress","Custom_Fig_Guest4")] {
            alert.addAction(UIAlertAction(title: title, style: .default) { _ in
                a.remember()
                a.state = verse_haunt_add_person(a.state,a.retain("person-"+UUID().uuidString),a.retain(alert.textFields?.first?.text ?? ""),a.retain(figure),a.retain(Data(repeating:0,count:8192)),a.retain(Data(repeating:0,count:8192)))
                a.rebuildWalls(); a.save()
            })
        }
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel)); present(alert, animated: true)
    }
    private func characterMenu(_ index: Int64) {
        guard let a = assets else { return }
        let p = a.state.people.data![Int(index)]
        let actions: [(String,Int64)] = [("Follow",0),("Wander",1),("Guard",2),("Stay",3),("Flee",4),("Move here",5),("Look",7),("Remove",6)]
        menu(hauntText(p.name), [SceneRow("Teach", "", { [weak self] in self?.teach(index) })] + actions.map { title, op in SceneRow(title,"",{ a.remember();a.state=verse_haunt_person_action(a.state,index,op);a.rebuildWalls();a.save() }) })
    }
    private func teach(_ index: Int64) {
        guard let a = assets else { return }
        let alert = UIAlertController(title: "Teach", message: nil, preferredStyle: .alert)
        alert.addTextField { $0.placeholder = "What should they know?" }
        alert.addAction(UIAlertAction(title:"Save",style:.default) { _ in a.remember();a.state=verse_haunt_teach(a.state,index,a.retain(alert.textFields?.first?.text ?? ""));a.save() })
        alert.addAction(UIAlertAction(title:"Cancel",style:.cancel));present(alert,animated:true)
    }
    private func openTalk() {
        sticks = verse_sticks_reset(); fingers = [:]; began = [:]
        guard let a = assets else { return }
        if a.state.talking < 0 {
            let near = verse_haunt_near(a.state)
            guard near >= 0 else { notice.text = "Walk up to a character."; return }
            a.state = verse_haunt_talk(a.state,near,0)
        }
        let person = a.state.people.data![Int(a.state.talking)]
        let alert = UIAlertController(title: hauntText(person.name), message: hauntText(a.state.reply), preferredStyle: .alert)
        alert.addTextField { $0.placeholder = "Ask…"; $0.accessibilityIdentifier = "haunt.question" }
        alert.addAction(UIAlertAction(title: "Ask", style: .default) { [weak self] _ in
            a.state = verse_haunt_ask(a.state,a.retain(alert.textFields?.first?.text ?? "")); self?.openTalk()
        })
        alert.addAction(UIAlertAction(title: "Hint", style: .default) { [weak self] _ in a.state = verse_haunt_talk(a.state,a.state.talking,1); self?.openTalk() })
        alert.addAction(UIAlertAction(title: "Teach", style: .default) { _ in a.remember(); a.state = verse_haunt_teach(a.state,a.state.talking,a.retain(alert.textFields?.first?.text ?? "")); a.save() })
        alert.addAction(UIAlertAction(title: "Bye", style: .cancel) { _ in a.state = verse_haunt_talk(a.state,-1,0) })
        present(alert, animated: true)
    }
    private func zonesList() {
        guard let a = assets else { return }
        menu("Zones", (0..<Int(a.state.zone_count)).map { i in SceneRow(hauntText(a.state.zones.data![i].name), "", { [weak self] in
            self?.menu("Zone", [SceneRow("Once / repeat", "", { a.state = verse_haunt_zone_edit(a.state,Int64(i),0,0) }), SceneRow("Larger", "", { a.state = verse_haunt_zone_edit(a.state,Int64(i),1,0.5) }), SceneRow("Smaller", "", { a.state = verse_haunt_zone_edit(a.state,Int64(i),1,-0.5) }), SceneRow("Rearm", "", { a.state = verse_haunt_zone_edit(a.state,Int64(i),2,0) })])
        }) })
    }
    private func worldMenu() {
        guard let a = assets else { return }
        menu("World", [SceneRow("Worlds", "", { [weak self] in self?.worlds() }), SceneRow("Create", "", { [weak self] in self?.newWorld() }), SceneRow("Save", "", { a.state = verse_haunt_quest(a.state,a.retain("publish")); a.save() }), SceneRow("Share", "", { [weak self] in self?.share() }), SceneRow("Import", "", { [weak self] in self?.importFile() }), SceneRow("Reset", "", { [weak self] in self?.confirmReset() })])
    }
    private func share() {
        guard let a = assets else { return }
        do {
            let url = try a.exportWorld(), sheet = UIActivityViewController(activityItems: [url], applicationActivities: nil)
            sheet.popoverPresentationController?.sourceView = hotbar
            sheet.completionWithItemsHandler = { _, completed, _, _ in if completed { a.state = verse_haunt_quest(a.state,a.retain(a.state.quest == 6 ? "invite" : "share")); a.save() } }
            present(sheet, animated: true)
        } catch { showFailure(error.localizedDescription) }
    }
    private func importFile() { let picker = UIDocumentPickerViewController(forOpeningContentTypes: [.data], asCopy: true); picker.delegate = self; present(picker, animated: true) }
    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) { guard let url = urls.first else { return }; do { try assets?.importWorld(url) } catch { showFailure(error.localizedDescription) } }
    private func confirmReset() {
        let alert = UIAlertController(title: "Reset?", message: nil, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: "Reset", style: .destructive) { [weak self] _ in do { try self?.assets?.reset() } catch { self?.showFailure(error.localizedDescription) } })
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel)); present(alert, animated: true)
    }
    private func openQuest() {
        guard let a = assets else { return }
        menu("Make it yours", [SceneRow(hauntText(verse_haunt_goal_name(a.state)), "", { [weak self] in
            switch a.state.quest { case 1,2: self?.openEdit(); case 3: a.state = verse_haunt_quest(a.state,a.retain("publish")); a.save(); case 4,6: self?.share(); case 5: self?.newWorld(); case 7: self?.survey(); default: break }
        })])
    }
    private func worlds() {
        guard let a = assets else { return }
        menu("Worlds", a.worlds().map { url,name in SceneRow(name,"",{ [weak self] in do { try a.switchWorld(url) } catch { self?.showFailure(error.localizedDescription) } }) })
    }
    private func newWorld() {
        guard let a = assets else { return }
        let alert = UIAlertController(title: "Create", message: nil, preferredStyle: .alert)
        alert.addTextField { $0.placeholder = "World name" }
        alert.addAction(UIAlertAction(title: "Create", style: .default) { _ in a.createWorld(alert.textFields?.first?.text ?? "My house") })
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel)); present(alert, animated: true)
    }
    private func survey() {
        guard let a = assets else { return }
        let alert = UIAlertController(title: "How was it?", message: nil, preferredStyle: .alert)
        alert.addTextField { $0.placeholder = "Feedback" }
        alert.addAction(UIAlertAction(title: "Save", style: .default) { _ in a.state = verse_haunt_feedback(a.state,a.retain(alert.textFields?.first?.text ?? "")); a.save() })
        alert.addAction(UIAlertAction(title: "Cancel", style: .cancel)); present(alert, animated: true)
    }
    private func openMap() {
        guard let a = assets else { return }
        let map = HauntMap(assets: a); map.title = "Map"
        let nav = UINavigationController(rootViewController: map); nav.overrideUserInterfaceStyle = .dark
        map.navigationItem.rightBarButtonItem = UIBarButtonItem(systemItem: .done, primaryAction: UIAction { [weak nav] _ in nav?.dismiss(animated: true) })
        present(nav, animated: true)
    }
    deinit { clock?.invalidate(); mr_destroy(handle); NotificationCenter.default.removeObserver(self) }
}

private struct SceneRow { let title: String; let detail: String; let run: () -> Void; init(_ title: String,_ detail: String,_ run: @escaping () -> Void) { self.title=title;self.detail=detail;self.run=run } }
private final class SceneList: UITableViewController {
    let rows: [SceneRow]
    init(title: String, rows: [SceneRow]) { self.rows=rows;super.init(style:.insetGrouped);self.title=title }
    required init?(coder: NSCoder) { fatalError() }
    override func viewDidLoad() { super.viewDidLoad(); view.backgroundColor = .black; navigationItem.rightBarButtonItem = UIBarButtonItem(systemItem:.done,primaryAction:UIAction { [weak self] _ in self?.dismiss(animated:true) }) }
    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { rows.count }
    override func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell { let cell=UITableViewCell(style:.subtitle,reuseIdentifier:nil);let row=rows[indexPath.row];cell.textLabel?.text=row.title;cell.textLabel?.font=UIFont(name:"Michroma",size:12);cell.detailTextLabel?.text=row.detail;cell.accessibilityIdentifier="haunt.row."+row.title;return cell }
    override func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) { let row=rows[indexPath.row];dismiss(animated:true) { row.run() } }
}
private final class HauntMap: UIViewController {
    let assets: HauntAssets
    init(assets: HauntAssets) { self.assets=assets;super.init(nibName:nil,bundle:nil) }
    required init?(coder: NSCoder) { fatalError() }
    override func loadView() { view=MapSurface(assets:assets) }
}
private final class MapSurface: UIView {
    let assets: HauntAssets
    init(assets: HauntAssets) { self.assets=assets;super.init(frame:.zero);backgroundColor = .black }
    required init?(coder: NSCoder) { fatalError() }
    override func draw(_ rect: CGRect) {
        guard let c=UIGraphicsGetCurrentContext() else { return }
        var boxes = [coax_t_HauntMapRect](repeating: coax_t_HauntMapRect(), count: 66)
        let count = boxes.withUnsafeMutableBufferPointer { verse_haunt_map(assets.state,Float(bounds.width),Float(bounds.height),coax_v_t_HauntMapRect(data:$0.baseAddress,count:Int64($0.count))) }
        c.setLineWidth(1)
        for i in 0..<Int(count) { let b=boxes[i];let rect=CGRect(x:CGFloat(b.x),y:CGFloat(b.y),width:CGFloat(b.width),height:CGFloat(b.height))
            if b.kind<2 { c.setStrokeColor(UIColor(white:1,alpha:b.kind==0 ? 0.8:0.3).cgColor);c.stroke(rect) }
            else { c.setFillColor((b.kind==2 ? UIColor.white:UIColor.orange).cgColor);c.fillEllipse(in:rect) }
        }
    }
}
