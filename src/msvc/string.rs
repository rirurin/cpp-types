#![allow(dead_code, unused_imports)]
use allocator_api2::alloc::{ Allocator, Global };
use std::{
    alloc::Layout,
    fmt::{ Debug, Display },
    hash::{ Hash, Hasher },
    mem::size_of,
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

impl String<u8, Global> {
    pub fn new() -> Self { Self::new_using(Global) }
    pub fn from_str(text: &str) -> Self { Self::from_str_in(text, Global) }
}

impl String<u16, Global> {
    pub fn new_wide() -> Self { Self::new_using_wide(Global) }
    pub fn from_str_wide(text: &str) -> Self { Self::from_str_in(text, Global) }
}

impl<A> String<u8, A>
where A: Allocator
{
    pub fn new_using(alloc: A) -> Self { Self::new_in(alloc) }
}

impl<A> String<u16, A>
where A: Allocator
{
    pub fn new_using_wide(alloc: A) -> Self { Self::new_in(alloc) }
}

impl<T, A> String<T, A>
where T: CharBehavior + PartialEq,
      A: Allocator
{
    pub fn new_in(alloc: A) -> Self {
        assert!(std::mem::size_of::<A>() == 0, "Allocator must be zero-sized!");
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
    fn can_inline(n: usize) -> bool { n <= MAX_STORAGE_SIZE / std::mem::size_of::<T>() }
    fn drop_inner(&mut self) { 
        let ptr = unsafe { NonNull::new_unchecked(self.get_large_ptr_mut() as *mut u8) };
        unsafe { self._allocator.deallocate(ptr, self.get_layout()); }
    }
    fn resize(&mut self, new: usize) {
        // Get pointer to old allocation
        let old = self.get_ptr();
        let was_inline = self.is_inline();
        let to_copy = if new > self.capacity { self.capacity } else { new };
        self.capacity = if Self::can_inline(new) { MAX_STORAGE_SIZE } else { new };
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
    pub fn clear(&mut self) { self.size = 0; }
    pub fn as_bytes(&self) -> &[u8] { unsafe { std::slice::from_raw_parts(self.get_ptr() as *const u8, self.size * std::mem::size_of::<T>()) } }
    pub fn len(&self) -> usize { self.size }
    pub fn capacity(&self) -> usize { self.capacity }
}

impl<A> String<u8, A>
where A: Allocator
{
    pub fn from_str_in(text: &str, alloc: A) -> Self {
        let mut new = Self::new_in(alloc);
        new.resize(text.len());
        // string slice is already UTF-8, so just memcpy it
        unsafe { std::ptr::copy_nonoverlapping(text.as_ptr(), new.get_ptr_mut(), text.len()); }
        new.size = text.len();
        new
    }

    pub fn push_str(&mut self, str: &str) {
        if self.len() + str.len() > self.capacity() { // round to nearest power of 2
            self.resize(1 << usize::BITS - (self.len() + str.len()).leading_zeros());
        }
        unsafe { std::ptr::copy_nonoverlapping(str.as_ptr(), self.get_ptr_mut().add(self.len()), str.len()); }
        self.size += str.len();
    }
}

impl<A> String<u16, A>
where A: Allocator
{
    pub fn from_str_in(text: &str, alloc: A) -> Self {
        let mut new = Self::new_in(alloc);
        new.resize(text.len());
        let utf16: Vec<u16> = text.encode_utf16().collect(); // convert UTF-8 => UTF-16
        unsafe { std::ptr::copy_nonoverlapping(utf16.as_ptr(), new.get_ptr_mut(), utf16.len()); }
        new.size = text.len();
        new
    }
    pub fn push_str(&mut self, str: &str) {
        if self.len() + str.len() > self.capacity() { // round to nearest power of 2
            self.resize(1 << usize::BITS - (self.len() + str.len()).leading_zeros());
        }
        let utf16: Vec<u16> = str.encode_utf16().collect(); // convert UTF-8 => UTF-16
        unsafe { std::ptr::copy_nonoverlapping(utf16.as_ptr(), self.get_ptr_mut(), utf16.len()); }
        self.size += str.len();
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
        if value.size > 0 {
            let vp = value.get_ptr();
            let s = unsafe { std::slice::from_raw_parts(vp, value.size) };
            unsafe { std::str::from_utf8_unchecked(s) }
        } else {
            ""
        }
    }
}

impl<A> From<&String<u8, A>> for RustString
where A: Allocator
{
    fn from(value: &String<u8, A>) -> Self {
        let vp = value.get_ptr();
        let s = unsafe { std::slice::from_raw_parts(vp, value.size) };
        Self::from(unsafe {std::str::from_utf8_unchecked(s)})
    }
}

impl<A> From<&String<u16, A>> for RustString
where A: Allocator
{
    fn from(value: &String<u16, A>) -> Self {
        let vp = value.get_ptr();
        let s = unsafe { std::slice::from_raw_parts(vp, value.size) };
        RustString::from_utf16_lossy(s)
    }
}

impl<A> Debug for String<u8, A>
where A: Allocator
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let as_str: &str = self.into();
        write!(f, "String {{ text: \"{}\", len: {}, cap: {} }}", as_str, self.size, self.capacity)
    }
}

impl<A> Debug for String<u16, A>
where A: Allocator
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let as_str: RustString = self.into();
        write!(f, "String {{ text: \"{}\", len: {}, cap: {} }}", &as_str, self.size, self.capacity)
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

impl<T, A> Hash for String<T, A>
where T: CharBehavior + PartialEq,
      A: Allocator
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.as_bytes()) 
    }
}

#[cfg(test)]
pub mod tests {
    use super::String;
    use std::string::String as RustString;
    use std::error::Error;

    type TestReturn = Result<(), Box<dyn Error>>;

    #[test]
    pub fn create_new_blank_string() -> TestReturn {
        let s = String::new();
        let s_str: &str = (&s).into();
        assert!(s_str == "", "String should be blank");
        assert!(s.len() == 0, "Length should be zero");
        assert!(s.capacity() == 16, "Capacity should be equal to storage size");
        Ok(())
    }

    #[test]
    pub fn create_new_long_string() -> TestReturn {
        // 45 characters, including null terminator
        let s = String::from_str("Even if there is some monster behind this...");
        let s_str: &str = (&s).into();
        println!("c: {}, l: {}", s.capacity(), s.len());
        assert!(s_str == "Even if there is some monster behind this...", "Text doesn't match");
        assert!(s.len() == 44, "Length should be 44");
        assert!(s.capacity() == 44, "Capacity should be equal to allocation size");
        Ok(())
    }

    #[test]
    pub fn create_new_short_string() -> TestReturn {
        // 8 characters, including null terminator
        let s = String::from_str("True...");
        let s_str: &str = (&s).into();
        assert!(s_str == "True...", "Text doesn't match");
        assert!(s.len() == 7, "Length should be 7");
        assert!(s.capacity() == 16, "Capacity should be equal to storage size");
        Ok(())
    }

    #[test]
    pub fn check_string_as_bytes() -> TestReturn {
        let s = String::from_str("True...");
        assert!(s.as_bytes() == [0x54, 0x72, 0x75, 0x65, 0x2E, 0x2E, 0x2E],
        "Byte representation doesn't match");
        Ok(())
    }

    #[test]
    pub fn create_mutable_string() -> TestReturn {
        let mut s = String::from_str("GALLICA!");
        assert!(s.len() == 8, "Length should be 8");
        // short push, stays inline
        s.push_str(" THE");
        assert!(s.len() == 12, "Length should be 12");
        // large push, move to allocation
        s.push_str(" SOUND OF YOUR WINGS KEEPS ME UP AT NIGHT!");
        assert!(s.len() == 54, "Length should be 54");
        assert!(s.as_bytes() == [ 0x47, 0x41, 0x4C, 0x4C, 0x49, 0x43, 0x41, 0x21, 0x20, 0x54, 0x48, 0x45, 0x20, 0x53, 0x4F, 0x55,
0x4E, 0x44, 0x20, 0x4F, 0x46, 0x20, 0x59, 0x4F, 0x55, 0x52, 0x20, 0x57, 0x49, 0x4E, 0x47, 0x53,
0x20, 0x4B, 0x45, 0x45, 0x50, 0x53, 0x20, 0x4D, 0x45, 0x20, 0x55, 0x50, 0x20, 0x41, 0x54, 0x20,
0x4E, 0x49, 0x47, 0x48, 0x54, 0x21], "Bytes don't match");
        s.clear();
        assert!(s.len() == 0, "Length should be zero");
        assert!(s.as_bytes() == [], "Bytes don't match");
        Ok(())
    }
}
