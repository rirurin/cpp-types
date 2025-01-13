#![allow(dead_code, unused_imports)]
use allocator_api2::alloc::{ Allocator, Global };
use std::{
    alloc::Layout,
    fmt::Display,
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

impl<T> List<ListNode<T, Global>, T, Global> {
    pub fn new() -> Self { Self::new_inner(Global) }
    pub fn from_vec(vec: Vec<T>) -> Self { Self::from_vec_in(vec, Global) }
}
impl<T, A> List<ListNode<T, A>, T, A>
where A: Allocator + Clone
{
    pub fn new_in(alloc: A) -> Self { Self::new_inner(alloc) }
}

impl<N, T, A> List<N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    fn new_inner(alloc: A) -> Self {
        assert!(std::mem::size_of::<A>() == 0, "Allocator must be zero-sized!");
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
        let mut iter = self.first;
        let mut target_node = iter;
        while let Some(mut v) = iter {
            iter = unsafe { v.as_mut() }.next();
            target_node = Some(v);
        }
        target_node.map(|mut f| unsafe { f.as_mut() })
    }
    pub fn push(&mut self, val: T) {
        let alloc = self._allocator.clone();
        let new = Some(N::new(val, alloc));
        match self.last_mut() {
            Some(v) => v.set_next(new),
            None => self.first = new // first entry
        };
        self.len += 1;
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() { return None }
        // get last entry value
        let last = self.last_mut().unwrap();
        let value = unsafe { std::ptr::read(last.value()) };
        unsafe { std::ptr::drop_in_place(&raw mut *last) };
        // set pointer for prev item
        let prev = if self.len == 1 {
            self.first_mut().unwrap()
        } else {
            let prev_index = self.len - 2;
            self.get_mut(prev_index).unwrap()
        };
        prev.set_next(None);
        if self.len == 1 { self.first = None; }
        self.len -= 1;
        Some(value)
    }
    pub fn get(&self, index: usize) -> Option<&N> {
        let mut curr = self.first();
        for _ in 0..index {
            if curr.is_none() { return None }
            curr = unsafe { curr.unwrap().next().map(|f| f.as_ref()) };
        }
        curr
    }
    pub fn get_mut(&mut self, index: usize) -> Option<&mut N> {
        let mut curr = self.first_mut();
        for _ in 0..index {
            if curr.is_none() { return None }
            curr = unsafe { curr.unwrap().next().map(|mut f| f.as_mut()) };
        }
        curr
    }
    pub fn get_unchecked(&self, index: usize) -> &N {
        assert!(self.len > index, "Tried to access out of bounds");
        let mut curr = self.first();
        for _ in 0..index {
            curr = unsafe { curr.unwrap().next().map(|f| f.as_ref()) };
        }
        curr.unwrap()
    }
    pub fn get_unchecked_mut(&mut self, index: usize) -> &mut N {
        assert!(self.len > index, "Tried to access out of bounds");
        let mut curr = self.first_mut();
        for _ in 0..index {
            curr = unsafe { curr.unwrap().next().map(|mut f| f.as_mut()) };
        }
        curr.unwrap()
    }
    pub fn iter(&self) -> ListIterator<'_, N, T, A> { self.into_iter() }
    pub fn iter_mut(&mut self) -> ListIteratorMut<'_, N, T, A> { self.into_iter() }

    pub fn from_vec_in(vec: Vec<T>, alloc: A) -> Self {
        assert!(std::mem::size_of::<A>() == 0, "Allocator must be zero-sized!");
        let mut new = List::new_inner(alloc);
        for el in vec { new.push(el) }
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

impl<N, T, A> List<N, T, A>
where N: ListSingleNode<T, A>,
      T: PartialEq,
      A: Allocator + Clone
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

impl<'a, N, T, A> IntoIterator for &'a List<N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    type Item = &'a T;
    type IntoIter = ListIterator<'a, N, T, A>;
    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            curr: self.first(),
            _type_marker: PhantomData::<T>,
            _alloc_marker: PhantomData::<A>
        }
    }
}

impl<'a, N, T, A> IntoIterator for &'a mut List<N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    type Item = &'a mut T;
    type IntoIter = ListIteratorMut<'a, N, T, A>;
    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            curr: self.first_mut(),
            _type_marker: PhantomData::<T>,
            _alloc_marker: PhantomData::<A>
        }
    }
}

pub struct ListIterator<'a, N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    curr: Option<&'a N>,
    _type_marker: std::marker::PhantomData<T>,
    _alloc_marker: std::marker::PhantomData<A>
}

impl<'a, N, T: 'a, A> Iterator for ListIterator<'a, N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        self.curr.take().map(|v| {
            self.curr = v.next().map(|v2| unsafe { v2.as_ref() });
            v.value()
        })
    }
}

pub struct ListIteratorMut<'a, N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    curr: Option<&'a mut N>,
    _type_marker: std::marker::PhantomData<T>,
    _alloc_marker: std::marker::PhantomData<A>
}

impl<'a, N, T: 'a, A> Iterator for ListIteratorMut<'a, N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
        self.curr.take().map(|v| {
            self.curr = v.next().map(|mut v2| unsafe { v2.as_mut() });
            v.value_mut()
        })
    }
}

impl<N, T, A> Drop for List<N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    fn drop(&mut self) {
        let mut curr_node = self.first;
        while let Some(mut v) = curr_node {
            curr_node = unsafe { v.as_mut() }.next();
            unsafe { std::ptr::drop_in_place(v.as_ptr()) };
        }   
    }
}

impl<N, T, A> Index<usize> for List<N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        assert!(self.len > index, "Tried to access out of bounds");
        let mut node = unsafe { self.first.unwrap().as_mut() };
        for _ in 0..index { node = unsafe { node.next().unwrap().as_mut() }; }
        node.value()
    }
}

impl<N, T, A> IndexMut<usize> for List<N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(self.len > index, "Tried to access out of bounds");
        let mut node = unsafe { self.first.unwrap().as_mut() };
        for _ in 0..index { node = unsafe { node.next().unwrap().as_mut() }; }
        node.value_mut()
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
    fn value(&self) -> &T { &self.val }
    fn value_mut(&mut self) -> &mut T { &mut self.val }
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
    fn value_mut(&mut self) -> &mut T;
    fn set_next(&mut self, next: Option<NonNull<Self>>) where Self: Sized;
}

pub trait ListDoubleNode<T, A> : ListSingleNode<T, A>
where A: Allocator
{
    fn prev(&self) -> Option<NonNull<Self>> where Self: Sized;
}

impl<N, T, A> Display for List<N, T, A>
where N: ListSingleNode<T, A>,
      T: Display,
      A: Allocator + Clone
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buf = String::from("List [ ");
        for (i, v) in self.iter().enumerate() {
            buf.push_str(&format!("{}", v));
            if i < self.len() - 1 { buf.push_str(", ") }
        }
        buf.push_str(" ]");
        write!(f, "{}", &buf)
    }
}

impl<N, T, A> From<List<N, T, A>> for Vec<T>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone,
{
    fn from(mut value: List<N, T, A>) -> Self {
        let old_len = value.len();
        let mut vec: Vec<T> = Vec::with_capacity(old_len);
        unsafe { vec.set_len(old_len); }
        for i in 0..old_len {
            let val = value.pop().unwrap();
            unsafe { vec.as_mut_ptr().add(old_len-i-1).write(val); }
        }        
        vec
    }
}

#[cfg(test)]
pub mod tests {
    use allocator_api2::alloc::Global;
    use super::{ List, ListNode, ListSingleNode, ListDoubleNode };

    use std::error::Error;
    type TestReturn = Result<(), Box<dyn Error>>;

    #[test]
    pub fn create_blank_list() -> TestReturn {
        let list: List<ListNode<u32, Global>, u32, Global> = List::new();
        assert!(list.len() == 0, "List should be blank");
        assert!(list.first().is_none(), "First element should be blank");
        Ok(())
    }

    #[test]
    pub fn create_list() -> TestReturn {
        let mut list = List::new();
        list.push(1);
        list.push(2);
        list.push(3);
        for i in 0..list.len() {
            assert!(list[i] == i + 1, "Element {} should have item {} instead of {}", i, i + 1, list[i]);
        }
        assert!(list.pop() == Some(3), "Element should be 3");
        assert!(list.pop() == Some(2), "Element should be 2");
        assert!(list.pop() == Some(1), "Element should be 1");
        assert!(list.pop() == None, "List should be empty");
        Ok(())
    }

    #[test]
    pub fn list_iterator() -> TestReturn {
        let mut list = List::new();
        for i in 5..10 { list.push(i) }
        for (i, v) in list.iter().enumerate() {
            assert!(*v == i + 5, "Element {} should have item {} instead of {}", i, i + 5, *v);
        }
        for (i, v) in list.iter_mut().enumerate() {
            *v *= 2;
            assert!(*v == (i + 5) * 2, "Element {} should have item {} instead of {}", i, (i + 5) * 2, *v);
        }
        Ok(())
    }

    #[test]
    pub fn rust_list_conversion() -> TestReturn {
        let rust_list = vec!["a", "b", "c", "d", "e"];
        let list = List::from_vec(rust_list.clone());
        for (i, v) in list.iter().enumerate() {
            assert!(*v == rust_list[i], "Element {} should have {} instead of {}", i, rust_list[i], *v);
        }
        let list_out: Vec<&str> = list.into();
        for (a, b) in list_out.iter().zip(rust_list.iter()) {
            assert!(*a == *b, "Out list doesn't equal in list: {} != {}", *a, *b);
        }
        Ok(())
    }

    #[test]
    pub fn list_find() -> TestReturn {
        let list = List::from_vec(vec![20, 30, 15, 5, 40, 25]);
        assert!(!list.contains(10), "List doesn't contain 10, but was found anyway");
        assert!(list.contains(30), "List contains 30, but wasn't found");
        assert!(list.index_of(40) == Some(4), "40 should be the fifth element");
        assert!(list.index_of(10) == None, "10 is not in the list");
        assert!(list.index_of_by_predicate(|f| f * 2 == 10) == Some(3), "Fourth element should be found (5)");
        assert!(*list.find_by_predicate(|f| f * 2 == 10).unwrap() == 5, "Should have found foruth element (5)");
        Ok(())
    }
}
