#![allow(dead_code, unused_imports)]
use allocator_api2::alloc::{ Allocator, Global };
use std::{
    alloc::Layout,
    fmt::{ Debug, Display },
    ptr::NonNull,
    string::String as RustString
};

// See https://devblogs.microsoft.com/oldnewthing/20230803-00/?p=108532

const MAX_STORAGE_SIZE: usize = 0x10;

pub trait CharBehavior { }
impl CharBehavior for u8 { } // std::string
impl CharBehavior for u16 { } // std::wstring

#[repr(C)]
pub struct String<T = u8, A = Global>
where T: CharBehavior + PartialEq,
      A: Allocator
{
    storage: [u8; MAX_STORAGE_SIZE],
    size: usize,
    capacity: usize,
    _allocator: A,
    _char_type: std::marker::PhantomData<T>
}

impl<T, A> String<T, A>
where T: CharBehavior + PartialEq,
      A: Allocator
{
    pub fn new(alloc: A) -> Self {
        assert!(std::mem::size_of::<T>() == 0, "Allocator must be zero-sized!");
        Self {
            storage: [0; MAX_STORAGE_SIZE],
            size: 0,
            capacity: MAX_STORAGE_SIZE / std::mem::size_of::<T>(),
            _allocator: alloc,
            _char_type: std::marker::PhantomData
        }
    }
    fn get_ptr(&self) -> *const T {
        if self.is_inline() { &raw const self.storage as *const T } else { unsafe { *(&raw const self.storage as *const *const T) } }
    }
    fn get_ptr_mut(&mut self) -> *mut T {
        if self.is_inline() { &raw mut self.storage as *mut T } else { unsafe { *(&raw mut self.storage as *mut *mut T) } }
    }
    fn get_small_ptr(&self) -> *const T { &raw const self.storage as *const T }
    fn get_small_ptr_mut(&mut self) -> *mut T { &raw mut self.storage as *mut T }
    unsafe fn get_large_ptr(&self) -> *const T { *(&raw const self.storage as *const *const T) }
    unsafe fn get_large_ptr_mut(&mut self) -> *mut T { *(&raw mut self.storage as *mut *mut T) }
    unsafe fn get_large_ptr_ptr_mut(&mut self) -> *mut *mut T { &raw mut self.storage as *mut *mut T }
    unsafe fn get_layout(&self) -> Layout { 
        Layout::from_size_align_unchecked(
            std::mem::size_of::<T>() * self.capacity, 
            std::mem::align_of::<T>()
        )
    }
    fn is_inline(&self) -> bool { self.capacity <= MAX_STORAGE_SIZE / std::mem::size_of::<T>() }
    fn can_inline(&self, n: usize) -> bool { n <= MAX_STORAGE_SIZE / std::mem::size_of::<T>() }
    fn drop_inner(&mut self) {
        let ptr = unsafe { NonNull::new_unchecked((&raw mut self.storage) as *mut u8) };
        unsafe { self._allocator.deallocate(ptr, self.get_layout()); }
    }
    fn resize(&mut self, new: usize) {
        // Get pointer to old allocation
        let old = self.get_ptr();
        let was_inline = self.is_inline();
        let to_copy = if new > self.capacity { self.capacity } else { new };
        self.capacity = new;
        // Point to new allocation and copy old info
        unsafe {
            if self.is_inline() {
                if self.size > 0 { std::ptr::copy(old, self.get_small_ptr_mut(), to_copy); }
            } else {
                let new = self._allocator.allocate(self.get_layout()).unwrap().as_ptr() as *mut T;
                if self.size > 0 {
                    std::ptr::copy_nonoverlapping(old, new, to_copy);
                    if !was_inline { self.drop_inner() }
                }
                std::ptr::write(self.get_large_ptr_ptr_mut(), new);
            }
        }
    }
    fn clear(&mut self) { self.size = 0; }
    fn as_bytes(&self) -> &[u8] { unsafe { std::slice::from_raw_parts(self.get_ptr() as *const u8, self.size * std::mem::size_of::<T>()) } }
}

impl<A> String<u8, A>
where A: Allocator
{
    fn from_str(text: &str, alloc: A) -> Self {
        let mut new = Self::new(alloc);
        new.resize(text.len() + 1);
        // string slice is already UTF-8, so just memcpy it
        unsafe { std::ptr::copy_nonoverlapping(text.as_ptr(), new.get_ptr_mut(), text.len()); }
        new.size = text.len();
        // add null terminator
        unsafe { *new.get_ptr_mut().add(new.size) = 0; }
        new.size += 1;
        new
    }
}

impl<A> String<u16, A>
where A: Allocator
{
    fn from_str(text: &str, alloc: A) -> Self {
        let mut new = Self::new(alloc);
        new.resize(text.len() + 1);
        let utf16: Vec<u16> = text.encode_utf16().collect(); // convert UTF-8 => UTF-16
        unsafe { std::ptr::copy_nonoverlapping(utf16.as_ptr(), new.get_ptr_mut(), utf16.len()); }
        new.size = text.len();
        // add null terminator
        unsafe { *new.get_ptr_mut().add(new.size) = 0; }
        new.size += 1;
        new
    }
}

impl<T, A> Drop for String<T, A>
where T: CharBehavior + PartialEq,
      A: Allocator
{
    fn drop(&mut self) {
        if !self.is_inline() { self.drop_inner() }
    }
}

impl<T, A> PartialEq for String<T, A>
where T: CharBehavior + PartialEq,
      A: Allocator
{
    fn eq(&self, other: &Self) -> bool {
        if self.size != other.size { return false; }
        let sp = self.get_ptr();
        let op = other.get_ptr();
        for i in 0..self.size {
            unsafe { if *sp.add(i) == *op.add(i) { 
                return false; 
            }}
        }
        true
    }
}
impl<A> From<&String<u8, A>> for &str
where A: Allocator
{
    fn from(value: &String<u8, A>) -> Self {
        let vp = value.get_ptr();
        // subtract one to remove null terminator
        let s = unsafe { std::slice::from_raw_parts(vp, value.size - 1) };
        unsafe { std::str::from_utf8_unchecked(s) }
    }
}

impl<A> From<&String<u8, A>> for RustString
where A: Allocator
{
    fn from(value: &String<u8, A>) -> Self {
        let vp = value.get_ptr();
        // subtract one to remove null terminator
        let s = unsafe { std::slice::from_raw_parts(vp, value.size - 1) };
        Self::from(unsafe {std::str::from_utf8_unchecked(s)})
    }
}

impl<A> From<&String<u16, A>> for RustString
where A: Allocator
{
    fn from(value: &String<u16, A>) -> Self {
        let vp = value.get_ptr();
        // subtract one to remove null terminator
        let s = unsafe { std::slice::from_raw_parts(vp, value.size - 1) };
        RustString::from_utf16_lossy(s)
    }
}

impl<A> Debug for String<u8, A>
where A: Allocator
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let as_str: &str = self.into();
        write!(f, "String {{ text: \"{}\", len: {}, cap: {}}}", as_str, self.size, self.capacity)
    }
}

impl<A> Debug for String<u16, A>
where A: Allocator
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let as_str: RustString = self.into();
        write!(f, "String {{ text: \"{}\", len: {}, cap: {}}}", &as_str, self.size, self.capacity)
    }
}

impl<A> Display for String<u8, A>
where A: Allocator
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let as_str: &str = self.into();
        write!(f, "\"{}\"", as_str)
    }
}

impl<A> Display for String<u16, A>
where A: Allocator
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let as_str: RustString = self.into();
        write!(f, "\"{}\"", &as_str)
    }
}

#[cfg(test)]
pub mod tests {

}
