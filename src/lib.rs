#![cfg(windows)]
#![feature(thread_id_value)]

extern crate chrono;
use chrono::prelude::*;
use std::fmt::Debug;

mod utils;
mod game;
use game::*;

use std::fs::{create_dir_all, File};
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use winapi::{
    shared::windef::{HHOOK},
    shared::minwindef::{TRUE, BOOL, DWORD, HINSTANCE, LPVOID, WPARAM, LPARAM, LRESULT},
    um::winuser::{
        SetWindowsHookExA,
        UnhookWindowsHookEx,
        CallNextHookEx,
        // TranslateMessage,
        // DispatchMessageA,
        // GetMessageA,
        // MSG,
        HOOKPROC,
        HC_ACTION,
        WM_KEYUP,
        KBDLLHOOKSTRUCT,
    },
};

use std::ffi::CString;
use winapi::um::winuser::{MessageBoxA, MB_OK, MB_ICONINFORMATION};

static HOTKEY_PRESSED_COUNT: AtomicU32 = AtomicU32::new(0);
static HOTKEY_PROCESSING: AtomicBool = AtomicBool::new(false);

#[no_mangle]
#[allow(non_snake_case, unused_variables)]
extern "system" fn DllMain(dll_module: HINSTANCE, call_reason: DWORD, reserved: LPVOID) -> BOOL {
    const DLL_PROCESS_ATTACH: DWORD = 1;
    const DLL_PROCESS_DETACH: DWORD = 0;

    match call_reason {
        DLL_PROCESS_ATTACH => on_process_attach(),
        DLL_PROCESS_DETACH => on_process_detach(),
        _ => (),
    }
    TRUE
}

fn on_process_detach() {
    let lp_text = CString::new(format!("I'm detached!")).unwrap();
    let lp_caption = CString::new("DLL_PROCESS_DETACH").unwrap();
    unsafe {
        MessageBoxA(
            std::ptr::null_mut(),
            lp_text.as_ptr(),
            lp_caption.as_ptr(),
            MB_OK | MB_ICONINFORMATION
        );
    };

    ()
}

fn on_process_attach() {
    std::thread::spawn(move || {
        // odd -> high players, even -> low players
        loop {
            if HOTKEY_PROCESSING.load(Ordering::Relaxed) {
                continue;
            }

            let hook_id = install_hook(Some(toggle_key_proc));
            let lp_text = CString::new("Leave me here, I'll be gone with game session.").unwrap();
            let lp_caption = CString::new("Toggy for D2 1.13D").unwrap();
            unsafe {
                MessageBoxA(
                    std::ptr::null_mut(),
                    lp_text.as_ptr(),
                    lp_caption.as_ptr(),
                    MB_OK | MB_ICONINFORMATION
                );
            }

            // let interval = std::time::Duration::from_millis(2500);
            // std::thread::sleep(interval);
            unsafe { UnhookWindowsHookEx(hook_id) };
        }
    });

    // std::thread::spawn(|| {
    //     // Listen on keyboard events
    //     let mut msg = MSG::default();
    //     while unsafe { GetMessageA(&mut msg, std::ptr::null_mut(), 0, 0) } != 0 {
    //         unsafe {
    //             TranslateMessage(&mut msg);
    //             DispatchMessageA(&mut msg);
    //         }
    //         let lp_text = CString::new(format!("Result: {:?}|{:?}|{:?}|{:?}", msg.message, msg.wParam, msg.lParam, msg.time)).unwrap();
    //         let lp_caption = CString::new("debug in loop").unwrap();
    //         unsafe {
    //             MessageBoxA(
    //                 std::ptr::null_mut(),
    //                 lp_text.as_ptr(),
    //                 lp_caption.as_ptr(),
    //                 MB_OK | MB_ICONINFORMATION
    //             );
    //         };
    //     }
    // });
    // let thread_id_u64 = thread::current().id().as_u64();
    // let bytes_u64 = thread_id_u64.get().to_le_bytes();
    // let bytes_u32:[u8; 4] = [bytes_u64[0], bytes_u64[1], bytes_u64[2], bytes_u64[3]];
    // let thread_id_u32 = u32::from_le_bytes(bytes_u32); // very tricky here
}

#[allow(non_snake_case)]
pub fn install_hook(lpfn: HOOKPROC) -> HHOOK {
    unsafe {
        SetWindowsHookExA(
            13, // WH_KEYBOARD:2, WH_KEYBOARD_LL:13
            lpfn,
            std::ptr::null_mut(),
            0,
            //lib_handle: HINSTANCE  std::ptr::null_mut()
            //dwThreadId: DWORD      0
        )
    }
}

#[no_mangle]
#[allow(non_snake_case)]
extern "system" fn toggle_key_proc(nCode: i32, wParam: WPARAM, lParam: LPARAM) -> LRESULT {
    unsafe {
        let hook_struct: *mut KBDLLHOOKSTRUCT = std::mem::transmute(lParam);
        let hook_struct = hook_struct.as_ref().unwrap();

        if nCode == HC_ACTION && wParam == WM_KEYUP as usize {
            if hook_struct.vkCode == 0xC0 {
                HOTKEY_PROCESSING.store(true, Ordering::Relaxed);
                // x & 1 = 1 -> odd, x & 1 = 0 -> even
                let prev_count = HOTKEY_PRESSED_COUNT.fetch_add(1, Ordering::SeqCst);
                change_playerx(
                    if prev_count & 1 == 0 { 3 } else { 8 },
                    HOTKEY_PRESSED_COUNT.load(Ordering::SeqCst)
                );
                HOTKEY_PROCESSING.store(false, Ordering::Relaxed);
            }
        }

        CallNextHookEx(std::ptr::null_mut(), nCode, wParam, lParam)
    }
}

pub fn change_playerx(number: u8, new_count: u32) {
    // Get Game Process
    let game_process = GameProcess::new().unwrap();

    // Get Loaded Modules
    let mut loaded_modules = game_process.get_modules().unwrap();
    let d2game_dll_module = loaded_modules.iter_mut().find(|x| x.name == "D2Game.dll").unwrap();

    // Deal with write memory requests
    let _player_x = unsafe { d2game_dll_module.write::<u8>(1121348, number) };
    
    // Read PlayerX variable
    let player_x = unsafe { d2game_dll_module.read::<u8>(1121348) };

    
    log_file(player_x, new_count);
}


pub fn log_file<T, F>(info1: T, info2: F)
where 
    T: Debug,
    F: Debug,
{
    let local_dt = Local::now().format("%H %M %S").to_string();
    let pid = std::process::id().to_string();
    let process_path = std::env::current_exe().unwrap();
    let args: Vec<String> = std::env::args().collect();
    let log_folder_path = "C:\\games\\Diablo II\\logs";
    let log_file_path = format!("{}\\toggy-{}.log", log_folder_path, local_dt);
    let output = format!(r"[*]             Pid: {:?}
[*]         Process: {:?}
[*]            Args: {:?}
[*]      Created At: {:?}
[*]        player X: {:?}
[*]       New Count: {:?}",
        pid,
        process_path,
        &args[1..],
        local_dt,
        info1,
        info2,
    );

    create_dir_all(log_folder_path).unwrap();
    let mut file = File::create(log_file_path).unwrap();
    file.write_all(output.as_bytes()).unwrap();
}
