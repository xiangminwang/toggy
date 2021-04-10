#![cfg(windows)]
#![feature(thread_id_value)]

extern crate chrono;
use chrono::prelude::*;
use std::fmt::{Debug};

mod utils;
mod game;

use std::{
    str::FromStr,
    fs::{create_dir_all, File},
    io::{Write},
    sync::atomic::*,
};
use winapi::{
    shared::{
        windef::{HHOOK},
        minwindef::{TRUE, BOOL, DWORD, HINSTANCE, LPVOID, WPARAM, LPARAM, LRESULT},
    },
    um::winuser::{
        UnhookWindowsHookEx, SetWindowsHookExA, GetForegroundWindow,
        GetMessageA, TranslateMessage, DispatchMessageA,
        MSG, CallNextHookEx, KBDLLHOOKSTRUCT, 
        WH_KEYBOARD_LL, WM_KEYFIRST, WM_KEYLAST, HC_ACTION, WM_KEYUP,
    },
};

use ini::Ini;
use game::*;

static GLOBAL_HOOK_PTR: AtomicPtr<HHOOK> = AtomicPtr::new(std::ptr::null_mut());
static UNIQUE_HWND: AtomicUsize = AtomicUsize::new(0);
static HOTKEY_PRESSED_COUNT: AtomicU32 = AtomicU32::new(0);
static HOTKEY_PROCESSING: AtomicBool = AtomicBool::new(false);
static HOTKEY: AtomicU32 = AtomicU32::new(192);
static LOWER_PLAYERS: AtomicU8 = AtomicU8::new(1);
static UPPER_PLAYERS: AtomicU8 = AtomicU8::new(8);

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
    apply_conf();

    std::thread::spawn(|| {
        for _ in 0..3 {
            std::thread::sleep(std::time::Duration::from_secs(3));
            UNIQUE_HWND.store(unsafe { GetForegroundWindow() } as usize, Ordering::SeqCst);
        }
    });

    std::thread::spawn(|| {
        // Register global hook, thread hook would not work since we're a non GUI thread
        unsafe {
            let mut hook = SetWindowsHookExA(
                WH_KEYBOARD_LL,          // WH_KEYBOARD: 2, WH_KEYBOARD_LL: 13
                Some(toggle_key_proc),
                std::ptr::null_mut(),    // lib_handle: HINSTANCE  std::ptr::null_mut()
                0,                       // dwThreadId: DWORD      GetCurrentThreadId()
            );
            GLOBAL_HOOK_PTR.store(&mut hook, Ordering::SeqCst);

            let mut received_msg = MSG::default();
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
        let callback_hwnd = GetForegroundWindow();
        if UNIQUE_HWND.load(Ordering::SeqCst).clone() != callback_hwnd.clone() as usize {
            return CallNextHookEx(std::ptr::null_mut(), nCode, wParam, lParam);
        }

        let hook_struct: *mut KBDLLHOOKSTRUCT = std::mem::transmute(lParam);
        let hook_struct = hook_struct.as_ref().unwrap();

        let hotkey = HOTKEY.load(Ordering::SeqCst);
        let lower_players = LOWER_PLAYERS.load(Ordering::SeqCst);
        let upper_players = UPPER_PLAYERS.load(Ordering::SeqCst);

        if nCode == HC_ACTION && wParam == WM_KEYUP as usize {
            if hook_struct.vkCode == hotkey {
                HOTKEY_PROCESSING.store(true, Ordering::Relaxed);
                // x & 1 = 1 -> odd, x & 1 = 0 -> even
                let prev_count = HOTKEY_PRESSED_COUNT.fetch_add(1, Ordering::SeqCst);
                change_playerx(
                    if prev_count & 1 == 0 { lower_players } else { upper_players },
                    HOTKEY_PRESSED_COUNT.load(Ordering::SeqCst)
                );
                HOTKEY_PROCESSING.store(false, Ordering::Relaxed);
            }
        }

        CallNextHookEx(std::ptr::null_mut(), nCode, wParam, lParam)
    }
}

// Apply Configurations
pub fn apply_conf() {
    let sections = Ini::load_from_file("toggy.ini").unwrap();

    for (section, properties) in sections.iter() {
        match section {
            Some("TOGGLE PLAYERS") => {
                for (k, v) in properties.iter() {
                    match k {
                        "hotkey" => HOTKEY.store(u32::from_str_radix(v.trim_start_matches("0x"), 16).unwrap(), Ordering::SeqCst),
                        "lower_players" => LOWER_PLAYERS.store(u8::from_str(v).unwrap(), Ordering::SeqCst),
                        "upper_players" => UPPER_PLAYERS.store(u8::from_str(v).unwrap(), Ordering::SeqCst),
                        _ => (),
                    }
                }
            },
            _ => (),
        }
    }
}

// Current Weapon Set -> D2Game.dll+111C44
pub fn change_playerx(number: u8, _new_count: u32) {
    // Get Game Process
    let game_process = GameProcess::new().unwrap();

    // Get Loaded Modules
    let mut loaded_modules = game_process.get_modules().unwrap();
    let d2game_dll_module = loaded_modules.iter_mut().find(|x| x.name == "D2Game.dll").unwrap();

    // Deal with write memory requests
    let _player_x = unsafe { d2game_dll_module.write::<u8>(1121348, number) };

    // Read PlayerX variable
    let __player_x = unsafe { d2game_dll_module.read::<u8>(1121348) };

    // log_file(format!("player X({:?})", player_x), format!("New Count({:?})", new_count));
    // log_file(UNIQUE_HWND.load(Ordering::SeqCst).clone(), callback_hwnd.clone() as usize);
}

// Current Weapon Set -> D2Client.dll+11CB84
pub fn _check_current_weapon_set() {
    unimplemented!()
}

pub fn log_file<T, F>(debug_info1: T, debug_info2: F)
where
    T: Debug,
    F: Debug,
{
    let local_dt = Local::now().format("%H-%M-%S").to_string();
    let pid = std::process::id().to_string();
    let process_path = std::env::current_exe().unwrap();
    let args: Vec<String> = std::env::args().collect();
    let log_folder_path = "C:\\games\\Diablo II\\logs";
    let log_file_path = format!("{}\\tg-{}.txt", log_folder_path, format!("{}-{}", local_dt, Local::now().nanosecond()));
    let output = format!(r"[*]             Pid: {:?}
[*]         Process: {:?}
[*]            Args: {:?}
[*]      Created At: {:?}
[*]    Debug Line 1: {:?}
[*]    Debug Line 2: {:?}",
        pid,
        process_path,
        &args[1..],
        local_dt,
        debug_info1,
        debug_info2,
    );

    create_dir_all(log_folder_path).unwrap();
    let mut file = File::create(log_file_path).unwrap();
    file.write_all(output.as_bytes()).unwrap();
}
