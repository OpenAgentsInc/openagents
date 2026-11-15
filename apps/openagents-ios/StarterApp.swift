import UIKit

@main final class StarterApp: UIResponder, UIApplicationDelegate {
    var window:UIWindow?
    func application(_ application:UIApplication,didRegisterForRemoteNotificationsWithDeviceToken token:Data) {Task {try? await AppAuth.shared.pushTarget(token)}}
    func application(_ application:UIApplication,didFinishLaunchingWithOptions options:[UIApplication.LaunchOptionsKey:Any]?=nil)->Bool {
        let window=UIWindow(frame:UIScreen.main.bounds);window.tintColor=NativeSkin.color(NativeSkin.tokens.ink)
        #if DEBUG
        if UserDefaults.standard.bool(forKey:"OAComponents") {
            window.rootViewController=UINavigationController(rootViewController:NativeComponents(closed:{}));window.makeKeyAndVisible();self.window=window;return true
        }
        #endif
        if SharedAppLaunch.on { window.rootViewController=SharedAppViewController(socket:SharedAppLaunch.socket,token:SharedAppLaunch.token); window.makeKeyAndVisible(); self.window=window; return true }
        do { window.rootViewController=NativeHost(updates:try NativeUpdates()) }
        catch { let vc=NativeControls();vc.render([NativeElement(id:"startup.error",role:"notice",label:"The embedded starter is unavailable.")]);window.rootViewController=vc }
        window.makeKeyAndVisible(); self.window=window; return true
    }
}

/// The book's first milestone (#13516): `-OASharedApp YES` opens the shared app by hand, and `-OADrive SOCKET`
/// opens it for coax drive on that socket. On a physical iPhone (#13517) SOCKET is `tcp:PORT` and `-OADriveToken
/// TOKEN` names the token a driver must attach with (phone-drive.c). Its services are recorded fakes; it reaches
/// no server.
enum SharedAppLaunch {
    static var socket: String? { UserDefaults.standard.string(forKey: "OADrive") }
    static var token: String? { UserDefaults.standard.string(forKey: "OADriveToken") }
    static var on: Bool { socket != nil || UserDefaults.standard.bool(forKey: "OASharedApp") }
}
