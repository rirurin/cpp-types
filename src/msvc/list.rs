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
    // first: Option<NonNull<N>>,
    head: *mut N,
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
        let head = N::new_nil(alloc.clone());
        Self {
            head,
            len: 0,
            _allocator: alloc,
            _data: PhantomData
        }
    }
    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn first(&self) -> Option<&N> {
        unsafe { (&*self.head).next(self.head) }
    }
    pub fn first_mut(&mut self) -> Option<&mut N> {
        unsafe { (&mut *self.head).next_mut(self.head) }
    }
    pub fn get_nil(&self) -> *mut N { self.head }
    pub fn last(&self) -> Option<&N> {
        let mut curr = self.first();
        if curr.is_none() { return None };
        while curr.as_ref().unwrap().next(self.head).is_some() {
            curr = curr.unwrap().next(self.head);
        }
        curr
    }
    pub fn last_mut(&mut self) -> Option<&mut N> {
        let head = self.head;
        let mut curr = self.first_mut();
        if curr.is_none() { return None };
        while curr.as_mut().unwrap().next_mut(head).is_some() {
            curr = curr.unwrap().next_mut(head);
        }
        curr
    }

    pub fn push(&mut self, val: T) {
        let head = self.head;
        let alloc = self._allocator.clone();
        let new = N::new(val, alloc, head); 
        match self.last_mut() {
            Some(v) => v.set_next(new, head),
            // set first entry
            None => unsafe { (&mut *head).set_next(new, head) }
        };
        self.len += 1;
    }

    pub fn insert(&mut self, after_index: usize, val: T) {
        assert!(self.len > after_index, "Tried to insert value out of bounds");
        let head = self.head;
        let allocator = self._allocator.clone();
        let insert_after = self.get_unchecked_mut(after_index);
        let new = N::new(val, allocator, head);
        if insert_after.next(head).is_some() {
            unsafe { (&mut *new).set_next(&raw mut *insert_after.next_mut(head).unwrap(), head) };
        }
        insert_after.set_next(new, head);
        self.len += 1;
    }

    pub(crate) unsafe fn insert_after_unchecked(&mut self, insert_after: *mut N, val: T) {
        let head = self.head;
        let allocator = self._allocator.clone();
        let new = N::new(val, allocator, head);
        let insert_after = &mut *insert_after;
        if insert_after.next(head).is_some() {
            (&mut *new).set_next(&raw mut *insert_after.next_mut(head).unwrap(), head);
        }
        insert_after.set_next(new, head);
        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        let head = self.head;
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
        prev.set_next(head, head);
        // if self.len == 1 { self.first = None; }
        self.len -= 1;
        Some(value)
    }
    pub fn get(&self, index: usize) -> Option<&N> {
        let mut curr = self.first();
        for _ in 0..index {
            curr = match curr {
                Some(v) => v.next(self.head),
                None => return None
            };
        }
        curr
    }
    pub fn get_mut(&mut self, index: usize) -> Option<&mut N> {
        let head = self.head;
        let mut curr = self.first_mut();
        for _ in 0..index {
            curr = match curr {
                Some(v) => v.next_mut(head),
                None => return None
            };
        }
        curr
    }
    pub fn get_unchecked(&self, index: usize) -> &N {
        assert!(self.len > index, "Tried to access out of bounds");
        let mut curr = self.first().unwrap();
        for _ in 0..index { curr = curr.next(self.head).unwrap() }
        curr
    }
    pub fn get_unchecked_mut(&mut self, index: usize) -> &mut N {
        let head = self.head;
        assert!(self.len > index, "Tried to access out of bounds");
        let mut curr = self.first_mut().unwrap();
        for _ in 0..index { curr = curr.next_mut(head).unwrap() }
        curr
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
            nil: self.head,
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
            nil: self.head,
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
    nil: *mut N,
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
            self.curr = v.next(self.nil);
            v.value()
        })
    }
}

pub struct ListIteratorMut<'a, N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    nil: *mut N,
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
            self.curr = unsafe { v.next_ptr(self.nil).map(|mut f| f.as_mut()) };
            v.value_mut()
        })
    }
}

impl<N, T, A> Drop for List<N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    fn drop(&mut self) {
        let mut curr_node = self.first();
        while let Some(v) = curr_node {
            curr_node = v.next(self.head);
            unsafe { std::ptr::drop_in_place(&raw const *v as *mut ListNode<T, A>) };
        }
        unsafe { std::ptr::drop_in_place(self.head) };
    }
}

impl<N, T, A> Index<usize> for List<N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        self.get_unchecked(index).value()
    }
}

impl<N, T, A> IndexMut<usize> for List<N, T, A>
where N: ListSingleNode<T, A>,
      A: Allocator + Clone
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_unchecked_mut(index).value_mut()
    }
}

#[repr(C)]
pub struct ListNode<T, A = Global>
where A: Allocator
{
    // next: Option<NonNull<Self>>,
    // prev: Option<NonNull<Self>>,
    next: *mut Self,
    prev: *mut Self,
    val: T,
    _allocator: A
}

#[repr(C)]
pub struct ListForwardNode<T, A = Global>
where A: Allocator
{
    // next: Option<NonNull<Self>>,
    next: *mut Self,
    val: T,
    _allocator: A
}

pub trait ListSingleNode<T, A>
where A: Allocator
{
    fn new(val: T, alloc: A, nil: *mut Self) -> *mut Self where Self: Sized;
    fn new_nil(alloc: A) -> *mut Self where Self: Sized;
    fn next(&self, nil: *mut Self) -> Option<&Self> where Self: Sized;
    fn next_mut(&mut self, nil: *mut Self) -> Option<&mut Self> where Self: Sized;
    fn next_ptr(&mut self, nil: *mut Self) -> Option<NonNull<Self>> where Self: Sized;
    fn value(&self) -> &T;
    fn value_mut(&mut self) -> &mut T;
    fn set_next(&mut self, next: *mut Self, nil: *mut Self) where Self: Sized;
}

pub trait ListDoubleNode<T, A> : ListSingleNode<T, A>
where A: Allocator
{
    fn prev(&self, nil: *mut Self) -> Option<&Self> where Self: Sized;
    fn prev_mut(&mut self, nil: *mut Self) -> Option<&mut Self> where Self: Sized;
    fn prev_ptr(&self, nil: *mut Self) -> Option<NonNull<Self>> where Self: Sized;
}

impl<T, A> ListSingleNode<T, A> for ListNode<T, A>
where A: Allocator + Clone
{
    fn new(val: T, alloc: A, nil: *mut Self) -> *mut Self where Self: Sized {
        let new = alloc.allocate(Layout::new::<Self>()).unwrap().as_ptr() as *mut Self;
        let new_edit: &mut Self = unsafe { &mut *new };
        new_edit.next = nil;
        new_edit.prev = nil;
        new_edit.val = val;
        new
    }

    fn new_nil(alloc: A) -> *mut Self where Self: Sized {
        let new = alloc.allocate(Layout::new::<Self>()).unwrap().as_ptr() as *mut Self;
        let new_edit: &mut Self = unsafe { &mut *new };
        new_edit.next = new;
        new_edit.prev = new;
        new_edit._allocator = alloc;
        new
    }

    fn next(&self, nil: *mut Self) -> Option<&Self> where Self: Sized {
        match std::ptr::eq(self.next, nil) {
            true => None,
            false => Some(unsafe{&*self.next})
        }
    }
    fn next_mut(&mut self, nil: *mut Self) -> Option<&mut Self> where Self: Sized {
        match std::ptr::eq(self.next, nil) {
            true => None,
            false => Some(unsafe{&mut *self.next})
        }
    }
    fn next_ptr(&mut self, nil: *mut Self) -> Option<NonNull<Self>> where Self: Sized {
        match std::ptr::eq(self.next, nil) {
            true => None,
            false => Some(unsafe{NonNull::new_unchecked(self.next)})
        }
    }
    fn set_next(&mut self, next: *mut Self, nil: *mut Self) where Self: Sized {
        if next != nil {
            unsafe { (&mut *next).prev = self.next }
        }
        self.next = next;
    }
    fn value(&self) -> &T { &self.val }
    fn value_mut(&mut self) -> &mut T { &mut self.val }
}

impl<T, A> ListDoubleNode<T, A> for ListNode<T, A>
where A: Allocator + Clone
{
    fn prev(&self, nil: *mut Self) -> Option<&Self> where Self: Sized {
        match std::ptr::eq(self.prev, nil) {
            true => None,
            false => Some(unsafe{&*self.prev})
        }
    }
    fn prev_mut(&mut self, nil: *mut Self) -> Option<&mut Self> where Self: Sized {
        match std::ptr::eq(self.prev, nil) {
            true => None,
            false => Some(unsafe{&mut *self.prev})
        }
    }
    fn prev_ptr(&self, nil: *mut Self) -> Option<NonNull<Self>> where Self: Sized {
        match std::ptr::eq(self.prev, nil) {
            true => None,
            false => Some(unsafe{NonNull::new_unchecked(self.prev)})
        }
    }
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
    use super::{ List, ListNode, ListSingleNode };

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

    #[test]
    pub fn list_insertion() -> TestReturn {
        let mut list = List::new();
        list.push(1);
        list.push(3);
        list.push(5);
        list.push(7);
        list.push(9);
        list.insert(0, 2);
        assert!(list[1] == 2, "Second list entry should be 2");
        list.insert(2, 4);
        assert!(list[3] == 4, "Fourth list entry should be 4");
        let five = &raw mut *list.get_unchecked_mut(4);
        let seven = &raw mut *list.get_unchecked_mut(5);
        unsafe { list.insert_after_unchecked(five, 6); }
        unsafe { list.insert_after_unchecked(seven, 8); }
        assert!(list[5] == 6, "Sixth list entry should be 7");
        assert!(list[7] == 8, "Eighth list entry should be 8");
        for i in &list {
            assert!(list[*i-1] == *i, "Element {} should equal {} instead of {}", *i-1, *i, list[*i-1]);
        }
        Ok(())
    }
}
