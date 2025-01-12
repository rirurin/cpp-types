#![allow(dead_code, unused_imports)]
use allocator_api2::alloc::{ Allocator, Global };
use std::{
    alloc::Layout,
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

impl<T, A> Vector<T, A>
where A: Allocator
{
    pub fn new(alloc: A) -> Self {
        assert!(std::mem::size_of::<T>() == 0, "Allocator must be zero-sized!");
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
        unsafe { self.last.sub(self.first as usize) as usize / std::mem::size_of::<T>() }
    }
    pub fn cap(&self) -> usize {
        unsafe { self.end.sub(self.first as usize) as usize / std::mem::size_of::<T>() }
    }
    pub fn resize(&mut self, new: usize) {
        unsafe {
            let alloc = self._allocator.allocate(Self::get_layout(new)).unwrap().as_ptr() as *mut T;
            // if old exists, copy pointer
            if !self.first.is_null() {
                std::ptr::copy_nonoverlapping(self.first, alloc, new);
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
        if self.len() == 0 { self.resize(START_ALLOC_SIZE); }
        unsafe { 
            std::ptr::write(self.first.add(self.len()), val); 
            self.last = self.last.add(1);
        }
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.len() > 0 {
            let _ = unsafe { self.last.sub(1) };
            Some(unsafe { std::ptr::read(self.first.add(self.len()))})
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

    pub fn iter(&self) -> Iter<'_, T> {
        self.as_slice().iter()
    }
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        self.as_slice_mut().iter_mut()
    }
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
/*
impl<T, A> From<Vector<T, A>> for Vec<T>
where A: Allocator
{
    fn from(value: Vector<T, A>) -> Self {
        let mut vec = Vec::with_capacity(value.cap());
        for v in value {
            let a = v;
        }
        vec
    }
}
*/
/*
impl<'a, T, A> IntoIterator for &'a Vector<T, A>
where A: Allocator 
{
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        
    }
}

impl<'a, T, A> IntoIterator for &'a mut Vector<T, A>
where A: Allocator 
{
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        
    }
}
*/
/*
impl<T, A> IntoIterator for Vector<T, A>
where A: Allocator
{
    type Item = T;
    type IntoIter = Iter<'_, T>;
    fn into_iter(self) -> Self::IntoIter {
        let m = ManuallyDrop::new(self);
        // self.iter()
    }
}

struct IntoIter<T, A>
where A: Allocator
{
    ptr: *mut T,

    _allocator: A
}
*/

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

#[cfg(test)]
pub mod tests {
    
}
