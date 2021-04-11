use super::utils::*;

use std::io::{Error, ErrorKind};
use winapi::shared::minwindef::{DWORD, HMODULE};
use winapi::um::{
    winnt::HANDLE,
    psapi::{GetModuleInformation, MODULEINFO, EnumProcessModules, GetModuleBaseNameA},
};

#[derive(Debug)]
pub struct Module {
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

    pub fn find_in_process(process_handle: HANDLE, dll_name: &str) -> Result<Module, Error> {
        let mut size_needed: DWORD = 0;

        let result = unsafe { EnumProcessModules(process_handle, std::ptr::null_mut(), 0, &mut size_needed) };
    
        if result == 0 {
            return Err(Error::last_os_error());
        }
    
        let handle_size = std::mem::size_of::<HMODULE>() as u32;
        let module_count = size_needed / handle_size;
        let mut modules: Vec<HMODULE> = vec![std::ptr::null_mut(); module_count as usize];
        let result = unsafe {
            EnumProcessModules(
                process_handle,
                modules.as_mut_ptr(),
                module_count * handle_size,
                &mut size_needed,
            )
        };
    
        if result == 0 {
            return Err(Error::new(ErrorKind::Other, "failed to enumerate process modules"));
        }
    
        const MODULE_NAME_LEN: usize = 50;
        let mut module_name_buf: [i8; MODULE_NAME_LEN] = [0; MODULE_NAME_LEN];
    
        for module_handle in modules {
            let read_len = unsafe {
                GetModuleBaseNameA(
                    process_handle,
                    module_handle,
                    &mut module_name_buf[0],
                    MODULE_NAME_LEN as DWORD,
                )
            };
    
            if read_len == 0 {
                continue;
            }
    
            let cur_mod_name = std::str::from_utf8(&realign_unchecked(&module_name_buf)[..read_len as usize])
                .map_err(|_| Error::new(ErrorKind::Other, "failed to convert string"))?;
    
            let cur_mod_info = Self::get_module_info(process_handle, module_handle)?;
    
            if cur_mod_name == dll_name {
                return Ok(Module {
                    base: module_handle as usize,
                    size: cur_mod_info.SizeOfImage as usize,
                });
            }
        }
    
        return Err(Error::new(ErrorKind::Other, "failed to find module"));
    }
    
    fn get_module_info(process_handle: HANDLE, module_handle: HMODULE) -> Result<MODULEINFO, Error> {
        let mut result = MODULEINFO {
            EntryPoint: std::ptr::null_mut(),
            SizeOfImage: 0,
            lpBaseOfDll: std::ptr::null_mut(),
        };
    
        let success = unsafe {
            GetModuleInformation(
                process_handle,
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
}
