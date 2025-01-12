#![allow(dead_code, unused_imports)]
use allocator_api2::alloc::{ Allocator, Global };
use std::{
    alloc::Layout,
    marker::PhantomData,
    ops::{ Index, IndexMut },
    ptr::NonNull
};

// See https://devblogs.microsoft.com/oldnewthing/20230804-00/?p=108547
// https://github.com/microsoft/STL/blob/main/stl/inc/list

type ForwardList<T, A> = List<ListForwardNode<T, A>, T, A>; // std::forward_list

#[repr(C)]
pub struct List<N, T, A = Global> // std::list
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    first: Option<NonNull<N>>,
    len: usize,
    _allocator: A,
    _data: PhantomData<T>
}

impl<N, T, A> List<N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    pub fn new(alloc: A) -> Self {
        assert!(std::mem::size_of::<T>() == 0, "Allocator must be zero-sized!");
        Self {
            first: None,
            len: 0,
            _allocator: alloc,
            _data: PhantomData
        }
    }
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.first.is_none() }
    pub fn first(&self) -> Option<&N> {
        unsafe { self.first.map(|f| f.as_ref()) }
    }
    pub fn first_mut(&mut self) -> Option<&mut N> {
        unsafe { self.first.map(|mut f| f.as_mut()) }
    }
    pub fn last(&self) -> Option<&N> {
        let mut target_node = self.first;
        while let Some(v) = target_node {
            target_node = unsafe { v.as_ref() }.next();
        }
        target_node.map(|f| unsafe { f.as_ref() })
    }
    pub fn last_mut(&mut self) -> Option<&mut N> {
        let mut target_node = self.first;
        while let Some(mut v) = target_node {
            target_node = unsafe { v.as_mut() }.next();
        }
        target_node.map(|mut f| unsafe { f.as_mut() })
    }
    pub fn push(&mut self, val: T) {
        let alloc = self._allocator.clone();
        self.len += 1;
        match self.last_mut() {
            Some(v) => v.set_next(Some(N::new(val, alloc))),
            None => ()
        };
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() { return None }
        // get last entry value
        let last = self.last().unwrap();
        let value = unsafe { std::ptr::read(last.value()) };
        // drop
        // set pointer for prev item
        let prev = if self.len == 1 {
            self.first_mut().unwrap()
        } else {
            let prev_index = self.len - 2;
            &mut self[prev_index]
        };
        prev.set_next(None);
        self.len -= 1;
        Some(value)
    }
}
/*
impl<N, T, A> Drop for List<N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{

}
*/

impl<N, T, A> Index<usize> for List<N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    type Output = N;
    fn index(&self, index: usize) -> &Self::Output {
        assert!(self.len < index, "Tried to access out of bounds");
        let mut node = unsafe { self.first.unwrap().as_mut() };
        for _ in 0..index { node = unsafe { node.next().unwrap().as_mut() }; }
        node
    }
}

impl<N, T, A> IndexMut<usize> for List<N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(self.len < index, "Tried to access out of bounds");
        let mut node = unsafe { self.first.unwrap().as_mut() };
        for _ in 0..index { node = unsafe { node.next().unwrap().as_mut() }; }
        node
    }
}

#[repr(C)]
pub struct ListNode<T, A = Global>
where A: Allocator
{
    next: Option<NonNull<Self>>,
    prev: Option<NonNull<Self>>,
    val: T,
    _allocator: A
}

#[repr(C)]
pub struct ListForwardNode<T, A = Global>
where A: Allocator
{
    next: Option<NonNull<Self>>,
    val: T,
    _allocator: A
}

impl<T, A> ListSingleNode<T, A> for ListNode<T, A>
where A: Allocator + Clone
{
    fn new(val: T, alloc: A) -> NonNull<Self> where Self: Sized {
        let mut new = alloc.allocate(Layout::new::<Self>()).unwrap().cast();
        let new_edit: &mut ListNode<T, A> = unsafe { new.as_mut() };
        new_edit.next = None;
        new_edit.prev = None;
        new_edit.val = val;
        new_edit._allocator = alloc;
        new
    }
    fn next(&self) -> Option<NonNull<Self>> where Self: Sized {
        self.next
    }
    fn set_next(&mut self, next: Option<NonNull<Self>>) where Self: Sized {
        if let Some(mut v) = next {
            unsafe { v.as_mut() }.prev = self.next;
        }
        self.next = next;
    }
    fn value(&self) -> &T {
        &self.val
    }
}

impl<T, A> ListDoubleNode<T, A> for ListNode<T, A>
where A: Allocator + Clone
{
    fn prev(&self) -> Option<NonNull<Self>> where Self: Sized {
        self.prev
    }
}

pub trait ListSingleNode<T, A>
where A: Allocator
{
    fn new(val: T, alloc: A) -> NonNull<Self> where Self: Sized;
    fn next(&self) -> Option<NonNull<Self>> where Self: Sized;
    fn value(&self) -> &T;
    fn set_next(&mut self, next: Option<NonNull<Self>>) where Self: Sized;
}

pub trait ListDoubleNode<T, A> : ListSingleNode<T, A>
where A: Allocator
{
    fn prev(&self) -> Option<NonNull<Self>> where Self: Sized;
}

#[cfg(test)]
pub mod tests {

}
