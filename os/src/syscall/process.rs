//! Process management syscalls
use crate::mm::{translated_byte_buffer, PageTable};
use crate::task::{change_program_brk, current_user_token, exit_current_and_run_next, suspend_current_and_run_next};
use crate::timer::get_time_us;
use core::marker::StructuralPartialEq;
use core::{ptr,mem::size_of};
use crate::task::TASK_MANAGER;
use crate::mm::VirtAddr;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let token = current_user_token();
    let mut buffers = translated_byte_buffer(token, _ts as *const u8, size_of::<TimeVal>());
    let time_val = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    let _bytes = unsafe {
        let mut bytes = [0u8; size_of::<TimeVal>()];
        ptr::copy_nonoverlapping(&time_val as *const TimeVal as *const u8, bytes.as_mut_ptr(), bytes.len());
        bytes
    };
    if buffers.len() == 1 {
        buffers[0].copy_from_slice(&_bytes);
    }
    else if buffers.len() == 2 {
        let len = buffers[0].len();
        buffers[0].copy_from_slice(&_bytes[0..len]);
        buffers[1].copy_from_slice(&_bytes[len..]);
    }
    0
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(_trace_request: usize, _id: usize, _data: usize) -> isize {
    trace!("kernel: sys_trace");
    match _trace_request {
        0=>{
            let token = current_user_token();
            let start_va = VirtAddr::from(_id);
            let page_table = PageTable::from_token(token);
            let pte = page_table.translate(start_va.floor());
            if pte.is_none() || !pte.unwrap().readable() {
                return -1;
            }
            else {
                return pte.unwrap()
                .ppn()
                .get_bytes_array()[start_va.page_offset()] as isize;
            }
        }
        1=>{
            let token = current_user_token();
            let start_va = VirtAddr::from(_id);
            let page_table = PageTable::from_token(token);
            let pte = page_table.translate(start_va.floor());
            if pte.is_none() || !pte.unwrap().writable() {
                return -1;
            }
            else {
                let data = (_data & 0xFF) as u8;
                let bytes = unsafe {
                    let mut bytes = [0u8; size_of::<u8>()];
                    ptr::copy_nonoverlapping(&data as *const u8 as *const u8, bytes.as_mut_ptr(), bytes.len());
                    bytes
                };
                let mut buffers = translated_byte_buffer(token, _id as *const u8, size_of::<u8>());
                buffers[0].copy_from_slice(&bytes);
                return 0;
            }
        }
        2=>{
            TASK_MANAGER.syscall_count(_id)
        }
        _=>-1
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    if _start%4096 != 0 || prot & !0x7 != 0 || prot & 0x7 = 0 {
        return -1;
    }
    let start_va = VirtAddr::from(_start);
    let end_va = VirtAddr::from(_start+_len);
    let token = current_user_token();
    for i in start_va .. end_va {
        let page_table = PageTable::from_token(token);
        let out = page_table.translate(i.floor());sdf
    }
    
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    -1
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
