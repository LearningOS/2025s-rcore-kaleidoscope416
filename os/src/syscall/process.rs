//! Process management syscalls
//!
use alloc::sync::Arc;

use crate::{
    fs::{open_file, OpenFlags},
    mm::{translated_refmut, translated_str},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next,
    },
};
use crate::mm::{translated_byte_buffer, MapPermission,mmap,munmap};

use crate::timer::get_time_us;
use core::mem::size_of;
use core::ptr;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    //trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    //trace!("kernel: sys_waitpid");
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
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

pub fn permission(p:usize)->MapPermission{
    let mut out : MapPermission = MapPermission::empty();
    if (p & 0b1) == 0b1 { out =out | MapPermission::R;}
    if (p & 0b10) == 0b10 { out = out | MapPermission::W;}
    if (p & 0b100) == 0b100 { out = out | MapPermission::X;}
    out 
}

/// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    //trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    if _start%4096 != 0 || _port & !0x7 != 0 || _port & 0x7 == 0 {
        return -1;
    }
    let perm = permission(_port);
    mmap(_start,_start+_len,perm)
}

/// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    if _start%4096!=0||_len%4096!=0{
        return -1;
    }
    munmap(_start,_start+_len)
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(_path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let path = translated_str(token, _path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY)  {
        
        let task = current_task().unwrap();
        let all_data = app_inode.read_all();
        let new_task = task.spawn(all_data.as_slice());
        let new_pid = new_task.pid.0;
        // modify trap context of new_task, because it returns immediately after switching
        let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
        // we do not have to move to next instruction since we have done it before
        // for child process, fork returns 0
        trap_cx.x[10] = 0;
        // add new task to scheduler
        add_task(new_task);
        new_pid as isize

    } else {
        -1
    }
}

// YOUR JOB: Set task priority.
pub fn sys_set_priority(_prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    if _prio < 2 {
        return -1;
    }
    current_task()
    .unwrap()
    .inner_exclusive_access()
    .priority = _prio as usize;
    _prio
}
