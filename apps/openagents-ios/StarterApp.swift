import UIKit

@main final class StarterApp: UIResponder, UIApplicationDelegate {
    var window:UIWindow?
    func application(_ application:UIApplication,didFinishLaunchingWithOptions options:[UIApplication.LaunchOptionsKey:Any]?=nil)->Bool {
        let window=UIWindow(frame:UIScreen.main.bounds)
        do { window.rootViewController=NativeHost(updates:try NativeUpdates()) }
        catch { let vc=UIViewController(); vc.view.backgroundColor = .black; let label=UILabel(frame:CGRect(x:24,y:100,width:320,height:100)); label.text="The embedded starter is unavailable."; label.textColor = .white; vc.view.addSubview(label); window.rootViewController=vc }
        window.makeKeyAndVisible(); self.window=window; return true
    }
}
