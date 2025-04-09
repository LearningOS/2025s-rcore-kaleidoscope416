//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the operating system.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.

mod context;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::loader::{get_app_data, get_num_app};
use crate::mm::{ MapPermission, PageTable, VirtAddr, VirtPageNum};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::vec::Vec;
use lazy_static::*;

use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};
pub use context::TaskContext;
use crate::mm::frame_allocator::FRAME_ALLOCATOR;
/// The task manager, where all the tasks are managed.
///
/// Functions implemented on `TaskManager` deals with all task state transitions
/// and task context switching. For convenience, you can find wrappers around it
/// in the module level.
///
/// Most of `TaskManager` are hidden behind the field `inner`, to defer
/// borrowing checks to runtime. You can see examples on how to use `inner` in
/// existing functions on `TaskManager`.
pub struct TaskManager {
    /// total number of tasks
    num_app: usize,
    /// use inner value to get mutable access
    inner: UPSafeCell<TaskManagerInner>,
}

/// The task manager inner in 'UPSafeCell'
struct TaskManagerInner {
    /// task list
    tasks: Vec<TaskControlBlock>,
    /// id of current `Running` task
    current_task: usize,
}

lazy_static! {
    /// a `TaskManager` global instance through lazy_static!
    pub static ref TASK_MANAGER: TaskManager = {
        println!("init TASK_MANAGER");
        let num_app = get_num_app();
        println!("num_app = {}", num_app);
        let mut tasks: Vec<TaskControlBlock> = Vec::new();
        for i in 0..num_app {
            tasks.push(TaskControlBlock::new(get_app_data(i), i));
        }
        TaskManager {
            num_app,
            inner: unsafe {
                UPSafeCell::new(TaskManagerInner {
                    tasks,
                    current_task: 0,
                })
            },
        }
    };
}

impl TaskManager {
    /// Run the first task in task list.
    ///
    /// Generally, the first task in task list is an idle task (we call it zero process later).
    /// But in ch4, we load apps statically, so the first task is a real app.
    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let next_task = &mut inner.tasks[0];
        next_task.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &next_task.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        // before this, we should drop local variables that must be dropped manually
        unsafe {
            __switch(&mut _unused as *mut _, next_task_cx_ptr);
        }
        panic!("unreachable in run_first_task!");
    }

    /// Change the status of current `Running` task into `Ready`.
    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].task_status = TaskStatus::Ready;
    }

    /// Change the status of current `Running` task into `Exited`.
    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].task_status = TaskStatus::Exited;
    }

    /// Find next task to run and return task id.
    ///
    /// In this case, we only return the first `Ready` task in task list.
    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }

    /// Get the current 'Running' task's token.
    fn get_current_token(&self) -> usize {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_user_token()
    }

    /// Get the current 'Running' task's trap contexts.
    fn get_current_trap_cx(&self) -> &'static mut TrapContext {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_trap_cx()
    }

    /// Change the current 'Running' task's program break
    pub fn change_current_program_brk(&self, size: i32) -> Option<usize> {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].change_program_brk(size)
    }

    /// Switch current `Running` task to the task we have found,
    /// or there is no `Ready` task and we can exit with all applications completed
    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            drop(inner);
            // before this, we should drop local variables that must be dropped manually
            unsafe {
                __switch(current_task_cx_ptr, next_task_cx_ptr);
            }
            // go back to user mode
        } else {
            panic!("All applications completed!");
        }
    }
    ///213
    pub fn syscall_plus (&self,_id:usize){
        let current_index = self.inner.exclusive_access().current_task;
        match _id {
            64=> self.inner.exclusive_access().tasks[current_index].task_syscall_count[0]+=1,
            93 => self.inner.exclusive_access().tasks[current_index].task_syscall_count[1]+=1,
            124 => self.inner.exclusive_access().tasks[current_index].task_syscall_count[2]+=1,
            169 => self.inner.exclusive_access().tasks[current_index].task_syscall_count[3]+=1,
            214 => self.inner.exclusive_access().tasks[current_index].task_syscall_count[4]+=1,
            215 => self.inner.exclusive_access().tasks[current_index].task_syscall_count[5]+=1,
            222 => self.inner.exclusive_access().tasks[current_index].task_syscall_count[6]+=1,
            410 => self.inner.exclusive_access().tasks[current_index].task_syscall_count[7]+=1,
            _=> panic!("Unsupported syscall_id: {}", _id),
        }
    }
    ///234
    pub fn syscall_count (&self,_id:usize) -> isize{
        let current_index = self.inner.exclusive_access().current_task;
        match _id {
            64=> self.inner.exclusive_access().tasks[current_index].task_syscall_count[0],
            93 => self.inner.exclusive_access().tasks[current_index].task_syscall_count[1],
            124 => self.inner.exclusive_access().tasks[current_index].task_syscall_count[2],
            169 => self.inner.exclusive_access().tasks[current_index].task_syscall_count[3],
            214 => self.inner.exclusive_access().tasks[current_index].task_syscall_count[4],
            215 => self.inner.exclusive_access().tasks[current_index].task_syscall_count[5],
            222 => self.inner.exclusive_access().tasks[current_index].task_syscall_count[6],
            410 => self.inner.exclusive_access().tasks[current_index].task_syscall_count[7],
            _=> panic!("Unsupported syscall_id: {}", _id),
        }
    }

    /// only insert framed area
    pub fn mmap(&self,start:usize,end:usize,perm:MapPermission)->isize{
        //检查物理frame number
        if (end-start+4095)/4096 > FRAME_ALLOCATOR.exclusive_access().number(){
            
            return -1
            
        }
        //检查页号分配
        let svpn = VirtAddr::from(start).floor();
        let evpn =VirtAddr::from(end).ceil();
        for i in svpn.0..evpn.0{
            let vpn = VirtPageNum::from(i);
            let page_table = PageTable::from_token(current_user_token());
            let out = page_table.find_pte(vpn);
            if out.is_some()&&out.unwrap().is_valid() {
                    println!("sadfasdfasdfasdfasdf");
                    return -1;
                }
            
        }
        let start_va = VirtAddr::from(start);
        let end_va = VirtAddr::from(end);
        let current_index = self.inner.exclusive_access().current_task; 
        //insert
        self.inner.exclusive_access()
        .tasks[current_index]
        .memory_set
        .insert_framed_area(start_va,end_va,perm|MapPermission::U);
        return 0;
    }
    /// munmap
    pub fn munmap(&self,start:usize,end:usize)->isize{
        let svpn = VirtAddr::from(start).floor();
        let evpn =VirtAddr::from(end).ceil();
        for i in svpn.0..evpn.0{
            let vpn = VirtPageNum::from(i);
            let page_table = PageTable::from_token(current_user_token());
            let out = page_table.find_pte(vpn);
            if out.is_none() { 
                return -1
            } else {
                if !out.unwrap().is_valid(){return -1;}   
            }
          
        }
        //remove 
        let start_va = VirtAddr::from(start);
        let end_va = VirtAddr::from(end);
        let current_index = self.inner.exclusive_access().current_task; 
        self.inner.exclusive_access()
        .tasks[current_index]
        .memory_set 
        .remove_framed_area(start_va, end_va)
        
    }
}

/// Run the first task in task list.
pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

/// Switch current `Running` task to the task we have found,
/// or there is no `Ready` task and we can exit with all applications completed
fn run_next_task() {
    TASK_MANAGER.run_next_task();
}

/// Change the status of current `Running` task into `Ready`.
fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}

/// Change the status of current `Running` task into `Exited`.
fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}

/// Suspend the current 'Running' task and run the next task in task list.
pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    run_next_task();
}

/// Exit the current 'Running' task and run the next task in task list.
pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}

/// Get the current 'Running' task's token.
pub fn current_user_token() -> usize {
    TASK_MANAGER.get_current_token()
}

/// Get the current 'Running' task's trap contexts.
pub fn current_trap_cx() -> &'static mut TrapContext {
    TASK_MANAGER.get_current_trap_cx()
}

/// Change the current 'Running' task's program break
pub fn change_program_brk(size: i32) -> Option<usize> {
    TASK_MANAGER.change_current_program_brk(size)
}
