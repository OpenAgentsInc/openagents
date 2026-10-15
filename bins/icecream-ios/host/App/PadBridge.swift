import GameController
import WebKit

/// Game controllers drive the page as a keyboard and mouse would: the
/// page's script (`script`, injected at document start) turns each pad
/// state into the key and pointer events the game already understands.
///
/// Left stick / d-pad: w a s d (the d-pad also sends arrows, which page
/// through the tutorial). A: space (jump) and Enter. B: e (use). X: q (next
/// item). Y: v (camera view). Left shoulder or left stick press: Shift
/// (sprint). Right trigger or right shoulder: fire. Right stick: turn the
/// camera. Menu: Escape (pause menu). Hardware keyboards need nothing:
/// WKWebView hands their keys to the page.
final class PadBridge {
    weak var webView: WKWebView?
    private var last = ""
    private var observers: [NSObjectProtocol] = []

    func start() {
        let center = NotificationCenter.default
        observers.append(center.addObserver(forName: .GCControllerDidConnect, object: nil, queue: .main) { [weak self] note in
            if let controller = note.object as? GCController { self?.attach(controller) }
        })
        observers.append(center.addObserver(forName: .GCControllerDidDisconnect, object: nil, queue: .main) { [weak self] _ in
            self?.send(PadState())
        })
        GCController.controllers().forEach(attach)
        GCController.startWirelessControllerDiscovery {}
    }

    private func attach(_ controller: GCController) {
        guard let gamepad = controller.extendedGamepad else { return }
        gamepad.valueChangedHandler = { [weak self] pad, _ in
            self?.send(PadState(pad))
        }
    }

    private func send(_ state: PadState) {
        let json = state.json
        guard json != last else { return }
        last = json
        DispatchQueue.main.async { [weak self] in
            self?.webView?.evaluateJavaScript("window.__icecreamPad && window.__icecreamPad(\(json))")
        }
    }

    static let script = """
    (function () {
      var held = {};
      var orbit = null;
      var fire = false;
      var stick = { x: 0, y: 0 };
      function key(name, down) {
        if (!!held[name] === down) return;
        held[name] = down;
        window.dispatchEvent(new KeyboardEvent(down ? 'keydown' : 'keyup', { key: name, bubbles: true, cancelable: true }));
      }
      function canvas() { return document.getElementById('icecream-canvas') || document.querySelector('canvas'); }
      function pointer(type, button, x, y) {
        var c = canvas();
        if (!c) return;
        c.dispatchEvent(new PointerEvent(type, { pointerId: 77, pointerType: 'mouse', isPrimary: true,
          button: button, buttons: type === 'pointerup' ? 0 : (button === 2 ? 2 : 1),
          clientX: x, clientY: y, bubbles: true, cancelable: true }));
      }
      function turn() {
        if (!orbit) return;
        if (Math.abs(stick.x) < 0.01 && Math.abs(stick.y) < 0.01) {
          pointer('pointerup', 2, orbit.x, orbit.y);
          orbit = null;
          return;
        }
        orbit.x += stick.x * 9;
        orbit.y += stick.y * 6;
        pointer('pointermove', -1, orbit.x, orbit.y);
        requestAnimationFrame(turn);
      }
      window.__icecreamPad = function (s) {
        var dead = 0.3;
        key('w', s.ly > dead || s.up);
        key('s', s.ly < -dead || s.down);
        key('a', s.lx < -dead || s.left);
        key('d', s.lx > dead || s.right);
        key('arrowup', s.up);
        key('arrowdown', s.down);
        key('arrowleft', s.left);
        key('arrowright', s.right);
        key(' ', s.a);
        key('enter', s.a);
        key('e', s.b);
        key('q', s.x);
        key('v', s.y);
        key('shift', s.lb || s.l3);
        key('escape', s.menu);
        var firing = s.rt > 0.4 || s.rb;
        if (firing !== fire) {
          fire = firing;
          var c = canvas();
          var cx = c ? c.clientWidth / 2 : 0, cy = c ? c.clientHeight / 2 : 0;
          pointer(firing ? 'pointerdown' : 'pointerup', 0, cx, cy);
        }
        stick.x = Math.abs(s.rx) > 0.15 ? s.rx : 0;
        stick.y = Math.abs(s.ry) > 0.15 ? -s.ry : 0;
        if (!orbit && (stick.x || stick.y)) {
          var c2 = canvas();
          orbit = { x: c2 ? c2.clientWidth / 2 : 0, y: c2 ? c2.clientHeight / 2 : 0 };
          pointer('pointerdown', 2, orbit.x, orbit.y);
          requestAnimationFrame(turn);
        }
      };
    })();
    """
}

struct PadState {
    var lx: Float = 0, ly: Float = 0, rx: Float = 0, ry: Float = 0, rt: Float = 0
    var a = false, b = false, x = false, y = false, lb = false, rb = false, l3 = false, menu = false
    var up = false, down = false, left = false, right = false

    init() {}

    init(_ pad: GCExtendedGamepad) {
        lx = pad.leftThumbstick.xAxis.value
        ly = pad.leftThumbstick.yAxis.value
        rx = pad.rightThumbstick.xAxis.value
        ry = pad.rightThumbstick.yAxis.value
        rt = pad.rightTrigger.value
        a = pad.buttonA.isPressed
        b = pad.buttonB.isPressed
        x = pad.buttonX.isPressed
        y = pad.buttonY.isPressed
        lb = pad.leftShoulder.isPressed
        rb = pad.rightShoulder.isPressed
        l3 = pad.leftThumbstickButton?.isPressed ?? false
        menu = pad.buttonMenu.isPressed
        up = pad.dpad.up.isPressed
        down = pad.dpad.down.isPressed
        left = pad.dpad.left.isPressed
        right = pad.dpad.right.isPressed
    }

    var json: String {
        func n(_ v: Float) -> String { String(format: "%.2f", v) }
        return "{\"lx\":\(n(lx)),\"ly\":\(n(ly)),\"rx\":\(n(rx)),\"ry\":\(n(ry)),\"rt\":\(n(rt)),"
            + "\"a\":\(a),\"b\":\(b),\"x\":\(x),\"y\":\(y),\"lb\":\(lb),\"rb\":\(rb),\"l3\":\(l3),"
            + "\"menu\":\(menu),\"up\":\(up),\"down\":\(down),\"left\":\(left),\"right\":\(right)}"
    }
}
