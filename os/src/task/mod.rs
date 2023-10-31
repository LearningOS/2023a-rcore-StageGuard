//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the whole operating system.
//!
//! A single global instance of [`Processor`] called `PROCESSOR` monitors running
//! task(s) for each core.
//!
//! A single global instance of `PID_ALLOCATOR` allocates pid for user apps.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.
mod context;
mod id;
mod manager;
mod processor;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::loader::get_app_data_by_name;
use alloc::sync::Arc;
use lazy_static::*;
pub use manager::{fetch_task, TaskManager};
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};

pub use context::TaskContext;
pub use id::{kstack_alloc, pid_alloc, KernelStack, PidHandle};
pub use manager::add_task;
pub use processor::{
    current_task, current_trap_cx, current_user_token, run_tasks, schedule, take_current_task,
    Processor,
};
use crate::config::{MAX_SYSCALL_NUM, PAGE_SIZE};
use crate::mm::MapPermission;

/// Suspend the current 'Running' task and run the next task in task list.
pub fn suspend_current_and_run_next() {
    // There must be an application running.
    let task = take_current_task().unwrap();

    // ---- access current TCB exclusively
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    // Change status to Ready
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);
    // ---- release current PCB

    // push back to ready queue.
    add_task(task);
    // jump to scheduling cycle
    schedule(task_cx_ptr);
}

/// pid of usertests app in make run TEST=1
pub const IDLE_PID: usize = 0;

/// Exit the current 'Running' task and run the next task in task list.
pub fn exit_current_and_run_next(exit_code: i32) {
    // take from Processor
    let task = take_current_task().unwrap();

    let pid = task.getpid();
    if pid == IDLE_PID {
        println!(
            "[kernel] Idle process exit with exit_code {} ...",
            exit_code
        );
        panic!("All applications completed!");
    }

    // **** access current TCB exclusively
    let mut inner = task.inner_exclusive_access();
    // Change status to Zombie
    inner.task_status = TaskStatus::Zombie;
    // Record exit code
    inner.exit_code = exit_code;
    // do not move to its parent but under initproc

    // ++++++ access initproc TCB exclusively
    {
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        for child in inner.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }
    }
    // ++++++ release parent PCB

    inner.children.clear();
    // deallocate user space
    inner.memory_set.recycle_data_pages();
    drop(inner);
    // **** release current PCB
    // drop task manually to maintain rc correctly
    drop(task);
    // we do not have to save task context
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

/// Apply for a new virtual memory area to current task
pub fn mmap_memory_area_for_current(start_va: usize, len: usize, prot: usize) -> bool {
    // start is not aligned to PAGE_SIZE
    if start_va % PAGE_SIZE != 0 { return false }

    let mut perm = MapPermission::U;

    if prot & !0x7 != 0 { return false }
    if prot & 0x7 == 0 { return false }

    if prot & 0b1 == 1 { perm |= MapPermission::R; }
    if prot & 0b10 == 2 { perm |= MapPermission::W; }
    if prot & 0b100 == 4 { perm |= MapPermission::X; }

    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let mem_set = &mut inner.memory_set;

    let start = (start_va / PAGE_SIZE) * PAGE_SIZE;
    let mut end = (start_va + len) / PAGE_SIZE * PAGE_SIZE;
    if (start + len) % PAGE_SIZE > 0 { end += PAGE_SIZE }

    if mem_set.is_conflict(start..end) { return false }

    mem_set.insert_framed_area(start.into(), end.into(), perm);
    true
}

/// Apply for a new virtual memory area to current task
pub fn munmap_memory_area_for_current(start_va: usize, len: usize) -> bool {
    // start is not aligned to PAGE_SIZE
    if start_va % PAGE_SIZE != 0 { return false }

    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    let mem_set = &mut inner.memory_set;

    let start = (start_va / PAGE_SIZE) * PAGE_SIZE;
    let mut end = (start_va + len) / PAGE_SIZE * PAGE_SIZE;
    if (start + len) % PAGE_SIZE > 0 { end += PAGE_SIZE }

    mem_set.recycle_map_area(start.into(), end.into())
}

/// Increment syscall stat to current running task
pub fn increment_syscall_stat(syscall_id: usize) {
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();

    if inner.task_status != TaskStatus::Running { return }
    inner.syscall_statistics[syscall_id] += 1;
}
/// Get syscall stat of current running task
pub fn get_current_task_syscall_stat() -> Option<(TaskStatus, [u32; MAX_SYSCALL_NUM])> {
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();

    Some((inner.task_status.clone(),
          inner.syscall_statistics.clone()))
}

lazy_static! {
    /// Creation of initial process
    ///
    /// the name "initproc" may be changed to any other app name like "usertests",
    /// but we have user_shell, so we don't need to change it.
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new(TaskControlBlock::new(
        get_app_data_by_name("ch5b_initproc").unwrap()
    ));
}

///Add init process to the manager
pub fn add_initproc() {
    add_task(INITPROC.clone());
}
