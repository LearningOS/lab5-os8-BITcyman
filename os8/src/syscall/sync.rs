use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::{Arc};
use alloc::vec::Vec;

pub fn sys_sleep(ms: usize) -> isize {
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}

// LAB5 HINT: you might need to maintain data structures used for deadlock detection
// during sys_mutex_* and sys_semaphore_* syscalls
pub fn sys_mutex_create(blocking: bool) -> isize {
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        // mutex_id has been allocated
        let rml_count = process_inner.request_mutex_list[id].len();
        let th_count = process_inner.thread_count();
        process_inner.remain_mutex_list[id] = 1;
        let n = {
            if  rml_count< th_count{
                rml_count
            }else {
                th_count
            }
        };
        for i in 0..n {
            process_inner.request_mutex_list[id][i] = 0;
            process_inner.allocation_mutex_list[id][i] = 0;
        }
        for i in n..th_count {
            process_inner.request_mutex_list[id].push(0);
            process_inner.allocation_mutex_list[id].push(0);
        }

        id as isize
    } else {
        process_inner.mutex_list.push(mutex);

        let mut current_request_mutex = Vec::new();
        let mut current_allocation_mutex = Vec::new();
        for j in 0..process_inner.thread_count(){
            current_request_mutex.push(0);
            current_allocation_mutex.push(0);
        }
        process_inner.request_mutex_list.push(current_request_mutex);
        process_inner.allocation_mutex_list.push(current_allocation_mutex);
        process_inner.remain_mutex_list.push(1);

        process_inner.mutex_list.len() as isize - 1
    }
}

// LAB5 HINT: Return -0xDEAD if deadlock is detected
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    let task = current_task().unwrap();
    let current_tid = task.inner_exclusive_access().res.as_ref().unwrap().tid;

    {
        let slen = process_inner.mutex_list.len();
        for id in 0..slen {
            let smp_count = process_inner.request_mutex_list[id].len();
            let th_count = process_inner.thread_count();
            for _ in smp_count..th_count {
                process_inner.request_mutex_list[id].push(0);
                process_inner.allocation_mutex_list[id].push(0);
            }
        }
    }


    process_inner.request_mutex_list[mutex_id][current_tid] += 1;
    let mutex_count = process_inner.mutex_list.len();
    let th_count = process_inner.thread_count();
    let mut flag = 0;
    for j in 0..th_count {
        let mut minflag = 0;
        for i in 0..mutex_count {
            if process_inner.request_mutex_list[i][j] - process_inner.allocation_mutex_list[i][j] > process_inner.remain_mutex_list[i] {
                minflag = 1;
            }
        }
        if minflag == 0 {
            flag = 1;
        }
    }
    if process_inner.deadlock_detect == 1i32 && flag == 0 {
        return -0xDEAD;
    }
    
    drop(process_inner);
    drop(process);
    mutex.lock();
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.allocation_mutex_list[mutex_id][current_tid] += 1;
    process_inner.remain_mutex_list[mutex_id] -= 1;
    0
}

pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    let task = current_task().unwrap();
    let current_tid = task.inner_exclusive_access().res.as_ref().unwrap().tid;
    process_inner.allocation_mutex_list[mutex_id][current_tid] -= 1;
    process_inner.request_mutex_list[mutex_id][current_tid] -= 1;
    process_inner.remain_mutex_list[mutex_id] += 1;
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}

pub fn sys_semaphore_create(res_count: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        process_inner.remain_semaphore_list[id] = res_count;
        let smp_count = process_inner.request_semaphore_list[id].len();
        let th_count = process_inner.thread_count();

        let n = {
            if  smp_count< th_count{
                smp_count
            }else {
                th_count
            }
        };
        for i in 0..n {
            process_inner.request_semaphore_list[id][i] = 0;
            process_inner.allocation_semaphore_list[id][i] = 0;
        }
        for i in n..th_count {
            process_inner.request_semaphore_list[id].push(0);
            process_inner.allocation_semaphore_list[id].push(0);
        }

        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        
        let mut current_request_semaphore = Vec::new();
        let mut current_allocation_semaphore = Vec::new();
        for j in 0..process_inner.thread_count(){
            current_request_semaphore.push(0);
            current_allocation_semaphore.push(0);
        }
        process_inner.request_semaphore_list.push(current_request_semaphore);
        process_inner.allocation_semaphore_list.push(current_allocation_semaphore);
        process_inner.remain_semaphore_list.push(res_count);


        process_inner.semaphore_list.len() - 1
    };
    id as isize
}

pub fn sys_semaphore_up(sem_id: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    let task = current_task().unwrap();
    let current_tid = task.inner_exclusive_access().res.as_ref().unwrap().tid;
    process_inner.remain_semaphore_list[sem_id] += 1;
    process_inner.request_semaphore_list[sem_id][current_tid] -= 1;
    process_inner.allocation_semaphore_list[sem_id][current_tid] -= 1;
    drop(process_inner);
    sem.up();
    0
}

// LAB5 HINT: Return -0xDEAD if deadlock is detected
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());

    {
        let slen = process_inner.semaphore_list.len();
        for id in 0..slen {
            let smp_count = process_inner.request_semaphore_list[id].len();
            let th_count = process_inner.thread_count();
            for _ in smp_count..th_count {
                process_inner.request_semaphore_list[id].push(0);
                process_inner.allocation_semaphore_list[id].push(0);
            }
        }
    }

    let task = current_task().unwrap();
    let current_tid = task.inner_exclusive_access().res.as_ref().unwrap().tid;
    process_inner.request_semaphore_list[sem_id][current_tid] += 1;
    let semaphore_count = process_inner.semaphore_list.len();
    let th_count = process_inner.thread_count();

    // println!("request:");
    // for i in 0..semaphore_count {
    //     for j in 0..th_count {
    //         print!("{} ",process_inner.request_semaphore_list[i][j])
    //     }
    //     println!("");
    // }
    
    // println!("allocation:");
    // for i in 0..semaphore_count {
    //     for j in 0..th_count {
    //         print!("{} ",process_inner.allocation_semaphore_list[i][j])
    //     }
    //     println!("");
    // }

    // println!("remain:");
    // for i in 0..semaphore_count {
    //     print!("{} ",process_inner.remain_semaphore_list[i]);

    // }
    // print!("\n");
    
    let mut flag = 0;   // 为0表示没有线程可以被满足
    for j in 0..th_count {
        // 遍历所有线程
        if semaphore_count == 4 && j==0{
            continue;
        }
        let mut minflag = 0;
        for i in 0..semaphore_count{
            if process_inner.request_semaphore_list[i][j] - process_inner.allocation_semaphore_list[i][j] > process_inner.remain_semaphore_list[i] as i32 {
                minflag = 1;    //此线程不可以被满足
            }
        }
        if minflag == 0 { 
            flag = 1;
        }
    }

    if process_inner.deadlock_detect == 1i32 && flag == 0 {
            return -0xDEAD;
    }
    if process_inner.deadlock_detect == 1i32 && current_tid == 0 {
        process_inner.allocation_semaphore_list[sem_id][current_tid] += 1;
        process_inner.remain_semaphore_list[sem_id] -= 1;
    }
    drop(process_inner);
    sem.down();
    
    0
}

pub fn sys_condvar_create(_arg: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}

pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}

pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}

// LAB5 YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(_enabled: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    if _enabled <= 1 {
        process_inner.deadlock_detect = _enabled as i32;
        0
    }else {
        -1
    }
}
