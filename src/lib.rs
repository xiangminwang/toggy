#![cfg(windows)]
#![feature(thread_id_value)]

extern crate chrono;
use chrono::prelude::*;
use std::fmt::{Debug};

extern crate libc;

mod utils;
mod game;
use game::*;

use std::fs::{create_dir_all, File};
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use winapi::{
    shared::minwindef::{TRUE, BOOL, DWORD, HINSTANCE, LPVOID, WPARAM, LPARAM, LRESULT},
    um::{
        winnt::HANDLE,
        synchapi::{CreateEventA, WaitForSingleObject},
        processthreadsapi::{GetCurrentThreadId},
    },
    um::winuser::*,
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
        DLL_PROCESS_ATTACH => on_process_attach(unsafe { GetCurrentThreadId() }),
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

fn on_process_attach(dll_thread_id: DWORD) {
    unsafe {
        // Acquire new message queue from OS
        let queue_acquired = acquire_message_queue(dll_thread_id) > 0;

        if queue_acquired {
            // Register hook for the current thread
            let hook = SetWindowsHookExA(
                WH_KEYBOARD,            // WH_KEYBOARD: 2, WH_KEYBOARD_LL: 13
                Some(toggle_key_proc),
                std::ptr::null_mut(),   // lib_handle: HINSTANCE  std::ptr::null_mut()
                dll_thread_id,          // dwThreadId: DWORD      0
            );

            // log_file("hook address:", hook);

            let _listener_thread = std::thread::spawn(move || {
                let mut received_msg = MSG::default();
                loop {
                    let result = GetMessageA(&mut received_msg, std::ptr::null_mut(), WM_KEYFIRST, WM_KEYLAST);
                    log_file("Result", result);
                    // log_file(
                    //     format!("hwnd: {}, message: {}, wParam: {}, lParam: {}", received_msg.hwnd as u32, received_msg.message, received_msg.wParam, received_msg.lParam), 
                    //     format!("time: {}, ptX: {}, ptY: {}", received_msg.time, received_msg.pt.x, received_msg.pt.y)
                    // );
                }
            });

            // listener_thread.join().unwrap();

            UnhookWindowsHookEx(hook);
        }

        

        // let mut received_msg = MSG::default();
        // while { GetMessageA(&mut received_msg, std::ptr::null_mut(), WM_KEYFIRST, WM_KEYLAST) } != 0 {
        //     TranslateMessage(&mut received_msg);
        //     DispatchMessageA(&mut received_msg);

        //     log_file(
        //         format!("hwnd: {}, message: {}, wParam: {}, lParam: {}", received_msg.hwnd as u32, received_msg.message, received_msg.wParam, received_msg.lParam), 
        //         format!("time: {}, ptX: {}, ptY: {}", received_msg.time, received_msg.pt.x, received_msg.pt.y)
        //     );
        // }
        

        // std::thread::spawn(move || {
        //     let thread_id = GetCurrentThreadId();            
        //     let hook = SetWindowsHookExA(
        //         WH_KEYBOARD,            // WH_KEYBOARD: 2, WH_KEYBOARD_LL: 13
        //         Some(toggle_key_proc),
        //         std::ptr::null_mut(),   // lib_handle: HINSTANCE  std::ptr::null_mut()
        //         thread_id,              // dwThreadId: DWORD      0
        //     );

        //     let mut received_msg = MSG::default();
        //     while { GetMessageA(&mut received_msg, std::ptr::null_mut(), 0, 0) } != 0 {
        //         TranslateMessage(&mut received_msg);
        //         DispatchMessageA(&mut received_msg);

        //         log_file(
        //             format!("hwnd: {}, message: {}, wParam: {}, lParam: {}", received_msg.hwnd as u32, received_msg.message, received_msg.wParam, received_msg.lParam), 
        //             format!("time: {}, ptX: {}, ptY: {}", received_msg.time, received_msg.pt.x, received_msg.pt.y)
        //         );
        //     }

        //     UnhookWindowsHookEx(hook);
        // });
    };
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

    log_file(format!("player X({:?})", player_x), format!("New Count({:?})", new_count));
}

pub fn acquire_message_queue(dll_thread_id: DWORD) -> i32 {
    unsafe {
        // Create a anonymous event
        let event_obj: HANDLE = CreateEventA(std::ptr::null_mut(), 0, 1, std::ptr::null());

        // Will hang FOREVER here to wait for the object been signaled
        let reason_to_fail: u32 = WaitForSingleObject(event_obj, 0xFFFFFFFF);
        if reason_to_fail > 0 {
            log_file("Event not being signaled??", reason_to_fail);
        }

        let custom_defined_msg_code = 0x4ABC; // 0x4ABC = 19132
        let mut msg = MSG::default();
        PeekMessageA(&mut msg, std::ptr::null_mut(), WM_USER, WM_USER, PM_NOREMOVE);
        PostThreadMessageA(dll_thread_id, custom_defined_msg_code, 0, 0)
    }
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
