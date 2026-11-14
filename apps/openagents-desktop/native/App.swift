// Firm reason: 6 (AppKit application lifecycle; application state is Coax).
import AppKit

@main struct NativeDesktopApp {
    @MainActor static func main() {
        if CommandLine.arguments.contains("--inspect-controls") {
            NSApplication.shared.setActivationPolicy(.prohibited)
            do {let url=Bundle.main.url(forResource:"Starter",withExtension:nil)!;let controller=try EnvironmentWindow(directory:url);try controller.inspectControls()}
            catch {fputs(("Native controls inspection failed: "+error.localizedDescription+"\n"),stderr);exit(1)}
            return
        }
        if CommandLine.arguments.contains("--inspect-program") {
            do {
                let url=Bundle.main.url(forResource:"Starter",withExtension:nil)!
                let p=try NativeProgram(directory:url,module:"app",diagnostic:{fputs(($0+"\n"),stderr)});try p.frame();_ = try p.action();try p.event(101,0,"{\"loggedIn\":false}");try p.frame()
                let elements=p.elements();guard elements.contains(where:{$0.id=="login.email"}),elements.contains(where:{$0.id=="login.send"}) else {exit(2)}
                print("Coax Native macOS: generated program and native login semantics passed")
            }catch {fputs(("Native program inspection failed: "+error.localizedDescription+"\n"),stderr);exit(1)}
            return
        }
        let app=NSApplication.shared;app.setActivationPolicy(.regular)
        let menu=NSMenu(),item=NSMenuItem(),application=NSMenu();application.addItem(withTitle:"Quit OpenAgents",action:#selector(NSApplication.terminate(_:)),keyEquivalent:"q");item.submenu=application;menu.addItem(item);app.mainMenu=menu
        oa_environments_open();app.run()
    }
}
