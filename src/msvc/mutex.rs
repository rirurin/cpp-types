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
    fn new(mutex: &'a mut Mutex, data: &'a mut T) -> Self {
        let curr_thread = unsafe { GetCurrentThreadId() };
        if (mutex._type & MTX_PLAIN) == MTX_PLAIN { // plain lock
            if mutex.thread_id != curr_thread { // not current thread, acquire lock
                unsafe { AcquireSRWLockExclusive(mutex.get_srw_lock()); }
                mutex.thread_id = curr_thread;
            }
            mutex.count += 1;
        } else { // timed or recursive lock
            if mutex.thread_id != curr_thread {
                unsafe { AcquireSRWLockExclusive(mutex.get_srw_lock()); }
            }
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
}

unsafe impl Send for Mutex {}
unsafe impl Sync for Mutex {}

// std::condition_variable
#[repr(C)]
pub struct ConditionVariable {
    // storage: MutexStorage
    critical_section: CriticalSection,
    cs_storage: [u8; 0x30],
}
