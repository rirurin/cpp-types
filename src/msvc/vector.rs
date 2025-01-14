#![allow(dead_code, unused_imports)]
use allocator_api2::alloc::{ Allocator, Global };
use std::{
    alloc::Layout,
    fmt::Display,
    iter::IntoIterator,
    mem::ManuallyDrop,
    ops::{ Index, IndexMut },
    ptr::NonNull,
    slice::{ Iter, IterMut }
};

static START_ALLOC_SIZE: usize = 0x8;

#[repr(C)]
#[derive(Debug)]
pub struct Vector<T, A = Global>
where A: Allocator
{
    first: *mut T,
    last: *mut T,
    end: *mut T,
    _allocator: A
}

impl<T> Vector<T, Global> {
    pub fn new() -> Self { Self::new_in(Global) }
    pub fn from_vec(vec: Vec<T>) -> Self { Self::from_vec_in(vec, Global) }
}

impl<T, A> Vector<T, A>
where A: Allocator
{
    pub fn new_in(alloc: A) -> Self {
        assert!(std::mem::size_of::<A>() == 0, "Allocator must be zero-sized!");
        Self {
            first: std::ptr::null_mut(),
            last: std::ptr::null_mut(),
            end: std::ptr::null_mut(),
            _allocator: alloc
        }
    }
    unsafe fn get_layout(len: usize) -> Layout {
        Layout::from_size_align_unchecked(
            std::mem::size_of::<T>() * len, 
            std::mem::align_of::<T>()
        )
    }
    unsafe fn get_nonnull(&self) -> NonNull<u8> {
        NonNull::new_unchecked(self.first as *mut u8)
    }
    pub fn len(&self) -> usize {
        (self.last as usize - self.first as usize) / std::mem::size_of::<T>()
    }
    pub fn cap(&self) -> usize {
        (self.end as usize - self.first as usize) / std::mem::size_of::<T>()
    }
    pub fn is_empty(&self) -> bool { self.len() == 0 }
    pub fn resize(&mut self, new: usize) {
        unsafe {
            let alloc = self._allocator.allocate(Self::get_layout(new)).unwrap().as_ptr() as *mut T;
            // if old exists, copy pointer
            if !self.first.is_null() {
                std::ptr::copy_nonoverlapping(self.first, alloc, self.cap());
                self._allocator.deallocate(self.get_nonnull(), Self::get_layout(self.cap()));
                let old_len = self.len();
                self.first = alloc;
                self.last = alloc.add(old_len);
            } else {
                self.first = alloc;
                self.last = alloc;
            } 
            self.end = alloc.add(new);
        }
    }
    pub fn push(&mut self, val: T) {
        if self.len() == self.cap() {
            self.resize(if self.len() == 0 { START_ALLOC_SIZE } else { self.cap() * 2 });
        }
        unsafe { 
            std::ptr::write(self.first.add(self.len()), val); 
            self.last = self.last.add(1);
        }
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.len() > 0 {
            let val = unsafe { self.last.sub(1) };
            self.last = unsafe { self.last.sub(1) };
            Some(unsafe { std::ptr::read(val)})
        } else {
            None
        }
    }

    pub fn as_slice(&self) -> &[T] {
        unsafe {std::slice::from_raw_parts(
            self.first, self.len() 
        )}
    }

    pub fn as_slice_mut(&mut self) -> &mut [T] {
        unsafe {std::slice::from_raw_parts_mut(
            self.first, self.len() 
        )}
    }

    pub fn as_ptr(&self) -> *const T { self.first }
    pub fn as_mut_ptr(&mut self) -> *mut T { self.first }

    pub fn iter(&self) -> Iter<'_, T> {
        self.as_slice().iter()
    }
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        self.as_slice_mut().iter_mut()
    }

    pub fn from_vec_in(vec: Vec<T>, alloc: A) -> Self {
        assert!(std::mem::size_of::<A>() == 0, "Allocator must be zero-sized!");
        let mut new = Vector::new_in(alloc);
        let new_size = 1 << (usize::BITS - vec.len().leading_zeros());
        let new_size = if new_size < START_ALLOC_SIZE { START_ALLOC_SIZE } else { new_size };
        let alloc = unsafe { new._allocator.allocate(Self::get_layout(new_size)).unwrap().as_ptr() as *mut T };
        new.first = alloc;
        unsafe { std::ptr::copy_nonoverlapping(vec.as_ptr(), new.first, vec.len()); }
        unsafe {
            new.last = alloc.add(vec.len());
            new.end = alloc.add(new_size);
        }
        new
    }

    pub fn index_of_by_predicate<F>(&self, cb: F) -> Option<usize>
    where F: Fn(&T) -> bool
    {
        for (i, v) in self.iter().enumerate() {
            if cb(v) { return Some(i)}
        }
        None       
    }

    pub fn contains_by_predicate<F>(&self, cb: F) -> bool
    where F: Fn(&T) -> bool { self.find_by_predicate(cb).is_some() }

    pub fn find_by_predicate<F>(&self, cb: F) -> Option<&T>
    where F: Fn(&T) -> bool
    {
        for v in self {
            if cb(v) { return Some(v)}
        }
        None
    }
}

impl<T, A> Vector<T, A>
where T: PartialEq,
      A: Allocator
{
    pub fn index_of(&self, val: T) -> Option<usize> {
        for (i, v) in self.iter().enumerate() {
            if *v == val { return Some(i)}
        }
        None
    }

    pub fn find(&self, val: T) -> Option<&T> {
        for v in self {
            if *v == val { return Some(v)}
        }
        None
    }

    pub fn find_mut(&mut self, val: T) -> Option<&mut T> {
        for v in self {
            if *v == val { return Some(v)}
        }
        None
    }

    pub fn contains(&self, val: T) -> bool { self.find(val).is_some() }
}

impl<T, A> Drop for Vector<T, A>
where A: Allocator
{
    fn drop(&mut self) {
        if !self.first.is_null() {
            unsafe {
                let val = self.get_nonnull();
                let layout = Self::get_layout(self.cap());
                self._allocator.deallocate(val, layout);
            }
        }
    }
}

impl<T, A> From<Vector<T, A>> for Vec<T>
where A: Allocator
{
    fn from(value: Vector<T, A>) -> Self {
        let mut vec = Vec::new();
        for v in value { vec.push(v); }
        vec
    }
}

impl<'a, T, A> IntoIterator for &'a Vector<T, A>
where A: Allocator 
{
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().into_iter()
    }
}

impl<'a, T, A> IntoIterator for &'a mut Vector<T, A>
where A: Allocator 
{
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice_mut().into_iter()
    }
}
impl<T, A> IntoIterator for Vector<T, A>
where A: Allocator
{
    type Item = T;
    type IntoIter = IntoIter<T, A>;
    fn into_iter(self) -> Self::IntoIter {
        let mut m = ManuallyDrop::new(self);
        let ptr = m.as_mut_ptr();
        unsafe {
            Self::IntoIter {
                ptr,
                curr: ptr,
                end: ptr.add(m.len()),
                cap: ptr.add(m.cap()),
                _allocator: std::ptr::read(&m._allocator)
            }
        }
    }
}

pub struct IntoIter<T, A = Global>
where A: Allocator
{
    ptr: *mut T,
    curr: *mut T,
    end: *mut T,
    cap: *mut T,
    _allocator: A
}

impl<T, A> IntoIter<T, A>
where A: Allocator
{
    fn get_layout(&self) -> Layout {
        unsafe { 
            Layout::from_size_align_unchecked(
                self.cap as usize - self.ptr as usize,
                align_of::<T>()
            )
        }
    }
    fn get_nonnull(&self) -> NonNull<u8> {
        unsafe { NonNull::new_unchecked(self.ptr as *mut u8) }
    }
}

impl<T, A> Drop for IntoIter<T, A>
where A: Allocator
{
    fn drop(&mut self) {
        unsafe { self._allocator.deallocate(self.get_nonnull(), self.get_layout()) }
    }
}

impl<T, A> Iterator for IntoIter<T, A>
where A: Allocator
{
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.curr == self.end { None }
        else {
            let v = unsafe { std::ptr::read(self.curr) };
            self.curr = unsafe { self.curr.add(1) };
            Some(v)
        }
    }
}

impl<T, A> Index<usize> for Vector<T, A>
where A: Allocator
{
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.len(), "Tried to access an out of bounds value");
        unsafe { &*self.first.add(index) }
    }
}

impl<T, A> IndexMut<usize> for Vector<T, A>
where A: Allocator
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(index < self.len(), "Tried to access an out of bounds value");
        unsafe { &mut *self.first.add(index) }
    }
}

impl<T, A> Display for Vector<T, A>
where T: Display,
      A: Allocator
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buf = String::from("Vector [ ");
        for (i, v) in self.iter().enumerate() {
            buf.push_str(&format!("{}", v));
            if i < self.len() - 1 { buf.push_str(", ") }
        }
        buf.push_str(" ]");
        write!(f, "{}", &buf)
    }
}

#[cfg(test)]
pub mod tests {
    use super::Vector;
    use std::error::Error;
    type TestReturn = Result<(), Box<dyn Error>>;

    #[test]
    pub fn create_vector() -> TestReturn {
        let mut v: Vector<u32> = Vector::new();
        assert!(v.len() == 0, "Initial length should be zero");
        assert!(v.cap() == 0, "Initial capacity should be zero");
        v.push(1);
        v.push(2);
        v.push(3);
        assert!(v.len() == 3, "New length should be 3");
        assert!(v.cap() == 8, "New capacity should be 8");
        assert!(v.as_slice() == [1, 2, 3], "Values don't match");
        assert!(v.pop() == Some(3), "Popped value should be 3");
        assert!(v.pop() == Some(2), "Popped value should be 2");
        assert!(v.pop() == Some(1), "Popped value should be 1");
        assert!(v.pop() == None, "Popped value should be None");
        Ok(())
    }

    #[test]
    pub fn create_large_vector() -> TestReturn {
        let mut v: Vector<u32> = Vector::new();
        for i in 0..1000 { v.push(i * 2) }
        assert!(v.len() == 1000, "Length should be 1000");
        assert!(v.cap() == 1024, "Length should be 1024");
        assert!(v[0] == 0, "Value at v[0] should be 0");
        assert!(v[360] == 720, "Value at v[360] should be 720");
        Ok(())
    }

    #[test]
    pub fn rust_vector_conversion() -> TestReturn {
        let rv = vec!["a", "b", "c", "d", "e", "f", "g", "h", "i"];
        let v = Vector::from_vec(rv.clone());
        assert!(v.len() == 9, "Length should be 3");
        assert!(v.cap() == 16, "Capacity should be 16");
        let rv_out: Vec<&str> = v.into();
        assert!(rv == rv_out, "Output vec should be the same as vec slice");
        Ok(())
    }

    #[test]
    pub fn slice_iterator_test() -> TestReturn {
        let v = Vector::from_vec(vec![0, 5, 10, 15, 4, 8, 12, 16]);
        let slice_expect = [0, 5, 10, 15, 4, 8, 12, 16];
        for (i, e) in v.iter().enumerate() {
            assert!(*e == slice_expect[i], "Element {} should have value {} instead of {}", i, slice_expect[i], *e);
        }
        Ok(())
    }
    #[test]
    pub fn list_find() -> TestReturn {
        let list = Vector::from_vec(vec![20, 30, 15, 5, 40, 25]);
        assert!(!list.contains(10), "List doesn't contain 10, but was found anyway");
        assert!(list.contains(30), "List contains 30, but wasn't found");
        assert!(list.index_of(40) == Some(4), "40 should be the fifth element");
        assert!(list.index_of(10) == None, "10 is not in the list");
        assert!(list.index_of_by_predicate(|f| f * 2 == 10) == Some(3), "Fourth element should be found (5)");
        assert!(*list.find_by_predicate(|f| f * 2 == 10).unwrap() == 5, "Should have found foruth element (5)");
        Ok(())
    }
}
