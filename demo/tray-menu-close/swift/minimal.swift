// Absolute minimum: just NSStatusItem + menu with submenu.
// No activation hack, no observers, no timers, no overlay.

import AppKit

final class AppDelegate: NSObject, NSApplicationDelegate {
    var statusItem: NSStatusItem!

    func applicationDidFinishLaunching(_ notification: Notification) {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        statusItem.button?.title = "MIN"

        let menu = NSMenu()
        menu.addItem(NSMenuItem(title: "固定项", action: nil, keyEquivalent: ""))

        let submenu = NSMenu(title: "Agents")
        submenu.addItem(NSMenuItem(title: "Agent 0", action: nil, keyEquivalent: ""))
        let subItem = NSMenuItem(title: "Agents ← hover", action: nil, keyEquivalent: "")
        subItem.submenu = submenu
        menu.addItem(subItem)

        menu.addItem(NSMenuItem(
            title: "退出", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q"
        ))

        statusItem.menu = menu
        print("Minimal demo ready. Click 'MIN' in menu bar.")
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let d = AppDelegate()
app.delegate = d
app.run()
