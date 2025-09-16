#![allow(dead_code, unused_imports)]
use allocator_api2::alloc::{ Allocator, Global };
use crate::generic::string::CharBehavior;
use std::{
    mem::MaybeUninit,
    ptr::NonNull
};
use std::{
    alloc::Layout,
    marker::PhantomData
};

const MAX_STORAGE_SIZE: usize = 0x10;

#[repr(C)]
pub union StringStorage<T: CharBehavior> {
    buf: MaybeUninit<[u8; MAX_STORAGE_SIZE]>, // _M_local_buf
    capacity: usize, // _M_allocated_capacity
    _char_type: PhantomData<T>
}

impl<T: CharBehavior> StringStorage<T> {
    fn new() -> Self {
        Self {
            buf: MaybeUninit::uninit()
        }
    }
    fn get_buf(&self) -> *mut T {
        unsafe { self.buf.as_ptr() as *mut _ }
    }
}

#[repr(C)]
pub struct String<T = u8, A = Global>
where T: CharBehavior + PartialEq,
      A: Allocator + Clone
{
    ptr: NonNull<T>, // _M_dataplus
    size: usize, // _M_string_length
    storage: StringStorage<T>,
    _allocator: A,
}

impl String<u8, Global> {
    pub fn new() -> Self { Self::new_using(Global) }
}

impl<A> String<u8, A>
where A: Allocator + Clone
{
    pub fn new_using(alloc: A) -> Self { Self::new_in(alloc) }
}

impl<T, A> String<T, A>
where T: CharBehavior + PartialEq,
      A: Allocator + Clone
{
    pub fn new_in(alloc: A) -> Self {
        assert_eq!(size_of::<A>(), 0, "Allocator must be zero-sized!");
        Self {
            // this should point to self.storage if inlined
            // however we can't do this without heap pinning, but new()
            // needs to return a stack allocated String instance
            // Call setup_pointers() after this to fix it
            ptr: NonNull::dangling(),
            size: 0,
            storage: unsafe { MaybeUninit::uninit().assume_init() },
            _allocator: alloc,
        }
    }

    pub unsafe fn setup_pointers(&mut self) {
        if self.size < MAX_STORAGE_SIZE / size_of::<T>() {
            self.ptr = unsafe { NonNull::new_unchecked(self.storage.buf.as_ptr() as _) };
        }
    }

    pub fn get_ptr(&self) -> *const T {
        match self.is_inline() {
            true => self.storage.get_buf() as _,
            false => self.ptr.as_ptr()
        }
    }

    pub fn get_ptr_mut(&mut self) -> *mut T {
        self.get_ptr() as *mut _
    }

    fn is_inline(&self) -> bool {
        self.ptr.as_ptr() == self.storage.get_buf()
    }

    fn can_inline(n: usize) -> bool {
        n <= MAX_STORAGE_SIZE / size_of::<T>()
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn capacity(&self) -> usize {
        match self.is_inline() {
            true => MAX_STORAGE_SIZE / size_of::<T>(),
            false => unsafe { *(self.storage.get_buf() as *const usize) }
        }
    }

    unsafe fn get_layout(&self) -> Layout {
        Layout::from_size_align_unchecked(
            size_of::<T>() * self.capacity(),
            align_of::<T>()
        )
    }

    fn drop_inner(&mut self) {
        if self.ptr.as_ptr() != self.storage.get_buf() as _ {
            unsafe { self._allocator.deallocate(std::mem::transmute(self.ptr.as_ptr()), self.get_layout()) }
        }
    }

    pub fn clear(&mut self) {
        self.size = 0;
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.get_ptr() as *const u8, self.size * size_of::<T>()) }
    }
}

#[cfg(test)]
pub mod tests {
    use std::error::Error;
    use crate::{
        generic::string::CharBehavior,
        gcc::string::String
    };

    type TestReturn = Result<(), Box<dyn Error>>;

    #[test]
    pub fn create_new_blank_string() -> TestReturn {
        let mut s = String::new();
        unsafe { s.setup_pointers() };
        println!("0x{:x}, 0x{:x}", &raw const s as usize, s.ptr.as_ptr() as usize);
        Ok(())
    }
}