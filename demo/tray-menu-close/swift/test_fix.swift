// Test fix: replicate the exact AskHuman/tao timing, then apply fix.
// Usage:
//   ./test_fix broken      → tao default (.regular + activate, then setup sets .accessory) — BUG
//   ./test_fix deactivate  → same + deactivate() after switching to .accessory
//   ./test_fix reactivate  → same + deactivate() + activate(false)

import AppKit

let mode = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "broken"

final class AppDelegate: NSObject, NSApplicationDelegate {
    var statusItem: NSStatusItem!

    func applicationDidFinishLaunching(_ notification: Notification) {
        // === tao launched() sequence ===
        // apply_activation_policy → .regular (tao default, already set before app.run)
        // activateIgnoringOtherApps(true)
        NSApp.activate(ignoringOtherApps: true)
        print("tao: activated with .regular policy")

        // === Tauri setup hook ===
        NSApp.setActivationPolicy(.accessory)
        print("setup: switched to .accessory")

        // === Apply fix ===
        switch mode {
        case "deactivate":
            NSApp.deactivate()
            print("fix: deactivated")
        case "reactivate":
            NSApp.deactivate()
            NSApp.activate(ignoringOtherApps: false)
            print("fix: deactivated + reactivated(false)")
        default:
            print("no fix applied (broken)")
        }

        // === Create tray icon (same as AskHuman gui_host setup) ===
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        statusItem.button?.title = "FIX"

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
        print("Mode: \(mode). Click 'FIX'.")
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.regular)  // tao default

let d = AppDelegate()
app.delegate = d
app.run()
