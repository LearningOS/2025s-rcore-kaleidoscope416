//! Memory management implementation
//!
//! SV39 page-based virtual-memory architecture for RV64 systems, and
//! everything about memory management, like frame allocator, page table,
//! map area and memory set, is implemented here.
//!
//! Every task or process has a memory_set to control its virtual memory.
mod address;
pub mod frame_allocator;
mod heap_allocator;
mod memory_set;
pub mod page_table;

pub use address::{PhysAddr, PhysPageNum, VirtAddr, VirtPageNum};
use address::{StepByOne, VPNRange};
pub use frame_allocator::{frame_alloc, FrameTracker,FRAME_ALLOCATOR};
pub use memory_set::remap_test;
pub use memory_set::{MapPermission, MemorySet, KERNEL_SPACE};
pub use page_table::{translated_byte_buffer, translated_refmut, translated_str, PageTableEntry};
use page_table::{PTEFlags, PageTable};
use crate::task::{processor::PROCESSOR,current_user_token};
/// initiate heap allocator, frame allocator and kernel space
pub fn init() {
    heap_allocator::init_heap();
    frame_allocator::init_frame_allocator();
    KERNEL_SPACE.exclusive_access().activate();
}

 /// only insert framed area
 pub fn mmap(start:usize,end:usize,perm:MapPermission)->isize{
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
    
    
    //insert
    let current_pcb = PROCESSOR.exclusive_access().current();
    current_pcb
    .unwrap()
    .inner_exclusive_access()
    .memory_set
    .insert_framed_area(start_va, end_va, perm|MapPermission::U);
    0
}
/// munmap
pub fn munmap(start:usize,end:usize)->isize{
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
    let current_pcb = PROCESSOR.exclusive_access().current();
    current_pcb
    .unwrap()
    .inner_exclusive_access()
    .memory_set
    .remove_framed_area(start_va, end_va)
    
}

