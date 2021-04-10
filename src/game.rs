use super::utils::*;

use std::io::{Error, ErrorKind};
use winapi::shared::minwindef::{DWORD, HMODULE}; // LPVOID
use winapi::um::{processthreadsapi, psapi::MODULEINFO, winnt::HANDLE}; //libloaderapi

#[derive(Debug)]
pub struct GameProcess {
    handle: HANDLE,
    pid: u32,
    // base: HMODULE,
    // base_addr: usize,
}

#[derive(Debug)]
pub struct Module {
    pub name: String,
    pub base: usize,
    pub size: usize,
}

impl Module {
    fn fix_offset(&self, offset: usize) -> usize {
        (self.base as usize) + offset
    }

    pub unsafe fn read<T>(&self, offset: usize) -> &T {
        &*(self.fix_offset(offset) as *const T)
    }

    pub unsafe fn write<T>(&mut self, offset: usize, value: T) {
        *(self.fix_offset(offset) as *mut T) = value;
    }
}

impl GameProcess {
    pub fn new() -> Result<Self, Error> {
        let handle: HANDLE = unsafe { processthreadsapi::GetCurrentProcess() };

        if handle == std::ptr::null_mut() {
            Err(Error::last_os_error())
        } else {
            let pid: u32 = unsafe { processthreadsapi::GetProcessId(handle) };
            // let base = unsafe { libloaderapi::GetModuleHandleA(std::ptr::null()) };
            // let base_addr = unsafe { *(base as LPVOID as *mut u32) } as usize;

            Ok(GameProcess { 
                handle, 
                pid,
                // base,
                // base_addr,
            })
        }
    }

    pub fn close_handle(&self) -> Result<(), Error> {
        if unsafe { winapi::um::handleapi::CloseHandle(self.handle) } == 0 {
            Err(Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub fn get_modules(&self) -> Result<Vec<Module>, Error> {
        use winapi::um::psapi::{EnumProcessModules, GetModuleBaseNameA};

        let mut size_needed: DWORD = 0;

        let result = unsafe { EnumProcessModules(self.handle, std::ptr::null_mut(), 0, &mut size_needed) };

        if result == 0 {
            return Err(Error::last_os_error());
        }

        let handle_size = std::mem::size_of::<HMODULE>() as u32;
        let module_count = size_needed / handle_size;
        let mut modules: Vec<HMODULE> = vec![std::ptr::null_mut(); module_count as usize];
        let result = unsafe {
            EnumProcessModules(
                self.handle,
                modules.as_mut_ptr(),
                module_count * handle_size,
                &mut size_needed,
            )
        };

        if result == 0 {
            return Err(Error::new(
                ErrorKind::Other,
                "failed to enumerate process modules",
            ));
        }

        const MODULE_NAME_LEN: usize = 50;
        let mut module_name_buf: [i8; MODULE_NAME_LEN] = [0; MODULE_NAME_LEN];
        let mut valid_modules: Vec<Module> = vec![];

        for module_handle in modules {
            let read_len = unsafe {
                GetModuleBaseNameA(
                    self.handle,
                    module_handle,
                    &mut module_name_buf[0],
                    MODULE_NAME_LEN as DWORD,
                )
            };
    
            if read_len == 0 {
                continue;
            }
    
            let cur_mod_name =
                std::str::from_utf8(&realign_unchecked(&module_name_buf)[..read_len as usize])
                    .map_err(|_| Error::new(ErrorKind::Other, "failed to convert string"))?;

            let cur_mod_info = self.get_module_info(module_handle)?;

            valid_modules.push(Module {
                name: cur_mod_name.to_string(),
                base: module_handle as usize,
                size: cur_mod_info.SizeOfImage as usize,
            });
        }

        Ok(valid_modules)
    }

    // pub unsafe fn read<T>(&self, address: usize) -> &T {
    //     &*(self.fix_address(address) as *const T)
    // }

    fn get_module_info(&self, module_handle: HMODULE) -> Result<MODULEINFO, Error> {
        use winapi::um::psapi::GetModuleInformation;

        let mut result = MODULEINFO {
            EntryPoint: std::ptr::null_mut(),
            SizeOfImage: 0,
            lpBaseOfDll: std::ptr::null_mut(),
        };
    
        let success = unsafe {
            GetModuleInformation(
                self.handle,
                module_handle,
                &mut result,
                std::mem::size_of::<MODULEINFO>() as u32,
            )
        };
    
        if success == 0 {
            Err(Error::last_os_error())
        } else {
            Ok(result)
        }
    }

    // fn fix_address(&self, address: usize) -> usize {
    //     self.base_addr + address
    // }
}

impl Drop for GameProcess {
    fn drop(&mut self) {
        self.close_handle().unwrap();
    }
}
