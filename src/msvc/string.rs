//! Rust reimplementation of Visual C++'s std::string implementation

#![allow(dead_code, unused_imports)]
use allocator_api2::alloc::{ Allocator, Global };
use crate::generic::string::CharBehavior;
use std::{
    alloc::Layout,
    cmp::Ordering,
    fmt::{ Debug, Display },
    hash::{ Hash, Hasher },
    marker::PhantomData,
    mem::size_of,
    ptr::NonNull,
    string::String as RustString
};
use std::mem::MaybeUninit;
// See https://devblogs.microsoft.com/oldnewthing/20230803-00/?p=108532

const MAX_STORAGE_SIZE: usize = 0x10;

#[repr(C)]
pub union StringStorage<T: CharBehavior > {
    buf: MaybeUninit<[u8; MAX_STORAGE_SIZE]>, // _Buf
    ptr: NonNull<T>, // _Ptr
}

impl<T: CharBehavior > StringStorage<T> {
    fn new() -> Self {
        Self {
            buf: MaybeUninit::zeroed()
        }
    }
    fn get_buf(&self) -> *mut T {
        unsafe { self.buf.as_ptr() as *mut _ }
    }
    fn get_ptr(&self) -> *mut T {
        unsafe { self.ptr.as_ptr() }
    }
}

#[repr(C)]
pub struct String<T = u8, A = Global>
where T: CharBehavior  + PartialEq,
      A: Allocator + Clone
{
    storage: StringStorage<T>, // _Bx
    size: usize, // _Mysize
    capacity: usize, // _Myres
    _allocator: A,
    _char_type: PhantomData<T>
}

impl String<u8, Global> {
    pub fn new() -> Self { Self::new_using(Global) }
    pub fn from_str(text: &str) -> Self { Self::from_str_in(text, Global) }
}

impl String<u16, Global> {
    pub fn new_wide() -> Self { Self::new_using_wide(Global) }
    pub fn from_str_wide(text: &str) -> Self { Self::from_str_in_wide(text, Global) }
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

impl<T, A> String<T, A>
where T: CharBehavior  + PartialEq,
      A: Allocator + Clone
{
    pub fn new_in(alloc: A) -> Self {
        assert_eq!(size_of::<A>(), 0, "Allocator must be zero-sized!");
        Self {
            storage: StringStorage::new(),
            size: 0,
            capacity: MAX_STORAGE_SIZE / size_of::<T>() - 1,
            _allocator: alloc,
            _char_type: PhantomData
        }
    }

    fn get_ptr(&self) -> *const T {
        match self.is_inline() {
            true => self.storage.get_buf(),
            false => self.storage.get_ptr()
        }
    }

    fn get_ptr_mut(&mut self) -> *mut T {
        self.get_ptr() as *mut _
    }

    fn get_real_capacity(&self) -> usize {
        self.capacity + 1
    }

    unsafe fn get_layout(&self) -> Layout { 
        Layout::from_size_align_unchecked(
            size_of::<T>() * self.get_real_capacity(),
            align_of::<T>()
        )
    }

    fn is_inline(&self) -> bool { self.capacity < MAX_STORAGE_SIZE / size_of::<T>() }

    fn can_inline(n: usize) -> bool { n < MAX_STORAGE_SIZE / size_of::<T>() }

    fn drop_inner(&mut self) {
        let ptr = unsafe { NonNull::new_unchecked(self.storage.get_ptr() as *mut u8) };
        unsafe { self._allocator.deallocate(ptr, self.get_layout()); }
    }

    fn get_new_capacity(&self) -> usize {
        Self::get_new_capacity_static(self.get_real_capacity())
    }

    fn get_new_capacity_static(value: usize) -> usize {
        match value <= MAX_STORAGE_SIZE {
            true => value * 2,
            false => (value as f32 * 1.5) as usize
        }
    }

    fn resize(&mut self, new: usize) {
        // Get pointer to old allocation
        let old = self.get_ptr();
        let was_inline = self.is_inline();
        self.capacity = if Self::can_inline(new) { MAX_STORAGE_SIZE - 1 } else { new };
        // Point to new allocation and copy old info
        unsafe {
            match self.is_inline() {
                true => if self.size > 0 { std::ptr::copy(old, self.storage.get_buf(), self.size); },
                false => {
                    let new = self._allocator.allocate_zeroed(self.get_layout()).unwrap().as_ptr() as *mut T;
                    if self.size > 0 {
                        std::ptr::copy_nonoverlapping(old, new, self.size);
                        if !was_inline { self.drop_inner() }
                    }
                    self.storage.ptr = NonNull::new_unchecked(new);
                }
            }
        }
    }
    pub fn clear(&mut self) { self.size = 0; }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(self.get_ptr() as *const u8, self.size * size_of::<T>())
        }
    }

    pub fn len(&self) -> usize { self.size }

    pub fn capacity(&self) -> usize { self.capacity }

    fn grow_capacity(&mut self, new_len: usize) {
        if new_len > self.get_real_capacity() { // grow by a factor of 1.5
            let mut new_cap = self.get_new_capacity();
            while new_cap < new_len {
                new_cap = Self::get_new_capacity_static(new_cap);
            }
            self.resize(new_cap - 1);
        }
    }
}

impl<A> String<u8, A>
where A: Allocator + Clone
{
    pub fn from_str_in(text: &str, alloc: A) -> Self {
        let mut new = Self::new_in(alloc);
        // +1 to account for null terminator
        let full_len = text.len() + 1;
        let cap = ((full_len + (MAX_STORAGE_SIZE - 1)) & !(MAX_STORAGE_SIZE - 1)) - 1;
        new.resize(cap);
        // string slice is already UTF-8, so just memcpy it
        unsafe { std::ptr::copy_nonoverlapping(text.as_ptr(), new.get_ptr_mut(), text.len()); }
        new.size = text.len();
        new
    }

    #[deprecated(since = "0.2.0", note = "from_str_in now adds a null terminator to a Rust string if required. Use that instead")]
    pub fn from_str_in_null_term(text: &str, alloc: A) -> Self {
        Self::from_str_in(text, alloc)
    }

    pub fn push_str(&mut self, str: &str) {
        // +1 to account for null terminator
        let full_len = self.len() + str.len() + 1;
        self.grow_capacity(full_len);
        unsafe { std::ptr::copy_nonoverlapping(str.as_ptr(), self.get_ptr_mut().add(self.len()), str.len()); }
        self.size += str.len();
    }
}

impl<A> String<u16, A>
where A: Allocator + Clone
{
    pub fn from_str_in_wide(text: &str, alloc: A) -> Self {
        let mut new = Self::new_in(alloc);
        // +1 to account for null terminator
        let full_len = text.len() + 1;
        let c0 = (MAX_STORAGE_SIZE / size_of::<u16>()) - 1;
        let new_cap = ((full_len + c0) & !c0) - 1;
        new.resize(new_cap);
        let utf16: Vec<u16> = text.encode_utf16().collect(); // convert UTF-8 => UTF-16
        unsafe { std::ptr::copy_nonoverlapping(utf16.as_ptr(), new.get_ptr_mut(), utf16.len()); }
        new.size = text.len();
        new
    }

    pub fn push_str(&mut self, str: &str) {
        // +1 to account for null terminator
        let new_len = self.len() + str.len() + 1;
        self.grow_capacity(new_len);
        let utf16: Vec<u16> = str.encode_utf16().collect(); // convert UTF-8 => UTF-16
        unsafe { std::ptr::copy_nonoverlapping(utf16.as_ptr(), self.get_ptr_mut().add(self.len()), utf16.len()); }
        self.size += str.len();
    }
}

impl<T, A> Drop for String<T, A>
where T: CharBehavior  + PartialEq,
      A: Allocator + Clone
{
    fn drop(&mut self) {
        if !self.is_inline() { self.drop_inner() }
        self.size = 0;
        self.capacity = MAX_STORAGE_SIZE / size_of::<T>();
    }
}

impl<T, A> PartialEq for String<T, A>
where T: CharBehavior  + PartialEq,
      A: Allocator + Clone
{
    fn eq(&self, other: &Self) -> bool {
        if self.size != other.size { return false; }
        let sp = self.get_ptr();
        let op = other.get_ptr();
        for i in 0..self.size {
            unsafe { if *sp.add(i) != *op.add(i) { 
                return false; 
            }}
        }
        true
    }
}

impl<T, A> PartialOrd for String<T, A>
where T: CharBehavior  + PartialEq,
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
            let vp = value.get_ptr();
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
        let vp = value.get_ptr();
        let s = unsafe { std::slice::from_raw_parts(vp, value.size) };
        Self::from(unsafe {std::str::from_utf8_unchecked(s)})
    }
}

impl<A> From<&String<u16, A>> for RustString
where A: Allocator + Clone
{
    fn from(value: &String<u16, A>) -> Self {
        let vp = value.get_ptr();
        let s = unsafe { std::slice::from_raw_parts(vp, value.size) };
        RustString::from_utf16_lossy(s)
    }
}

impl<A> Debug for String<u8, A>
where A: Allocator + Clone
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let as_str: &str = self.into();
        write!(f, "String {{ text: \"{}\", len: {}, cap: {} }}", as_str, self.size, self.capacity)
    }
}

impl<A> Debug for String<u16, A>
where A: Allocator + Clone
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let as_str: RustString = self.into();
        write!(f, "String {{ text: \"{}\", len: {}, cap: {} }}", &as_str, self.size, self.capacity)
    }
}

impl<A> Display for String<u8, A>
where A: Allocator + Clone
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let as_str: &str = self.into();
        write!(f, "\"{}\"", as_str)
    }
}

impl<A> Display for String<u16, A>
where A: Allocator + Clone
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let as_str: RustString = self.into();
        write!(f, "\"{}\"", &as_str)
    }
}

impl<T, A> Hash for String<T, A>
where T: CharBehavior  + PartialEq,
      A: Allocator + Clone
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.as_bytes()) 
    }
}

impl<T, A> Clone for String<T, A>
where T: CharBehavior  + PartialEq,
      A: Allocator + Clone
{
    fn clone(&self) -> Self {
        let storage = if self.is_inline() {
            unsafe { std::ptr::read(&raw const self.storage) }
        } else {
            // make new allocation
            unsafe {
                let mut out = StringStorage::new();
                let new = self._allocator.allocate_zeroed(self.get_layout()).unwrap().as_ptr() as *mut T;
                if self.size > 0 {
                    std::ptr::copy_nonoverlapping(self.get_ptr(), new, self.capacity);
                }
                out.ptr = NonNull::new_unchecked(new);
                out
            } 
        };
        Self {
            storage,
            size: self.size,
            capacity: self.capacity,
            _allocator: self._allocator.clone(),
            _char_type: PhantomData
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
        generic::string::CharBehavior,
        msvc::string::String
    };

    type TestReturn = Result<(), Box<dyn Error>>;

    #[test]
    pub fn create_new_blank_string() -> TestReturn {
        let s = String::new();
        let s_str: &str = (&s).into();
        assert_eq!("", s_str, "String should be blank");
        assert_eq!(0, s.len(), "Length of empty string should be zero");
        assert_eq!(15, s.capacity(), "Capacity of empty string should be equal to storage size (excluding null terminator)");
        Ok(())
    }

    #[test]
    pub fn create_new_long_string() -> TestReturn {
        // 45 characters, including null terminator
        let s = String::from_str("Even if there is some monster behind this...");
        let s_str: &str = (&s).into();
        assert_eq!(s_str, "Even if there is some monster behind this...", "Text doesn't match");
        assert_eq!(s.len(), 44, "Length should be 44");
        assert_eq!(s.capacity(), 47, "Capacity should be equal to allocation size");
        Ok(())
    }

    #[test]
    pub fn create_new_short_string() -> TestReturn {
        // 8 characters, including null terminator
        let s = String::from_str("True...");
        let s_str: &str = (&s).into();
        assert_eq!(s_str, "True...", "Text doesn't match");
        assert_eq!(s.len(), 7, "Length should be 7");
        assert_eq!(s.capacity(), 15, "Capacity should be equal to storage size");
        Ok(())
    }

    #[test]
    pub fn check_string_as_bytes() -> TestReturn {
        let s = String::from_str("True...");
        assert_eq!(s.as_bytes(), [0x54, 0x72, 0x75, 0x65, 0x2E, 0x2E, 0x2E],
        "Byte representation doesn't match");
        Ok(())
    }

    #[test]
    pub fn create_mutable_string() -> TestReturn {
        let mut s = String::from_str("GALLICA!");
        assert_eq!(s.len(), 8, "Length should be 8");
        assert_eq!(s.capacity(), 15, "Capacity should be 15");
        // short push, stays inline
        s.push_str(" THE");
        assert_eq!(s.len(), 12, "Length should be 12");
        assert_eq!(s.capacity(), 15, "Capacity should be 15");
        // large push, move to allocation
        s.push_str(" SOUND OF YOUR WINGS KEEPS ME UP AT NIGHT!");
        assert_eq!(s.len(), 54, "Length should be 54");
        assert_eq!(s.capacity(), 71, "Capacity should be 71");
        assert_eq!(s.as_bytes(), [ 0x47, 0x41, 0x4C, 0x4C, 0x49, 0x43, 0x41, 0x21, 0x20, 0x54, 0x48, 0x45, 0x20, 0x53, 0x4F, 0x55,
0x4E, 0x44, 0x20, 0x4F, 0x46, 0x20, 0x59, 0x4F, 0x55, 0x52, 0x20, 0x57, 0x49, 0x4E, 0x47, 0x53,
0x20, 0x4B, 0x45, 0x45, 0x50, 0x53, 0x20, 0x4D, 0x45, 0x20, 0x55, 0x50, 0x20, 0x41, 0x54, 0x20,
0x4E, 0x49, 0x47, 0x48, 0x54, 0x21], "Bytes don't match");
        s.clear();
        assert_eq!(s.len(), 0, "Length should be zero");
        assert_eq!(s.as_bytes(), [], "Bytes don't match");
        Ok(())
    }

    #[test]
    pub fn create_mutable_string_wide() -> TestReturn {
        let mut s = String::<u16, _>::from_str_wide("GALLICA!");
        assert_eq!(s.len(), 8, "Length should be 8");
        assert_eq!(s.capacity(), 15, "Capacity should be 15");
        // short push, stays inline
        s.push_str(" THE");
        assert_eq!(s.len(), 12, "Length should be 12");
        assert_eq!(s.capacity(), 15, "Capacity should be 15");
        // large push, move to allocation
        s.push_str(" SOUND OF YOUR WINGS KEEPS ME UP AT NIGHT!");
        assert_eq!(s.len(), 54, "Length should be 54");
        assert_eq!(s.capacity(), 71, "Capacity should be 71");
        assert_eq!(s.as_bytes(), [0x47, 0x0, 0x41, 0x0, 0x4c, 0x0, 0x4c, 0x0, 0x49, 0x0, 0x43, 0x0, 0x41, 0x0, 0x21, 0x0, 0x20, 0x0, 0x54,
            0x0, 0x48, 0x0, 0x45, 0x0, 0x20, 0x0, 0x53, 0x0, 0x4f, 0x0, 0x55, 0x0, 0x4e, 0x0, 0x44, 0x0, 0x20, 0x0, 0x4f, 0x0, 0x46, 0x0,
            0x20, 0x0, 0x59, 0x0, 0x4f, 0x0, 0x55, 0x0, 0x52, 0x0, 0x20, 0x0, 0x57, 0x0, 0x49, 0x0, 0x4e, 0x0, 0x47, 0x0, 0x53, 0x0, 0x20,
            0x0, 0x4b, 0x0, 0x45, 0x0, 0x45, 0x0, 0x50, 0x0, 0x53, 0x0, 0x20, 0x0, 0x4d, 0x0, 0x45, 0x0, 0x20, 0x0, 0x55, 0x0, 0x50, 0x0,
            0x20, 0x0, 0x41, 0x0, 0x54, 0x0, 0x20, 0x0, 0x4e, 0x0, 0x49, 0x0, 0x47, 0x0, 0x48, 0x0, 0x54, 0x0, 0x21, 0x0], "Bytes don't match");
        s.clear();
        assert_eq!(s.len(), 0, "Length should be zero");
        assert_eq!(s.as_bytes(), [], "Bytes don't match");
        Ok(())
    }
    #[test]
    pub fn string_compare() -> TestReturn {
        let s0 = String::from_str("True...");
        let s1 = s0.clone();
        assert_eq!(s0, s1, "String s0 (True...) should equal s1 (also True...)");
        let s2 = String::from_str("False!");
        assert_ne!(s0, s2, "String s0 (True...) should not equal s2 (False!)");
        Ok(())
    }
}

#[repr(C)]
pub struct StringView<T = u8, A = Global>
where T: CharBehavior  + PartialEq,
      A: Allocator + Clone
{
    ptr: NonNull<T>,
    size: usize,
    _allocator: A
}

impl StringView<u8, Global> {
    pub fn new() -> Self { Self::new_using(Global) }
    pub fn from_str(text: &str) -> Self { Self::from_str_in(text, Global) }
}

impl StringView<u16, Global> {
    pub fn new_wide() -> Self { Self::new_using_wide(Global) }
    pub fn from_str_wide(text: &str) -> Self { Self::from_str_in_wide(text, Global) }
}

impl<A> StringView<u8, A>
where A: Allocator + Clone
{
    pub fn new_using(alloc: A) -> Self { Self::new_in(alloc) }
}

impl<A> StringView<u16, A>
where A: Allocator + Clone
{
    pub fn new_using_wide(alloc: A) -> Self { Self::new_in(alloc) }
}

impl<T, A> StringView<T, A>
where T: CharBehavior  + PartialEq,
      A: Allocator + Clone
{
    pub fn new_in(alloc: A) -> Self {
        assert_eq!(size_of::<A>(), 0, "Allocator must be zero-sized!");
        Self {
            ptr: NonNull::dangling(),
            size: 0,
            _allocator: alloc,
        }
    }

    fn get_ptr(&self) -> *const T {
        self.ptr.as_ptr()
    }

    fn get_ptr_mut(&mut self) -> *mut T {
        self.get_ptr() as *mut _
    }

    pub fn get_size(&self) -> usize {
        self.size
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.get_ptr() as *const u8, self.size * size_of::<T>()) }
    }

    unsafe fn get_layout(&self) -> Layout {
        Self::get_layout_static(self.get_size())
    }

    unsafe fn get_layout_static(size: usize) -> Layout {
        Layout::from_size_align_unchecked(
            size_of::<T>() * size,
            align_of::<T>()
        )
    }
}

impl<A> StringView<u8, A>
where A: Allocator + Clone
{
    pub fn from_str_in(text: &str, alloc: A) -> Self {
        let mut new = Self::new_in(alloc);
        // +1 to account for null terminator
        let new_size = text.len() + 1;
        new.ptr = unsafe { new._allocator.allocate_zeroed(Self::get_layout_static(new_size)).unwrap().cast() };
        new.size = text.len();
        // string slice is already UTF-8, so just memcpy it
        unsafe { std::ptr::copy_nonoverlapping(text.as_ptr(), new.get_ptr_mut(), text.len()); }
        new
    }
}

impl<A> StringView<u16, A>
where A: Allocator + Clone
{
    pub fn from_str_in_wide(text: &str, alloc: A) -> Self {
        let mut new = Self::new_in(alloc);
        // +1 to account for null terminator
        let new_size = text.len() + 1;
        new.ptr = unsafe { new._allocator.allocate_zeroed(Self::get_layout_static(new_size)).unwrap().cast() };
        new.size = text.len();
        let utf16: Vec<u16> = text.encode_utf16().collect(); // convert UTF-8 => UTF-16
        unsafe { std::ptr::copy_nonoverlapping(utf16.as_ptr(), new.get_ptr_mut(), utf16.len()); }
        new
    }
}

impl<T, A> Drop for StringView<T, A>
where T: CharBehavior  + PartialEq,
      A: Allocator + Clone
{
    fn drop(&mut self) {
        if self.size > 0 {
            let ptr = unsafe { NonNull::new_unchecked(self.get_ptr() as *mut u8) };
            unsafe { self._allocator.deallocate(ptr, self.get_layout()); }
        }
    }
}

impl<T, A> PartialEq for StringView<T, A>
where T: CharBehavior + PartialEq,
      A: Allocator + Clone
{
    fn eq(&self, other: &Self) -> bool {
        if self.size != other.size { return false; }
        let sp = self.get_ptr();
        let op = other.get_ptr();
        for i in 0..self.size {
            unsafe { if *sp.add(i) != *op.add(i) {
                return false;
            }}
        }
        true
    }
}

impl<A> From<&StringView<u8, A>> for &str
where A: Allocator + Clone
{
    fn from(value: &StringView<u8, A>) -> Self {
        if value.size > 0 {
            let vp = value.get_ptr();
            let s = unsafe { std::slice::from_raw_parts(vp, value.size) };
            unsafe { std::str::from_utf8_unchecked(s) }
        } else {
            ""
        }
    }
}

impl<A> From<&StringView<u8, A>> for RustString
where A: Allocator + Clone
{
    fn from(value: &StringView<u8, A>) -> Self {
        let vp = value.get_ptr();
        let s = unsafe { std::slice::from_raw_parts(vp, value.size) };
        Self::from(unsafe {std::str::from_utf8_unchecked(s)})
    }
}

impl<A> From<&StringView<u16, A>> for RustString
where A: Allocator + Clone
{
    fn from(value: &StringView<u16, A>) -> Self {
        let vp = value.get_ptr();
        let s = unsafe { std::slice::from_raw_parts(vp, value.size) };
        RustString::from_utf16_lossy(s)
    }
}

impl<A> Debug for StringView<u8, A>
where A: Allocator + Clone
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let as_str: &str = self.into();
        write!(f, "StringView {{ text: \"{}\", len: {} }}", as_str, self.size)
    }
}

impl<A> Debug for StringView<u16, A>
where A: Allocator + Clone
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let as_str: RustString = self.into();
        write!(f, "StringView {{ text: \"{}\", len: {} }}", &as_str, self.size)
    }
}

impl<A> Display for StringView<u8, A>
where A: Allocator + Clone
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let as_str: &str = self.into();
        write!(f, "\"{}\"", as_str)
    }
}

impl<A> Display for StringView<u16, A>
where A: Allocator + Clone
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let as_str: RustString = self.into();
        write!(f, "\"{}\"", &as_str)
    }
}

impl<T, A> Hash for StringView<T, A>
where T: CharBehavior  + PartialEq,
      A: Allocator + Clone
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.as_bytes())
    }
}