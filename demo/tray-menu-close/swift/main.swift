// Isolation test: toggle each tao/tray-icon component via command-line flags.
// Usage:
//   ./tray-swift-demo                     → all components ON (full tao simulation)
//   ./tray-swift-demo --no-overlay        → no TrayTarget overlay
//   ./tray-swift-demo --no-timer          → no 0.1µs waker timer
//   ./tray-swift-demo --no-observers      → no run loop observers
//   ./tray-swift-demo --no-proxy          → no CFRunLoopSource proxy
//   Flags can be combined.

import AppKit
import Foundation

// MARK: - Config

let args = Set(CommandLine.arguments.dropFirst())
let useOverlay    = !args.contains("--no-overlay")
let useTimer      = !args.contains("--no-timer")
let useObservers  = !args.contains("--no-observers")
let useProxy      = !args.contains("--no-proxy")

// MARK: - EventLoopWaker  (tao observer.rs)

final class EventLoopWaker {
    private var timer: CFRunLoopTimer?

    func setup() {
        guard useTimer else { return }
        timer = CFRunLoopTimerCreateWithHandler(
            nil, Double.greatestFiniteMagnitude, 0.000_000_1, 0, 0,
            { _ in }
        )
        CFRunLoopAddTimer(CFRunLoopGetMain(), timer!, CFRunLoopMode.commonModes)
    }

    func start() {
        guard let t = timer else { return }
        CFRunLoopTimerSetNextFireDate(t, -.greatestFiniteMagnitude)
    }
}

// MARK: - AppState  (tao app_state.rs)

final class AppState {
    static let shared = AppState()
    var isReady = false
    var inCallback = false
    let waker = EventLoopWaker()

    func wakeup() {
        guard isReady, !inCallback else { return }
        inCallback = true
        inCallback = false
    }

    func cleared() {
        guard isReady, !inCallback else { return }
        inCallback = true
        inCallback = false
    }
}

// MARK: - EventProxy  (tao Proxy CFRunLoopSource)

final class EventProxy {
    static let shared = EventProxy()
    private var source: CFRunLoopSource?

    func setup() {
        guard useProxy else { return }
        var ctx = CFRunLoopSourceContext()
        ctx.version = 0
        ctx.perform = { _ in }
        source = CFRunLoopSourceCreate(nil, 0, &ctx)
        CFRunLoopAddSource(CFRunLoopGetMain(), source!, CFRunLoopMode.commonModes)
    }

    func signal() {
        guard let s = source else { return }
        CFRunLoopSourceSignal(s)
        CFRunLoopWakeUp(CFRunLoopGetMain())
    }
}

// MARK: - TrayTarget  (tray-icon TaoTrayTarget)

final class TrayTarget: NSView {
    weak var statusItem: NSStatusItem?

    override func mouseDown(with event: NSEvent) {
        EventProxy.shared.signal()
        guard let si = statusItem, let button = si.button,
              let menu = si.menu, menu.numberOfItems > 0 else { return }
        button.performClick(nil)
    }

    override func mouseUp(with event: NSEvent) {
        statusItem?.button?.highlight(false)
        EventProxy.shared.signal()
    }

    override func mouseExited(with event: NSEvent)  { EventProxy.shared.signal() }
    override func mouseEntered(with event: NSEvent) { EventProxy.shared.signal() }
    override func mouseMoved(with event: NSEvent)   { EventProxy.shared.signal() }

    override func updateTrackingAreas() {
        for area in trackingAreas { removeTrackingArea(area) }
        super.updateTrackingAreas()
        addTrackingArea(NSTrackingArea(
            rect: .zero,
            options: [.mouseEnteredAndExited, .mouseMoved, .activeAlways, .inVisibleRect],
            owner: self, userInfo: nil
        ))
    }
}

// MARK: - AppDelegate

final class AppDelegate: NSObject, NSApplicationDelegate {
    var statusItem: NSStatusItem!
    var trayTarget: TrayTarget?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.activate(ignoringOtherApps: true)
        AppState.shared.isReady = true
        AppState.shared.waker.start()

        AppState.shared.inCallback = true
        setupTrayIcon()
        AppState.shared.inCallback = false
    }

    func setupTrayIcon() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        statusItem.button?.title = "BUG-Swift"

        let menu = NSMenu()
        menu.addItem(NSMenuItem(title: "固定项", action: nil, keyEquivalent: ""))

        let submenu = NSMenu(title: "Agents")
        submenu.addItem(NSMenuItem(title: "Agent 0", action: nil, keyEquivalent: ""))
        let subItem = NSMenuItem(title: "Agents ← hover 这里", action: nil, keyEquivalent: "")
        subItem.submenu = submenu
        menu.addItem(subItem)

        menu.addItem(NSMenuItem(
            title: "退出", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q"
        ))

        statusItem.menu = menu

        if useOverlay, let button = statusItem.button {
            let t = TrayTarget(frame: button.frame)
            t.statusItem = statusItem
            t.wantsLayer = true
            button.addSubview(t)
            trayTarget = t
        }
    }
}

// MARK: - Main

let app = NSApplication.shared
app.setActivationPolicy(.regular)

EventProxy.shared.setup()
AppState.shared.waker.setup()

if useObservers {
    let beginObs = CFRunLoopObserverCreateWithHandler(
        nil, CFRunLoopActivity.afterWaiting.rawValue, true, CFIndex.min,
        { _, _ in AppState.shared.wakeup() }
    )!
    CFRunLoopAddObserver(CFRunLoopGetMain(), beginObs, CFRunLoopMode.commonModes)

    let endObs = CFRunLoopObserverCreateWithHandler(
        nil, CFRunLoopActivity.beforeWaiting.rawValue, true, CFIndex.max,
        { _, _ in AppState.shared.cleared() }
    )!
    CFRunLoopAddObserver(CFRunLoopGetMain(), endObs, CFRunLoopMode.commonModes)
}

let delegate = AppDelegate()
app.delegate = delegate

print("Config: overlay=\(useOverlay) timer=\(useTimer) observers=\(useObservers) proxy=\(useProxy)")
print("Click 'BUG-Swift' → hover 'Agents' submenu to test.")

app.run()
