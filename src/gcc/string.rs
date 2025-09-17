//! Rust reimplementation of libstdc++'s std::string implementation

#![allow(dead_code, unused_imports)]
use allocator_api2::{
    alloc::{ Allocator, Global },
    boxed::Box as ABox
};
use crate::generic::string::CharBehavior;
use std::{
    alloc::Layout,
    cmp::Ordering,
    hash::{Hash, Hasher},
    marker::PhantomData,
    mem::MaybeUninit,
    ptr::NonNull,
    string::String as RustString
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
    fn get_capacity(&self) -> usize {
        unsafe { self.capacity }
    }
    fn set_capacity(&mut self, new: usize) {
        self.capacity = new
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
    pub unsafe fn new() -> Self { Self::new_using(Global) }
    pub fn new_standalone() -> ABox<Self, Global> { Self::new_standalone_using(Global) }
    pub unsafe fn from_str(text: &str) -> Self { Self::from_str_in(text, Global) }
    pub fn from_str_standalone(text: &str) -> ABox<Self, Global> { Self::from_str_in_standalone(text, Global) }
}

impl String<u16, Global> {
    pub unsafe fn new_wide() -> Self { Self::new_using_wide(Global) }
    pub fn new_standalone_wide() -> ABox<Self, Global> { Self::new_standalong_using_wide(Global) }
}

impl<A> String<u8, A>
where A: Allocator + Clone
{
    pub unsafe fn new_using(alloc: A) -> Self { Self::new_in(alloc) }
    pub fn new_standalone_using(alloc: A) -> ABox<Self, A> { Self::new_standalone_in(alloc) }
}

impl<A> String<u16, A>
where A: Allocator + Clone
{
    pub unsafe fn new_using_wide(alloc: A) -> Self { Self::new_in(alloc) }
    pub fn new_standalong_using_wide(alloc: A) -> ABox<Self, A> { Self::new_standalone_in(alloc) }
}

impl<T, A> String<T, A>
where T: CharBehavior + PartialEq,
      A: Allocator + Clone
{
    pub unsafe fn new_in(alloc: A) -> Self {
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

    pub fn new_standalone_in(alloc: A) -> ABox<Self, A> {
        let mut new = ABox::new_in(unsafe { Self::new_in(alloc.clone()) }, alloc);
        unsafe { new.setup_pointers() };
        new
    }

    pub unsafe fn setup_pointers(&mut self) {
        if self.size < MAX_STORAGE_SIZE / size_of::<T>() {
            self.ptr = unsafe { NonNull::new_unchecked(self.storage.buf.as_ptr() as _) };
        }
    }

    fn is_inline(&self) -> bool {
        match self.ptr.as_ptr() != NonNull::<T>::dangling().as_ptr() {
            true => self.ptr.as_ptr() == self.storage.get_buf(),
            false => true
        }
    }

    fn can_inline(n: usize) -> bool {
        n < MAX_STORAGE_SIZE / size_of::<T>()
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn capacity(&self) -> usize {
        match self.is_inline() {
            true => (MAX_STORAGE_SIZE / size_of::<T>()) - 1,
            false => self.storage.get_capacity()
        }
    }

    fn get_real_capacity(&self) -> usize {
        self.capacity() + 1
    }

    unsafe fn get_layout(&self) -> Layout {
        Self::get_layout_static(self.capacity())
    }

    unsafe fn get_layout_static(capacity: usize) -> Layout {
        Layout::from_size_align_unchecked(
            size_of::<T>() * capacity + 1,
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
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr() as *const u8, self.size * size_of::<T>()) }
    }

    fn resize(&mut self, new: usize) {
        let old = self.ptr.as_ptr();
        let was_inline = self.is_inline();
        if !Self::can_inline(new) {
            self.storage.set_capacity(new);
        }
        // Point to new allocation and copy old info
        unsafe {
            match Self::can_inline(new) {
                true => if self.size > 0 {
                    std::ptr::copy(old, self.storage.get_buf(), self.size + 1);
                    self.setup_pointers();
                },
                false => {
                    let new = self._allocator.allocate(Self::get_layout_static(new)).unwrap().as_ptr() as *mut T;
                    if self.size > 0 {
                        std::ptr::copy_nonoverlapping(old, new, self.size + 1);
                        if !was_inline { self.drop_inner() }
                    }
                    self.ptr = NonNull::new_unchecked(new);
                }
            }
        }
    }

    fn grow_capacity(&mut self, new_len: usize) {
        if new_len > self.get_real_capacity() { // grow by a factor of 1.5
            let mut new_cap = self.get_real_capacity() * 2;
            while new_cap < new_len {
                new_cap *= 2;
            }
            self.resize(new_cap - 1);
        }
    }
}

impl<A> String<u8, A>
where A: Allocator + Clone
{
    pub unsafe fn from_str_in(text: &str, alloc: A) -> Self {
        let mut new = Self::new_in(alloc);
        let has_null_term = text.as_bytes().last().map_or(false, |c| *c == 0);
        let new_size = text.len() + (1 * Into::<usize>::into(has_null_term));
        let new_cap = ((new_size + (MAX_STORAGE_SIZE - 1)) & !(MAX_STORAGE_SIZE - 1)) - 1;
        new.resize(new_cap);
        // string slice is already UTF-8, so just memcpy it
        let copy_to = match new.is_inline() {
            true => new.storage.get_buf(), // pointer is still set as dangling if inline
            false => new.ptr.as_ptr()
        };
        std::ptr::copy_nonoverlapping(text.as_ptr(), copy_to, text.len());
        // add the null terminator if needed
        new.size = new_size;
        if !has_null_term {
            *copy_to.add(new_size) = 0;
        }
        new
    }

    pub fn from_str_in_standalone(text: &str, alloc: A) -> ABox<Self, A> {
        let mut new = unsafe { ABox::new_in(Self::from_str_in(text, alloc.clone()), alloc) };
        unsafe { new.setup_pointers() };
        new
    }

    pub fn push_str(&mut self, str: &str) {
        // remove null terminator from string to push
        let str = str.strip_suffix("\0").unwrap_or(str);
        let has_null_term = self.as_bytes().last().map_or(false, |c| *c == 0);
        let new_len = self.len() + str.len() + (1 * Into::<usize>::into(has_null_term));
        self.grow_capacity(new_len);
        unsafe { std::ptr::copy_nonoverlapping(str.as_ptr(), self.ptr.as_ptr().add(self.len()), str.len()); }
        if !has_null_term {
            unsafe { *self.ptr.as_ptr().add(new_len) = 0 };
        }
        self.size += str.len();
    }
}

impl<A> String<u16, A>
where A: Allocator + Clone
{
}

impl<T, A> Drop for String<T, A>
where T: CharBehavior + PartialEq,
      A: Allocator + Clone
{
    fn drop(&mut self) {
        if !self.is_inline() { self.drop_inner() }
        self.size = 0;
    }
}

impl<T, A> PartialEq for String<T, A>
where T: CharBehavior + PartialEq,
      A: Allocator + Clone
{
    fn eq(&self, other: &Self) -> bool {
        if self.size != other.size { return false; }
        let sp = self.ptr.as_ptr();
        let op = other.ptr.as_ptr();
        for i in 0..self.size {
            unsafe { if *sp.add(i) != *op.add(i) {
                return false;
            }}
        }
        true
    }
}

impl<T, A> PartialOrd for String<T, A>
where T: CharBehavior + PartialOrd,
      A: Allocator + Clone
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.as_bytes().partial_cmp(other.as_bytes())
    }
}

impl<A> From<&String<u8, A>> for &str
where A: Allocator + Clone
{
    fn from(value: &String<u8, A>) -> Self {
        if value.size > 0 {
            let vp = value.ptr.as_ptr();
            let s = unsafe { std::slice::from_raw_parts(vp, value.size) };
            unsafe { std::str::from_utf8_unchecked(s) }
        } else {
            ""
        }
    }
}

impl<A> From<&String<u8, A>> for RustString
where A: Allocator + Clone
{
    fn from(value: &String<u8, A>) -> Self {
        let vp = value.ptr.as_ptr();
        let s = unsafe { std::slice::from_raw_parts(vp, value.size) };
        Self::from(unsafe {std::str::from_utf8_unchecked(s)})
    }
}

impl<A> From<&String<u16, A>> for RustString
where A: Allocator + Clone
{
    fn from(value: &String<u16, A>) -> Self {
        let vp = value.ptr.as_ptr();
        let s = unsafe { std::slice::from_raw_parts(vp, value.size) };
        RustString::from_utf16_lossy(s)
    }
}

impl<T, A> Hash for String<T, A>
where T: CharBehavior + PartialEq,
      A: Allocator + Clone
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.as_bytes())
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
        let s = String::new_standalone();
        let s_str: &str = s.as_ref().into();
        assert_eq!("", s_str, "String should be blank");
        assert_eq!(0, s.len(), "Length of empty string should be zero");
        assert_eq!(15, s.capacity(), "Capacity of empty string should be equal to storage size (excluding null terminator)");
        Ok(())
    }

    #[test]
    pub fn create_new_long_string() -> TestReturn {
        // 45 characters, including null terminator
        let mut s = unsafe { String::from_str("Even if there is some monster behind this...") };
        unsafe { s.setup_pointers() };
        let s_str: &str = (&s).into();
        assert_eq!(s_str, "Even if there is some monster behind this...", "Text doesn't match");
        assert_eq!(s.len(), 44, "Length should be 44");
        assert_eq!(s.capacity(), 47, "Capacity should be equal to allocation size");
        Ok(())
    }

    #[test]
    pub fn create_new_short_string() -> TestReturn {
        // 8 characters, including null terminator
        let mut s = unsafe { String::from_str("True...") };
        unsafe { s.setup_pointers() };
        let s_str: &str = (&s).into();
        assert_eq!(s_str, "True...", "Text doesn't match");
        assert_eq!(s.len(), 7, "Length should be 7");
        assert_eq!(s.capacity(), 15, "Capacity should be equal to storage size");
        Ok(())
    }

    /*
    #[test]
    pub fn create_mutable_string() -> TestReturn {
        let mut s = unsafe { String::from_str("GALLICA!") };
        unsafe { s.setup_pointers() };
        assert_eq!(s.len(), 8, "Length should be 8");
        assert_eq!(s.capacity(), 15, "Capacity should be 15");
        // short push, stays inline
        s.push_str(" THE");
        assert_eq!(s.len(), 12, "Length should be 12");
        assert_eq!(s.capacity(), 15, "Capacity should be 15");
        // large push, move to allocation
        s.push_str(" SOUND OF YOUR WINGS KEEPS ME UP AT NIGHT!");
        assert_eq!(s.len(), 54, "Length should be 54");
        // assert_eq!(s.capacity(), 71, "Capacity should be 71");
        assert_eq!(s.as_bytes(), [ 0x47, 0x41, 0x4C, 0x4C, 0x49, 0x43, 0x41, 0x21, 0x20, 0x54, 0x48, 0x45, 0x20, 0x53, 0x4F, 0x55,
            0x4E, 0x44, 0x20, 0x4F, 0x46, 0x20, 0x59, 0x4F, 0x55, 0x52, 0x20, 0x57, 0x49, 0x4E, 0x47, 0x53,
            0x20, 0x4B, 0x45, 0x45, 0x50, 0x53, 0x20, 0x4D, 0x45, 0x20, 0x55, 0x50, 0x20, 0x41, 0x54, 0x20,
            0x4E, 0x49, 0x47, 0x48, 0x54, 0x21], "Bytes don't match");
        s.clear();
        assert_eq!(s.len(), 0, "Length should be zero");
        assert_eq!(s.as_bytes(), [], "Bytes don't match");
        Ok(())
    }
    */

    #[test]
    pub fn check_string_as_bytes() -> TestReturn {
        let s = String::from_str_standalone("True...");
        assert_eq!(s.as_bytes(), [0x54, 0x72, 0x75, 0x65, 0x2E, 0x2E, 0x2E],
                   "Byte representation doesn't match");
        Ok(())
    }
}