//! Rust reimplementation of libc++'s std::string implementation

#![allow(dead_code, unused_imports)]
use allocator_api2::alloc::{ Allocator, Global };
use crate::generic::string::CharBehavior;
use std::{
    alloc::Layout,
    marker::PhantomData,
    mem::{ ManuallyDrop, MaybeUninit },
    ptr::NonNull
};
use std::ffi::CStr;

const MAX_STORAGE_SIZE: usize = 0x17;

#[repr(C)]
pub struct LargeString<T = u8, A = Global> // __l
where T: CharBehavior + PartialEq,
      A: Allocator + Clone
{
    // bit 0 is always set - capacity will always be a multiple of 2
    capacity: usize, // __cap_
    size: usize, // __size_
    ptr: NonNull<T>, // __data_
    _allocator: A
}

#[repr(C)]
pub struct SmallString<T = u8, A = Global> // __s
where T: CharBehavior + PartialEq,
A: Allocator + Clone
{
    size: u8, // __size_
    storage: MaybeUninit<[u8; MAX_STORAGE_SIZE]>, // __data_
    _allocator: A,
    _type_marker: PhantomData<T>
}

pub union StringImpl<T = u8, A = Global>
where T: CharBehavior + PartialEq,
      A: Allocator + Clone
{
    large: ManuallyDrop<LargeString<T, A>>,
    small: ManuallyDrop<SmallString<T, A>>,
}

impl<T, A> StringImpl<T, A>
where T: CharBehavior + PartialEq,
      A: Allocator + Clone
{
    fn is_large(&self) -> bool {
        unsafe { self.small.size & 1 != 0 }
    }
    fn get_size(&self) -> usize {
        match self.is_large() {
            true => unsafe { self.large.size },
            false => unsafe { CStr::from_ptr(self.small.storage.as_ptr() as _).count_bytes() },
        }
    }
    fn get_capacity(&self) -> usize {
        match self.is_large() {
            true => unsafe { self.large.capacity },
            false => unsafe { self.small.size as usize },
        }
    }
    fn get_ptr(&self) -> *mut T {
        match self.is_large() {
            true => unsafe { self.large.ptr.as_ptr() },
            false => unsafe { self.small.storage.as_ptr() as _ }
        }
    }
    fn get_layout(&self) -> Layout {
        unsafe { Layout::from_size_align_unchecked(
            size_of::<T>() * self.get_capacity(),
            align_of::<T>()
        ) }
    }
    fn drop_inner(&mut self) {
        if self.is_large() {
            unsafe { self.large._allocator.deallocate(NonNull::new_unchecked(self.get_ptr() as _), self.get_layout()); }
        }
    }
    fn get_allocator(&self) -> A {
        unsafe { self.small._allocator.clone() }
    }
    fn resize(&mut self, new_size: usize) {
        let new_alloc = self.get_allocator().allocate(self.get_layout()).unwrap().as_ptr() as *mut T;
        unsafe { std::ptr::copy_nonoverlapping(self.get_ptr(), new_alloc, self.get_size() + 1) };
        if self.is_large() {
            self.drop_inner();
        }
        unsafe { self.large.ptr = NonNull::new_unchecked(new_alloc) };
        unsafe { self.large.capacity = new_size };
    }
}

impl String<u8, Global> {
    pub fn new() -> Self { Self::new_using(Global) }
    // pub fn from_str(text: &str) -> Self { Self::from_str_in(text, Global) }
}

impl String<u16, Global> {
    pub fn new_wide() -> Self { Self::new_using_wide(Global) }
    // pub fn from_str_wide(text: &str) -> Self { Self::from_str_in_wide(text, Global) }
}

impl<A> String<u8, A>
where A: Allocator + Clone
{
    pub fn new_using(alloc: A) -> Self { Self::new_in(alloc) }
}

impl<A> String<u16, A>
where A: Allocator + Clone
{
    pub fn new_using_wide(alloc: A) -> Self { Self::new_in(alloc) }
}

#[repr(C)]
pub struct String<T = u8, A = Global>
where T: CharBehavior + PartialEq,
      A: Allocator + Clone
{
    _impl: StringImpl<T, A>
}

impl<T, A> String<T, A>
where T: CharBehavior + PartialEq,
      A: Allocator + Clone
{
    pub fn new_in(alloc: A) -> Self {
        assert_eq!(size_of::<A>(), 0, "Allocator must be zero-sized!");
        let mut storage: MaybeUninit<[u8; MAX_STORAGE_SIZE]> = MaybeUninit::uninit();
        unsafe { (*storage.as_mut_ptr())[0] = 0 };
        Self {
            _impl: StringImpl {
                small: ManuallyDrop::new(
                    SmallString {
                        size: ((MAX_STORAGE_SIZE / size_of::<T>()) + 1) as u8,
                        storage,
                        _allocator: alloc,
                        _type_marker: PhantomData::<T>
                    }
                )
            }
        }
    }

    pub fn len(&self) -> usize {
        self._impl.get_size()
    }
    pub fn capacity(&self) -> usize {
        self._impl.get_capacity()
    }
}

impl<A> String<u8, A>
where A: Allocator + Clone
{

}

impl<A> String<u16, A>
where A: Allocator + Clone
{

}

impl<T, A> Drop for String<T, A>
where T: CharBehavior  + PartialEq,
      A: Allocator + Clone
{
    fn drop(&mut self) {
        if self._impl.is_large() {
            self._impl.drop_inner();
        }
    }
}

impl<A> From<&String<u8, A>> for &str
where A: Allocator + Clone
{
    fn from(value: &String<u8, A>) -> Self {
        match value._impl.is_large() {
            true => unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(value._impl.get_ptr(), value._impl.get_size())) },
            false => unsafe { CStr::from_ptr(value._impl.get_ptr() as _).to_str().unwrap() }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use allocator_api2::alloc::Global;
    use std::{
        error::Error,
        string::String as RustString
    };
    use crate::{
        clang::string::String,
        generic::string::CharBehavior,
    };

    type TestReturn = Result<(), Box<dyn Error>>;

    #[test]
    pub fn create_new_blank_string() -> TestReturn {
        let s = String::new();
        let s_str: &str = (&s).into();
        assert_eq!("", s_str, "String should be blank");
        assert_eq!(0, s.len(), "Length of empty string should be zero");
        assert_eq!(24, s.capacity(), "Capacity of empty string should be equal to storage size (excluding null terminator)");
        Ok(())
    }
}