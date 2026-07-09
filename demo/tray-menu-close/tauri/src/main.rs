#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItemBuilder, MenuItem, Submenu, SubmenuBuilder};
use tauri::{Manager, Wry};

struct State { _menu: Menu<Wry>, _sub: Submenu<Wry>, _item: MenuItem<Wry> }

mod swizzle {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::time::Instant;

    static INSTALLED: AtomicBool = AtomicBool::new(false);
    static TRAY_HOOKS_INSTALLED: AtomicBool = AtomicBool::new(false);
    static MENU_OPEN: AtomicBool = AtomicBool::new(false);
    static EPOCH: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

    fn ts() -> String {
        let elapsed = EPOCH.get_or_init(Instant::now).elapsed();
        format!("{:.3}s", elapsed.as_secs_f64())
    }

    type IMP = unsafe extern "C" fn(*mut c_void, *mut c_void);
    type IMP1 = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void);

    static mut ORIG_CANCEL: Option<IMP> = None;
    static mut ORIG_CANCEL_NO_ANIM: Option<IMP> = None;
    static mut ORIG_PERFORM_CLICK: Option<IMP1> = None;

    // TaoTrayTarget hooks
    static mut ORIG_TT_MOUSE_DOWN: Option<IMP1> = None;
    static mut ORIG_TT_MOUSE_UP: Option<IMP1> = None;
    static mut ORIG_TT_MOUSE_EXITED: Option<IMP1> = None;
    static mut ORIG_TT_MOUSE_ENTERED: Option<IMP1> = None;
    static mut ORIG_TT_MOUSE_MOVED: Option<IMP1> = None;

    // NSMenu tracking hooks
    static mut ORIG_MENU_WILL_SEND_ACTION: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void)> = None;

    extern "C" {
        fn class_getInstanceMethod(cls: *mut c_void, sel: *mut c_void) -> *mut c_void;
        fn method_setImplementation(method: *mut c_void, imp: *mut c_void) -> *mut c_void;
        fn objc_getClass(name: *const u8) -> *mut c_void;
        fn sel_registerName(name: *const u8) -> *mut c_void;
    }

    // --- NSMenu hooks ---
    unsafe extern "C" fn hooked_cancel(this: *mut c_void, sel: *mut c_void) {
        eprintln!("[{ts}] ===== [NSMenu cancelTracking] =====", ts = ts());
        short_backtrace();
        if let Some(orig) = ORIG_CANCEL { orig(this, sel); }
    }

    unsafe extern "C" fn hooked_cancel_no_anim(this: *mut c_void, sel: *mut c_void) {
        eprintln!("[{ts}] ===== [NSMenu cancelTrackingWithoutAnimation] =====", ts = ts());
        short_backtrace();
        if let Some(orig) = ORIG_CANCEL_NO_ANIM { orig(this, sel); }
    }

    // --- NSStatusBarButton hooks ---
    unsafe extern "C" fn hooked_perform_click(this: *mut c_void, sel: *mut c_void, sender: *mut c_void) {
        eprintln!("[{ts}] >>> [NSStatusBarButton performClick:] ENTER (menu tracking begins)", ts = ts());
        MENU_OPEN.store(true, Ordering::SeqCst);
        if let Some(orig) = ORIG_PERFORM_CLICK { orig(this, sel, sender); }
        MENU_OPEN.store(false, Ordering::SeqCst);
        eprintln!("[{ts}] <<< [NSStatusBarButton performClick:] EXIT (menu tracking ended)", ts = ts());
    }

    // --- TaoTrayTarget hooks ---
    unsafe extern "C" fn hooked_tt_mouse_down(this: *mut c_void, sel: *mut c_void, event: *mut c_void) {
        let during = if MENU_OPEN.load(Ordering::SeqCst) { " [DURING MENU TRACKING!]" } else { "" };
        eprintln!("[{ts}] [TaoTrayTarget mouseDown:]{during}", ts = ts());
        if let Some(orig) = ORIG_TT_MOUSE_DOWN { orig(this, sel, event); }
    }

    unsafe extern "C" fn hooked_tt_mouse_up(this: *mut c_void, sel: *mut c_void, event: *mut c_void) {
        let during = if MENU_OPEN.load(Ordering::SeqCst) { " [DURING MENU TRACKING!]" } else { "" };
        eprintln!("[{ts}] [TaoTrayTarget mouseUp:]{during}", ts = ts());
        if let Some(orig) = ORIG_TT_MOUSE_UP { orig(this, sel, event); }
    }

    unsafe extern "C" fn hooked_tt_mouse_exited(this: *mut c_void, sel: *mut c_void, event: *mut c_void) {
        let during = if MENU_OPEN.load(Ordering::SeqCst) { " [DURING MENU TRACKING!]" } else { "" };
        eprintln!("[{ts}] *** [TaoTrayTarget mouseExited:]{during} ***", ts = ts());
        if during.is_empty() {
            if let Some(orig) = ORIG_TT_MOUSE_EXITED { orig(this, sel, event); }
        } else {
            eprintln!("[{ts}] ^^^ SKIPPING original handler to test if this fixes the bug!", ts = ts());
            // DON'T call original - test if blocking this fixes it
        }
    }

    unsafe extern "C" fn hooked_tt_mouse_entered(this: *mut c_void, sel: *mut c_void, event: *mut c_void) {
        let during = if MENU_OPEN.load(Ordering::SeqCst) { " [DURING MENU TRACKING!]" } else { "" };
        eprintln!("[{ts}] [TaoTrayTarget mouseEntered:]{during}", ts = ts());
        if let Some(orig) = ORIG_TT_MOUSE_ENTERED { orig(this, sel, event); }
    }

    unsafe extern "C" fn hooked_tt_mouse_moved(this: *mut c_void, sel: *mut c_void, event: *mut c_void) {
        let during = if MENU_OPEN.load(Ordering::SeqCst) { " [DURING MENU TRACKING!]" } else { "" };
        if !during.is_empty() {
            eprintln!("[{ts}] [TaoTrayTarget mouseMoved:]{during}", ts = ts());
        }
        if let Some(orig) = ORIG_TT_MOUSE_MOVED { orig(this, sel, event); }
    }

    fn short_backtrace() {
        let bt = std::backtrace::Backtrace::force_capture();
        let s = format!("{bt}");
        for (i, line) in s.lines().enumerate() {
            if i > 15 { eprintln!("   ... (truncated)"); break; }
            eprintln!("{line}");
        }
    }

    pub fn install() {
        if INSTALLED.swap(true, Ordering::SeqCst) { return; }
        EPOCH.get_or_init(Instant::now);
        unsafe {
            let cls = objc_getClass(b"NSMenu\0".as_ptr());
            assert!(!cls.is_null());
            hook(cls, b"cancelTracking\0", hooked_cancel as *mut c_void, &mut ORIG_CANCEL);
            hook(cls, b"cancelTrackingWithoutAnimation\0", hooked_cancel_no_anim as *mut c_void, &mut ORIG_CANCEL_NO_ANIM);

            let cls = objc_getClass(b"NSStatusBarButton\0".as_ptr());
            assert!(!cls.is_null());
            hook1(cls, b"performClick:\0", hooked_perform_click as *mut c_void, &mut ORIG_PERFORM_CLICK);

            eprintln!("[{ts}] [swizzle] base hooks installed", ts = ts());
        }
    }

    /// Must be called AFTER tray icon is built (TaoTrayTarget class registered)
    pub fn install_tray_target_hooks() {
        if TRAY_HOOKS_INSTALLED.swap(true, Ordering::SeqCst) { return; }
        unsafe {
            let cls = objc_getClass(b"TaoTrayTarget\0".as_ptr());
            if cls.is_null() {
                eprintln!("[{ts}] WARNING: TaoTrayTarget class not found!", ts = ts());
                return;
            }
            hook1(cls, b"mouseDown:\0", hooked_tt_mouse_down as *mut c_void, &mut ORIG_TT_MOUSE_DOWN);
            hook1(cls, b"mouseUp:\0", hooked_tt_mouse_up as *mut c_void, &mut ORIG_TT_MOUSE_UP);
            hook1(cls, b"mouseExited:\0", hooked_tt_mouse_exited as *mut c_void, &mut ORIG_TT_MOUSE_EXITED);
            hook1(cls, b"mouseEntered:\0", hooked_tt_mouse_entered as *mut c_void, &mut ORIG_TT_MOUSE_ENTERED);
            hook1(cls, b"mouseMoved:\0", hooked_tt_mouse_moved as *mut c_void, &mut ORIG_TT_MOUSE_MOVED);

            eprintln!("[{ts}] [swizzle] TaoTrayTarget hooks installed!", ts = ts());
        }
    }

    unsafe fn hook(cls: *mut c_void, sel_name: &[u8], new_imp: *mut c_void, orig: &mut Option<IMP>) {
        let sel = sel_registerName(sel_name.as_ptr());
        let m = class_getInstanceMethod(cls, sel);
        if !m.is_null() {
            let old = method_setImplementation(m, new_imp);
            *orig = Some(std::mem::transmute(old));
        }
    }

    unsafe fn hook1(cls: *mut c_void, sel_name: &[u8], new_imp: *mut c_void, orig: &mut Option<IMP1>) {
        let sel = sel_registerName(sel_name.as_ptr());
        let m = class_getInstanceMethod(cls, sel);
        if !m.is_null() {
            let old = method_setImplementation(m, new_imp);
            *orig = Some(std::mem::transmute(old));
        }
    }
}

fn main() {
    swizzle::install();

    tauri::Builder::default()
        .setup(|app| {
            let menu = Menu::<Wry>::new(app)?;
            menu.append(&MenuItemBuilder::with_id("f", "固定项").build(app)?)?;
            let item = MenuItemBuilder::with_id("a0", "Agent 0").build(app)?;
            let sub: Submenu<Wry> =
                SubmenuBuilder::with_id(app, "a", "Agents ← hover 这里").build()?;
            sub.append(&item)?;
            menu.append(&sub)?;
            menu.append(&MenuItemBuilder::with_id("q", "退出").build(app)?)?;

            let mut builder = tauri::tray::TrayIconBuilder::with_id("t")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .title("BUG")
                .tooltip("BUG DEMO");
            if let Some(icon) = app.default_window_icon().cloned() {
                builder = builder.icon(icon).icon_as_template(false);
            }
            builder.build(app)?;

            // Install TaoTrayTarget hooks AFTER the tray icon is built
            swizzle::install_tray_target_hooks();

            app.manage(Arc::new(Mutex::new(State {
                _menu: menu, _sub: sub, _item: item
            })));
            Ok(())
        })
        .on_menu_event(|app, e| { if e.id().as_ref() == "q" { app.exit(0); } })
        .run(tauri::generate_context!())
        .expect("error");
}
