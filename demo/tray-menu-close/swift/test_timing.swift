// Test: replicate the tao timing issue.
// tao's launched() sets .regular + activate BEFORE the setup hook can change to .accessory.
// Usage:
//   ./test_timing late-switch    → .regular + activate, then switch to .accessory (tao behavior)
//   ./test_timing early-switch   → .accessory BEFORE activate (proposed fix)

import AppKit

let mode = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "late-switch"

final class AppDelegate: NSObject, NSApplicationDelegate {
    var statusItem: NSStatusItem!

    func applicationDidFinishLaunching(_ notification: Notification) {
        // --- Replicate tao launched() ---
        if mode == "late-switch" {
            // tao default: .regular is already set, then activate
            NSApp.activate(ignoringOtherApps: true)
            // THEN setup hook changes policy to .accessory (too late!)
            NSApp.setActivationPolicy(.accessory)
        } else {
            // Proposed fix: set .accessory BEFORE activate
            NSApp.setActivationPolicy(.accessory)
            NSApp.activate(ignoringOtherApps: true)
        }

        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        statusItem.button?.title = "TIME"

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
        print("Mode: \(mode). Click 'TIME'.")
    }
}

let app = NSApplication.shared
// tao default: set .regular before app.run()
app.setActivationPolicy(.regular)

let d = AppDelegate()
app.delegate = d
app.run()
