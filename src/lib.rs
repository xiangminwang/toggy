#![cfg(windows)]

use std::sync::atomic::{AtomicPtr, AtomicBool, Ordering};
use winapi::{
    shared::{
        windef::{HHOOK},
        minwindef::{TRUE, BOOL, DWORD, HINSTANCE, LPVOID, LPDWORD, WPARAM, LPARAM, LRESULT},
    },
    um::{
        processthreadsapi::{GetCurrentProcess},
        winuser::{
            GetForegroundWindow, GetWindowThreadProcessId,
            UnhookWindowsHookEx, SetWindowsHookExA,
            GetMessageA, TranslateMessage, DispatchMessageA,
            MSG, CallNextHookEx, KBDLLHOOKSTRUCT,
            WH_KEYBOARD_LL, WM_KEYFIRST, WM_KEYLAST, HC_ACTION, WM_KEYUP,
        },
    },
};

mod utils;
mod module;
mod config;

use module::*;
use config::*;

static GLOBAL_HOOK_PTR: AtomicPtr<HHOOK> = AtomicPtr::new(std::ptr::null_mut());
static HOTKEY_PROCESSING: AtomicBool = AtomicBool::new(false);

#[no_mangle]
#[allow(non_snake_case, unused_variables)]
extern "system" fn DllMain(dll_module: HINSTANCE, call_reason: DWORD, reserved: LPVOID) -> BOOL {
    const DLL_PROCESS_ATTACH: DWORD = 1;
    const DLL_PROCESS_DETACH: DWORD = 0;

    match call_reason {
        DLL_PROCESS_ATTACH => on_process_attach(),
        DLL_PROCESS_DETACH => unsafe { UnhookWindowsHookEx(*GLOBAL_HOOK_PTR.load(Ordering::SeqCst)); },
        _ => (),
    }
    TRUE
}

fn on_process_attach() {
    std::thread::spawn(|| {
        // Register global hook, thread hook would not work since we're a non GUI thread
        let mut hook = unsafe {
            SetWindowsHookExA(
                WH_KEYBOARD_LL,          // WH_KEYBOARD: 2, WH_KEYBOARD_LL: 13
                Some(toggle_key_proc),
                std::ptr::null_mut(),    // lib_handle: HINSTANCE  std::ptr::null_mut()
                0,                       // dwThreadId: DWORD      GetCurrentThreadId()
            )
        };
        GLOBAL_HOOK_PTR.store(&mut hook, Ordering::SeqCst);

        let mut received_msg = MSG::default();
        unsafe {
            while GetMessageA(&mut received_msg, std::ptr::null_mut(), WM_KEYFIRST, WM_KEYLAST) != 0 {
                TranslateMessage(&mut received_msg);
                DispatchMessageA(&mut received_msg);
            }
        }
    });
}

#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn toggle_key_proc(nCode: i32, wParam: WPARAM, lParam: LPARAM) -> LRESULT {
    unsafe {
        // First filter
        if nCode != HC_ACTION || wParam != WM_KEYUP as usize {
            return CallNextHookEx(std::ptr::null_mut(), nCode, wParam, lParam);
        }

        // Since this's global hook, we must find the right event from the game
        let mut some_process_id: DWORD = 0;
        GetWindowThreadProcessId(GetForegroundWindow(), &mut some_process_id as LPDWORD);

        if some_process_id != std::process::id() {
            return CallNextHookEx(std::ptr::null_mut(), nCode, wParam, lParam);
        }

        // Real business starts here
        let hook_struct: *mut KBDLLHOOKSTRUCT = std::mem::transmute(lParam);
        let hook_struct = hook_struct.as_ref().unwrap();

        // Load config from ini
        let mut config = Config::new();
        let config = config.reload("toggy.ini");

        if hook_struct.vkCode == config.hotkey {
            HOTKEY_PROCESSING.store(true, Ordering::Relaxed);

            let mut d2_client = Module::find_in_process(GetCurrentProcess(), "D2Client.dll").unwrap();
            let mut d2_game = Module::find_in_process(GetCurrentProcess(), "D2Game.dll").unwrap();

            // Toggle playerx based upon current weapon set
            match get_current_weapon_set(&mut d2_client) {
                0 => update_playerx(&mut d2_game, config.lower_players),
                1 => update_playerx(&mut d2_game, config.upper_players),
                _ => panic!("get_current_weapon_set not giving us 0 or 1!"),
            };

            HOTKEY_PROCESSING.store(false, Ordering::Relaxed);
        }

        CallNextHookEx(std::ptr::null_mut(), nCode, wParam, lParam)
    }
}

// Set Player X -> D2Game.dll+111C44
pub fn update_playerx(module: &mut Module, number: u8) -> u8 {
    unsafe {
        module.write::<u8>(usize::from_str_radix("111C44", 16).unwrap(), number);
        *module.read::<u8>(usize::from_str_radix("111C44", 16).unwrap())
    }
}

// Get Weapon Set -> D2Client.dll+11CB84
pub fn get_current_weapon_set(module: &mut Module) -> u8 {
    unsafe {
        *module.read::<u8>(usize::from_str_radix("11CB84", 16).unwrap())
    }
}
