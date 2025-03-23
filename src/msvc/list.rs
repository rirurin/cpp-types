#![allow(dead_code, unused_imports)]
use allocator_api2::alloc::{ Allocator, Global };
use std::{
    alloc::Layout,
    fmt::Display,
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::{ Add, Index, IndexMut },
    ptr::NonNull
};

// See https://devblogs.microsoft.com/oldnewthing/20230804-00/?p=108547
// https://github.com/microsoft/STL/blob/main/stl/inc/list

type ForwardList<T, A> = List<ListForwardNode<T, A>, T, A>; // std::forward_list

#[repr(C)]
pub struct List<N, T, A = Global> // std::list
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
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
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
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
    pub(crate) unsafe fn set_len(&mut self, new: usize) { self.len = new }

    pub fn first(&self) -> Option<&N> {
        unsafe { (&*self.head).next(self.head) }
    }
    pub fn first_mut(&mut self) -> Option<&mut N> {
        unsafe { (&mut *self.head).next_mut(self.head) }
    }
    pub fn first_ptr(&mut self) -> Option<NonNull<N>> {
        unsafe { (&mut *self.head).next_mut(self.head).map(|f| NonNull::new_unchecked(&raw mut *f)) }
    }
    pub fn last(&self) -> Option<&N> {
        unsafe { (&*self.head).prev(self.head) }
    }
    pub fn last_mut(&mut self) -> Option<&mut N> {
        unsafe { (&mut *self.head).prev_mut(self.head) }
    }
    pub fn last_ptr(&mut self) -> Option<NonNull<N>> {
        unsafe{ (&mut *self.head).prev_mut(self.head).map(|f| NonNull::new_unchecked(&raw mut *f)) }
    }

    pub fn get_nil(&self) -> *mut N { self.head }

    pub fn push(&mut self, val: T) {
        let head = self.head;
        let alloc = self._allocator.clone();
        let new = N::new(val, alloc, head); 
        match self.last_mut() {
            Some(v) => v.set_next(new, head),
            // set first entry
            None => unsafe { (&mut *head).set_next(new, head) }
        };
        unsafe { (&mut *head).set_prev(new, head) };
        self.len += 1;
    }
    // std::list::insert inserts the value before the index position
    // additionally, if index is set to self.len(), call push instead
    pub fn insert(&mut self, after_index: usize, val: T) {
        assert!(after_index <= self.len(), "Tried to insert value out of bounds");
        if self.len() == after_index { self.push(val) }
        else {
            let node = &raw mut *self.get_unchecked_mut(after_index);
            unsafe { self.insert_at_unchecked(node, val) }
        }
    }

    pub(crate) unsafe fn insert_at_unchecked(&mut self, node: *mut N, val: T) {
        let head = self.head;
        let allocator = self._allocator.clone();
        let new = N::new(val, allocator, head);
        self.link_before(&mut *node, &mut *new);
        self.len += 1;
    }

    pub(crate) unsafe fn insert_after_unchecked(&mut self, insert_after: *mut N, val: T) {
        let head = self.head;
        let allocator = self._allocator.clone();
        let new = N::new(val, allocator, head);
        self.link_after(&mut *insert_after, &mut *new);
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
        unsafe { (&mut *head).set_prev(prev, head) };
        self.len -= 1;
        Some(value)
    }

    pub fn pop_front(&mut self) -> Option<T> {
        match self.get_mut(0) {
            Some(v) => {
                let pv = &raw mut *v;
                unsafe { Some(self.remove_unchecked(pv)) }
            },
            None => None
        }
    }

    pub fn get(&self, index: usize) -> Option<&N> {
        if index >= self.len() { return None; } 
        Some(self.get_unchecked(index))
    }

    pub fn get_unchecked(&self, index: usize) -> &N {
        match index <= (self.len() - 1) / 2 {
            true => self.get_from_start(index),
            false => self.get_from_end(self.len() - 1 - index)
        }
    }

    fn get_from_start(&self, index: usize) -> &N {
        let mut curr = self.first().unwrap();
        for _ in 0..index { curr = curr.next(self.head).unwrap(); }
        curr
    }

    fn get_from_end(&self, from_end: usize) -> &N {
        let mut curr = self.last().unwrap();
        for _ in 0..from_end { curr = curr.prev(self.head).unwrap(); }
        curr
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut N> {
        if index >= self.len() { return None; }
        Some(self.get_unchecked_mut(index))
    }

    pub fn get_unchecked_mut(&mut self, index: usize) -> &mut N {
        match index <= (self.len() - 1) / 2 {
            true => self.get_from_start_mut(index),
            false => self.get_from_end_mut(self.len() - 1 - index)
        }
    }

    fn get_from_start_mut(&mut self, index: usize) -> &mut N {
        let head = self.head;
        let mut curr = self.first_mut().unwrap();
        for _ in 0..index { curr = curr.next_mut(head).unwrap(); }
        curr
    }

    fn get_from_end_mut(&mut self, from_end: usize) -> &mut N {
        let head = self.head;
        let mut curr = self.last_mut().unwrap();
        for _ in 0..from_end { curr = curr.prev_mut(head).unwrap(); }
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
        for v in self { if cb(v) { return Some(v) } }
        None
    }

    pub fn clear(&mut self) {
        let head = self.get_nil();
        let mut node = self.first_ptr();
        while let Some(mut n) = node {
            node = unsafe { (&mut *n.as_mut()).next_ptr(head) };
            unsafe { std::ptr::drop_in_place(n.as_mut()); }
        }
        self.len = 0;
        unsafe {
            (&mut *head).set_next(head, head);
            (&mut *head).set_prev(head, head);
        }
    }
}

impl<N, T, A> List<N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
      A: Allocator + Clone
{ 

    pub(crate) fn link_before(&mut self, before: &mut N, new: &mut N) {
        let head = self.head;
        match before.prev_mut(head) {
            Some(v) => v.set_next(new, head),
            None => unsafe { (&mut *head).set_next(new, head) }
        }
        new.set_next(before, head);
    }

    pub(crate) fn link_after(&mut self, after: &mut N, new: &mut N) {
        let head = self.head;
        match after.next_mut(head) {
            Some(v) => new.set_next(v, head),
            None => unsafe { (&mut *head).set_prev(new, head) }
        }
        after.set_next(new, head);
    }

    pub(crate) fn link_first(&mut self, first: &mut N) {
        if self.len() > 0 { return; }
        let head = self.head;
        unsafe { (&mut *head).set_next(first, head) }
        unsafe { (&mut *head).set_prev(first, head) }
    }

    // pub(crate) fn remove(&mut self, index: usize) -> T {
    pub fn remove(&mut self, index: usize) -> T {
        assert!(self.len > index, "Tried to remove value out of bounds");
        let el = self.get_unchecked_mut(index);
        let el_ptr = &raw mut *el;
        unsafe { self.remove_unchecked(el_ptr) }
    }

    pub fn remove_checked(&mut self, index: usize) -> Option<T> {
        if self.len > index { Some(self.remove(index)) }
        else { None }
    }

    unsafe fn unlink(&mut self, p_element: *mut N) {
        // remove curr from the linked list without deallocating it
        let head = self.head;
        let element = &mut* p_element;
        let next = match element.next_mut(head) {
            Some(v) => &raw mut *v,
            None => {
                (&mut *head).set_prev(
                    match element.prev_mut(head) { Some(v) => &raw mut *v, None => head }, 
                    head);
                head
            }
        };
        match element.prev_mut(head) {
            Some(v) => v.set_next(next, head),
            None => (&mut *head).set_next(next, head)
        };
    }

    pub(crate) unsafe fn remove_unchecked(&mut self, p_element: *mut N) -> T {
        self.len -= 1;
        self.unlink(p_element);
        let element = &mut *p_element;
        let val_out = std::ptr::read(element.value());
        std::ptr::drop_in_place(element);
        val_out
    }

    pub(crate) fn move_node(&mut self, old: usize, new: usize) -> bool {
        if self.len() < old || self.len() < new { return false; }
        unsafe {
            match self.len() == new {
                true => self.move_node_after_index_unchecked(old, new-1),
                false => self.move_node_before_index_unchecked(old, new),
            }
        }
    }
    pub(crate) unsafe fn move_node_before_index_unchecked(&mut self, old: usize, new: usize) -> bool {
        let val = &raw mut *self.get_unchecked_mut(old);
        self.unlink(&mut *val);
        let attach_to = &raw mut *self.get_unchecked_mut(new);
        self.link_before(&mut *attach_to, &mut *val);
        true
    }
    pub(crate) unsafe fn move_node_after_index_unchecked(&mut self, old: usize, new: usize) -> bool {
        let val = &raw mut *self.get_unchecked_mut(old);
        self.unlink(&mut *val);
        let attach_to = &raw mut *self.get_unchecked_mut(new);
        self.link_after(&mut *attach_to, &mut *val);
        true
    }
    pub(crate) unsafe fn move_node_before_unchecked(&mut self, target: &mut N, attach: &mut N) {
        self.unlink(&raw mut *target);
        self.link_before(attach, target);
    }
    pub(crate) unsafe fn move_node_after_unchecked(&mut self, target: &mut N, attach: &mut N) {
        self.unlink(&raw mut *target);
        self.link_after(attach, target);
    }
}

impl<N, T, A> List<N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
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
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
      A: Allocator + Clone
{
    type Item = &'a T;
    type IntoIter = ListIterator<'a, N, T, A>;
    fn into_iter(self) -> Self::IntoIter {
        Self::IntoIter {
            nil: self.head,
            curr: self.first(),
            curr_rev: self.last(),
            _type_marker: PhantomData::<T>,
            _alloc_marker: PhantomData::<A>
        }
    }
}

impl<'a, N, T, A> IntoIterator for &'a mut List<N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
      A: Allocator + Clone
{
    type Item = &'a mut T;
    type IntoIter = ListIteratorMut<'a, N, T, A>;
    fn into_iter(self) -> Self::IntoIter {
        let nil = self.head;
        // Rust isn't aware that we can safely split borrows here, since implementors of
        // DoubleEndedIterator can't allow the forward and back iterators to cross over
        let curr = unsafe {(&mut *self.head).next_mut(nil) }; 
        let curr_rev = self.last_mut();
        Self::IntoIter {
            nil, curr, curr_rev,
            _type_marker: PhantomData::<T>,
            _alloc_marker: PhantomData::<A>
        }
    }
}

pub struct ListIterator<'a, N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
      A: Allocator + Clone
{
    nil: *mut N,
    curr: Option<&'a N>,
    curr_rev: Option<&'a N>,
    _type_marker: std::marker::PhantomData<T>,
    _alloc_marker: std::marker::PhantomData<A>
}

impl<'a, N, T: 'a, A> ListIterator<'a, N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
      A: Allocator + Clone
{
    fn collided(&self) -> bool {
        let fwd_ptr = match &self.curr { Some(v) => &raw const **v, None => self.nil as *const N };
        let bck_ptr = match &self.curr_rev { Some(v) => &raw const **v, None => self.nil as *const N };
        std::ptr::eq(fwd_ptr, bck_ptr)
    }
}

impl<'a, N, T: 'a, A> Iterator for ListIterator<'a, N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
      A: Allocator + Clone
{
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        self.curr.take().map(|v| {
            self.curr = match self.collided() {
                false => v.next(self.nil),
                true => None
            };
            v.value()
        })
    }
}

impl<'a, N, T: 'a, A> DoubleEndedIterator for ListIterator<'a, N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
      A: Allocator + Clone
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.curr_rev.take().map(|v| {
            self.curr_rev = match self.collided() {
                false => v.prev(self.nil),
                true => None
            };
            v.value()
        })
    }
}

pub struct ListIteratorMut<'a, N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
      A: Allocator + Clone
{
    nil: *mut N,
    curr: Option<&'a mut N>,
    curr_rev: Option<&'a mut N>,
    _type_marker: std::marker::PhantomData<T>,
    _alloc_marker: std::marker::PhantomData<A>
}

impl<'a, N, T: 'a, A> ListIteratorMut<'a, N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
      A: Allocator + Clone
{
    fn collided(&self) -> bool {
        let fwd_ptr = match &self.curr { Some(v) => &raw const **v, None => self.nil as *const N };
        let bck_ptr = match &self.curr_rev { Some(v) => &raw const **v, None => self.nil as *const N };
        std::ptr::eq(fwd_ptr, bck_ptr)
    }
}

impl<'a, N, T: 'a, A> Iterator for ListIteratorMut<'a, N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
      A: Allocator + Clone
{
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
        self.curr.take().map(|v| {
            self.curr = match self.collided() {
                false => unsafe { v.next_ptr(self.nil).map(|mut f| f.as_mut()) },
                true => None
            };
            v.value_mut()
        })
    }
}

impl<'a, N, T: 'a, A> DoubleEndedIterator for ListIteratorMut<'a, N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
      A: Allocator + Clone
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.curr_rev.take().map(|v| {
            self.curr_rev = match self.collided() {
                false => unsafe { v.next_ptr(self.nil).map(|mut f| f.as_mut()) }
                true => None
            };
            v.value_mut()
        })
    }
}

impl<N, T, A> Drop for List<N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
      A: Allocator + Clone
{
    fn drop(&mut self) {
        let mut curr_node = self.first();
        while let Some(v) = curr_node {
            curr_node = v.next(self.head); 
            unsafe {
                std::ptr::drop_in_place(&raw const *v.value() as *mut T);
                std::ptr::drop_in_place(&raw const *v as *mut N) 
            };
        }
        unsafe { std::ptr::drop_in_place(self.head) };
    }
}

impl<N, T, A> Index<usize> for List<N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
      A: Allocator + Clone
{
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        self.get_unchecked(index).value()
    }
}

impl<N, T, A> IndexMut<usize> for List<N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
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
    val: ManuallyDrop<T>,
    _allocator: A
}

#[repr(C)]
pub struct ListForwardNode<T, A = Global>
where A: Allocator
{
    // next: Option<NonNull<Self>>,
    next: *mut Self,
    val: ManuallyDrop<T>,
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
    fn set_prev(&mut self, prev: *mut Self, nil: *mut Self) where Self: Sized;
}

impl<T, A> ListSingleNode<T, A> for ListNode<T, A>
where A: Allocator + Clone
{
    fn new(val: T, alloc: A, nil: *mut Self) -> *mut Self where Self: Sized {
        let new = alloc.allocate(Layout::new::<Self>()).unwrap().as_ptr() as *mut Self;
        let new_edit: &mut Self = unsafe { &mut *new };
        new_edit.next = nil;
        new_edit.prev = nil; 
        unsafe { std::ptr::write(&raw mut new_edit.val, ManuallyDrop::new(val)) }
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
            unsafe { (&mut *next).prev = self }
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
    fn set_prev(&mut self, prev: *mut Self, nil: *mut Self) where Self: Sized {
        if prev != nil {
            unsafe { (&mut *prev).next = self }
        }
        self.prev = prev;
    }
}

impl<N, T, A> Display for List<N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
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
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
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
    use allocator_api2::alloc::{ Allocator, Global };
    use crate::msvc::string::String as CppString;
    use super::{ List, ListNode, ListDoubleNode, ListSingleNode };

    use std::{
        fmt::{ Debug, Display },
        error::Error
    };
    type TestReturn = Result<(), Box<dyn Error>>;

impl<N, T, A> List<N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
      T: Display + PartialEq,
      A: Allocator + Clone
{
    pub(crate) fn check_list_iterator(&self, values: &[T]) {
        for (i, v) in self.iter().enumerate() {
            assert!(values[i] == *v, 
                "Index {} should have item {} instead of {}", 
                i, values[i], *v);
        }
    }

    pub(crate) fn check_list_iterator_reverse(&self, values: &[T]) {
        for (i, v) in self.iter().rev().enumerate() {
            assert!(values[i] == *v, 
                "Index {} should have item {} instead of {}",
                i, values[i], *v);
        }
    }
}

impl<N, T, A> List<N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
      T: Debug + PartialEq,
      A: Allocator + Clone
{
    pub(crate) fn check_list_iterator_debug(&self, values: &[T]) {
        for (i, v) in self.iter().enumerate() {
            assert!(values[i] == *v, 
                "Index {} should have item {:?} instead of {:?}", 
                i, values[i], *v);
        }
    }

    pub(crate) fn check_list_iterator_reverse_debug(&self, values: &[T]) {
        for (i, v) in self.iter().rev().enumerate() {
            assert!(values[i] == *v, 
                "Index {} should have item {:?} instead of {:?}",
                i, values[i], *v);
        }
    }
}

impl<N, T, A> List<N, T, A>
where N: ListSingleNode<T, A> + ListDoubleNode<T, A>,
      T: Debug,
      A: Allocator + Clone
{
    pub(crate) fn check_list_iterator_delegate<F, V>(&self, cb: F, expected: &[V])
    where F: Fn(&T, &V) -> bool,
          V: Debug
    {
        for (i, v) in self.iter().enumerate() {
            assert!(cb(v, &expected[i]), "Index {} should contain element {:?} instead of {:?}",
            i, expected[i], *v);
        }
    }
}

    #[test]
    pub fn create_blank_list() -> TestReturn {
        let list: List<ListNode<u32, Global>, u32, Global> = List::new();
        assert!(list.len() == 0, "List should be blank");
        assert!(list.first().is_none(), "First element should be blank");
        Ok(())
    }

    #[test]
    pub fn list_push_pop() -> TestReturn {
        let mut list = List::new();
        list.push(1);
        list.push(2);
        list.push(3);
        for i in 0..list.len() {
            // println!("{}: {}", i, list[i]);
            assert!(list[i] == i + 1, "Element at {} should have item {} instead of {}", i, i + 1, list[i]);
        }
        assert!(list.pop() == Some(3), "Element should be 3");
        assert!(list.pop() == Some(2), "Element should be 2");
        assert!(list.pop() == Some(1), "Element should be 1");
        assert!(list.pop() == None, "List should be empty");
        Ok(())
    }

    #[test]
    pub fn list_iterator() -> TestReturn {
        // Note that impl Iterator is only implemented for single ended iterators, since we don't
        // have a double ended linked list, so check backwards traversal on list_insertion
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
        list.insert(1, 2);
        assert!(list[1] == 2, "Second list entry should be 2");
        list.insert(3, 4);
        assert!(list[3] == 4, "Fourth list entry should be 4");
        let five = &raw mut *list.get_unchecked_mut(5);
        let seven = &raw mut *list.get_unchecked_mut(6);
        unsafe { list.insert_at_unchecked(five, 6); }
        unsafe { list.insert_at_unchecked(seven, 8); }
        assert!(list.len() == 9, "List length should be 9, got {} instead", list.len());
        assert!(list[5] == 6, "Sixth list entry should be 7");
        assert!(list[7] == 8, "Eighth list entry should be 8");
        for i in &list {
            assert!(list[*i-1] == *i, "Element {} should equal {} instead of {}", *i-1, *i, list[*i-1]);
        }
        let expected_reverse = [9, 8, 7, 6, 5, 4, 3, 2, 1];
        for (i, v) in list.iter().rev().enumerate() { 
            assert!(expected_reverse[i] == *v, "Element {} in reverse iterator should be {} instead of {}",
            i, expected_reverse[i], *v);
        }
        Ok(())
    }

    #[test]
    pub fn list_removal() -> TestReturn {
        let mut list = List::new();
        for i in 0..10 { list.push(i * 2) }
        assert!(list.len() == 10, "List length should be to, got {} instead", list.len());
        // delete at start
        assert!(list.remove(0) == 0, "The first element removed should be 0");
        // delete at end
        assert!(list.remove(8) == 18, "The last element removed should be 18");
        // delete inside
        assert!(list.remove(3) == 8, "The third element removed should be 8");
        assert!(list.len() == 7, "List length should be to, got {} instead", list.len());
        let expected = [2, 4, 6, 10, 12, 14, 16];
        for (i, v) in list.iter().enumerate() {
            assert!(*v == expected[i], "Element {} should be {} instead of {}", i, expected[i], *v);
        }
        // check backwards
        let expected_rev: Vec<i32> = expected.into_iter().rev().collect();
        for (i, v) in list.iter().rev().enumerate() {
            assert!(*v == expected_rev[i], "Element {} in reverse iterator should be {} instead of {}",
            i, expected_rev[i], *v);
        }
        Ok(())
    }

    #[test]
    pub fn list_move_by_index() -> TestReturn {
        let mut list = List::new();
        for i in 0..10 { list.push(i * 2) }
        // from beginning to end
        list.move_node(0, list.len());
        list.check_list_iterator(&[2, 4, 6, 8, 10, 12, 14, 16, 18, 0]);
        list.check_list_iterator_reverse(&[0, 18, 16, 14, 12, 10, 8, 6, 4, 2]);
        // from end to beginning
        list.move_node(9, 0);
        list.check_list_iterator(&[0, 2, 4, 6, 8, 10, 12, 14, 16, 18]);
        list.check_list_iterator_reverse(&[18, 16, 14, 12, 10, 8, 6, 4, 2, 0]);
        // from beginning into middle
        list.move_node(0, 2);
        list.check_list_iterator(&[2, 4, 0, 6, 8, 10, 12, 14, 16, 18]);
        list.check_list_iterator_reverse(&[18, 16, 14, 12, 10, 8, 6, 0, 4, 2]);
        // from end to middle
        list.move_node(9, 8);
        list.check_list_iterator(&[2, 4, 0, 6, 8, 10, 12, 18, 14, 16]);
        list.check_list_iterator_reverse(&[16, 14, 18, 12, 10, 8, 6, 0, 4, 2]);

        let mut list2 =  List::new();
        for i in 0..5 { list2.push(i); }
        // move node to the same position (should not change!)
        list2.move_node(2, 2);
        list2.check_list_iterator(&[0, 1, 2, 3, 4]);
        list2.check_list_iterator_reverse(&[4, 3, 2, 1, 0]);
        Ok(())
    }

    #[test]
    pub fn list_move_by_pointer() -> TestReturn {
        let mut list = List::new();
        for i in 0..10 { list.push(i * 2) }
        let first = &raw mut *list.get_unchecked_mut(0);
        let middle = &raw mut *list.get_unchecked_mut(5);
        let last = &raw mut* list.get_unchecked_mut(9);
        // move middle to front
        unsafe { list.move_node_before_unchecked(&mut *middle, &mut *first); }
        list.check_list_iterator(&[10, 0, 2, 4, 6, 8, 12, 14, 16, 18]);
        unsafe { list.move_node_after_unchecked(&mut *middle, &mut *last); }
        list.check_list_iterator(&[0, 2, 4, 6, 8, 12, 14, 16, 18, 10]);
        Ok(())
    }
/*
    #[test]
    pub fn list_node() -> TestReturn {
        let mut list = List::new();
        let newnode = unsafe { &mut *ListNode::new(5, Global, list.head) };
        list.link_first(newnode);
        for i in &list { println!("{}", *i); }
        Ok(())
    }
*/
    #[test]
    pub fn cpp_string_list() -> TestReturn {
        let mut list: List<ListNode<CppString, Global>, CppString, Global> = List::new_in(Global);
        list.push(CppString::from_str_in("Player", Global));
        for i in 0..4 { list.push(CppString::from_str_in(&format!("Enemy{}", i), Global)); }
        for i in 0..2 { list.push(CppString::from_str_in(&format!("Door{}", i), Global)); }
        let expected_strs = ["Player", "Enemy0", "Enemy1", "Enemy2", "Enemy3", "Door0", "Door1"];
        list.check_list_iterator_delegate(|v, e| { let s: &str = v.into(); s == *e }, &expected_strs);
        list[0] = CppString::from_str_in("Ally", Global);
        list[5] = CppString::from_str_in("Gimmick", Global);
        list[6] = CppString::from_str_in("Entrance", Global);
        let expected_strs = ["Ally", "Enemy0", "Enemy1", "Enemy2", "Enemy3", "Gimmick", "Entrance"];
        list.check_list_iterator_delegate(|v, e| { let s: &str = v.into(); s == *e }, &expected_strs);
        Ok(())
    }
}
