// extern crate chrono;
// use chrono::prelude::*;
// use std::fmt::{Debug};
// use std::io::{Write};
// use std::fs::{create_dir_all, File};

pub fn realign_unchecked<U, T>(data: &[U]) -> &[T] {
    unsafe { data.align_to().1 }
}

// pub fn log_file<T, F>(debug_info1: T, debug_info2: F)
// where
//     T: Debug,
//     F: Debug,
// {
//     let local_dt = Local::now().format("%H-%M-%S").to_string();
//     let pid = std::process::id().to_string();
//     let process_path = std::env::current_exe().unwrap();
//     let args: Vec<String> = std::env::args().collect();
//     let log_folder_path = "C:\\games\\Diablo II\\ToggyLogs";
//     let log_file_path = format!("{}\\{}[{:0>9}].txt", log_folder_path, local_dt, Local::now().nanosecond());
//     let output = format!(r"[*]             Pid: {:?}
// [*]         Process: {:?}
// [*]            Args: {:?}
// [*]      Created At: {:?}
// [*]    Debug Line 1: {:?}
// [*]    Debug Line 2: {:?}",
//         pid,
//         process_path,
//         &args[1..],
//         local_dt,
//         debug_info1,
//         debug_info2,
//     );

//     create_dir_all(log_folder_path).unwrap();
//     let mut file = File::create(log_file_path).unwrap();
//     file.write_all(output.as_bytes()).unwrap();
// }
