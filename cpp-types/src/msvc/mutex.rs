#![allow(dead_code, unused_imports)]
use std::{
    mem::ManuallyDrop,
    ops::{ Deref, DerefMut }
};
use windows::Win32::{
    Foundation::HANDLE,
    System::Threading::{
        AcquireSRWLockExclusive,
        ReleaseSRWLockExclusive,
        GetCurrentThreadId,
        SleepConditionVariableSRW,
        WakeConditionVariable,
        WakeAllConditionVariable,
        CONDITION_VARIABLE,
        SRWLOCK
    }
};

// mutex types
static MTX_PLAIN: u32 = 1;
static MTX_TRY: u32 = 2;
static MTX_TIMED: u32 = 4;
static MTX_RECURSIVE: u32 = 0x100;
/*
#[repr(C)]
pub union MutexStorage {
    critical_section: ManuallyDrop<CriticalSection>,
    cs_storage: [u8; 0x40]
}
*/

#[repr(C)]
pub struct CriticalSection {
    unused: usize,
    lock: SRWLOCK
}

// MutexGuard, storing references to the mutex that created it and data to access safely between
// threads. This automatically calls unlock when dropped out of scope
pub struct MutexGuard<'a, T> {
    mutex: &'a mut Mutex,
    data: &'a mut T
}

impl<'a, T> MutexGuard<'a, T> {
    // mtx_do_lock
    fn new(mutex: &'a mut Mutex, data: &'a mut T) -> Self {
        let curr_thread = unsafe { GetCurrentThreadId() };
        if (mutex._type & !MTX_RECURSIVE) == MTX_PLAIN { // plain lock
            if mutex.thread_id != curr_thread { // not current thread, acquire lock
                unsafe { AcquireSRWLockExclusive(mutex.get_srw_lock()); }
                mutex.thread_id = curr_thread;
            }
            mutex.count += 1;
        } else { // timed or recursive lock
            // assumed infinite timeout
            if mutex.thread_id != curr_thread {
                unsafe { AcquireSRWLockExclusive(mutex.get_srw_lock()); }
            }
            mutex.count += 1;
            if mutex.count > 1 {
                if (mutex._type & MTX_RECURSIVE) != MTX_RECURSIVE {
                    mutex.count -= 1; // if not recursive
                }
            } else { mutex.thread_id = curr_thread; }
        }
        Self { mutex, data }
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        assert!(self.mutex.count > 0, "Unlock of unowned mutex");
        assert!(self.mutex.thread_id == unsafe { GetCurrentThreadId() },
            "Unlock of mutex not owned by the current thread");
        self.mutex.count -= 1;
        if self.mutex.count == 0 {
            self.mutex.thread_id = u32::MAX;
            unsafe { ReleaseSRWLockExclusive(self.mutex.get_srw_lock()); }
        }
    }
}
impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.data
    }
}
impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

// std::mutex
#[repr(C)]
pub struct Mutex {
    _type: u32,
    // storage: MutexStorage,
    critical_section: CriticalSection,
    cs_storage: [u8; 0x30],
    thread_id: u32,
    count: u32
}

impl Mutex {
    fn get_srw_lock(&mut self) -> *mut SRWLOCK {
        &raw mut self.critical_section.lock
    }
    pub fn lock<'a, T>(&'a mut self, data: &'a mut T) -> MutexGuard<'a, T> {
        MutexGuard::new(self, data)
    }
    pub fn clear_owner(&mut self) {
        self.thread_id = u32::MAX;
        self.count -= 1;
    }
    pub fn reset_owner(&mut self) {
        self.thread_id = unsafe { GetCurrentThreadId() };
        self.count += 1;
    }
    pub fn get_count(&mut self) -> usize {
        self.count as usize
    }
}

unsafe impl Send for Mutex {}
unsafe impl Sync for Mutex {}

// std::condition_variable
#[repr(C)]
pub struct ConditionState {
    unused: usize,
    cond: CONDITION_VARIABLE
}

#[repr(C)]
pub struct ConditionVariable {
    // storage: MutexStorage
    state: ConditionState,
    cs_storage: [u8; 0x38],
}

impl ConditionVariable {
    pub fn signal(&mut self) {
        unsafe { WakeConditionVariable(&raw mut self.state.cond); }
    }
    pub fn broadcast(&mut self) {
        unsafe { WakeAllConditionVariable(&raw mut self.state.cond); }
    }
    // _Cnd_wait
    pub fn wait(&mut self, mutex: &mut Mutex) {
        let mtx_ptr = &raw mut *mutex;
        let mut locked_mtx = mutex.lock(unsafe { &mut *mtx_ptr});
        // _Mtx_clear_owner
        (&mut locked_mtx).clear_owner();
        // _Primitive_wait
        let srw_ptr = &raw mut (&mut locked_mtx).mutex.critical_section.lock;
        let srw_cnd = &raw mut self.state.cond;
        unsafe { SleepConditionVariableSRW(srw_cnd, srw_ptr, u32::MAX, 0).unwrap(); }
        // _Mtx_reset_owner
        (&mut locked_mtx).reset_owner();
    }
}

#[cfg(test)]
pub mod tests {
    use super::Mutex;
    use allocator_api2::alloc::{ Allocator, Global };
    use std::error::Error;
    type TestReturn = Result<(), Box<dyn Error>>;
}
