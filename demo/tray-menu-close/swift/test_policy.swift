// Test: activation policy effect on submenu hover.
// Usage:
//   ./test_policy regular       → .regular policy + activate
//   ./test_policy accessory     → .accessory policy, no activate
//   ./test_policy reg-no-act    → .regular policy, NO activate
//   ./test_policy acc-act       → .accessory policy + activate

import AppKit

let mode = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "regular"

final class AppDelegate: NSObject, NSApplicationDelegate {
    var statusItem: NSStatusItem!

    func applicationDidFinishLaunching(_ notification: Notification) {
        if mode == "regular" || mode == "acc-act" {
            NSApp.activate(ignoringOtherApps: true)
        }

        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        statusItem.button?.title = "TEST"

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
        print("Mode: \(mode). Click 'TEST'.")
    }
}

let app = NSApplication.shared
switch mode {
case "accessory", "acc-act":
    app.setActivationPolicy(.accessory)
default:
    app.setActivationPolicy(.regular)
}

let d = AppDelegate()
app.delegate = d
app.run()
