#![allow(dead_code, unused_imports)]
use allocator_api2::alloc::{ Allocator, Global };
use crate::msvc::{
    hash::HasherInit,
    list::{ List, ListSingleNode, ListNode },
    vector::Vector
};
use std::{
    hash::{ Hash, Hasher },
    marker::PhantomData
};
// See https://devblogs.microsoft.com/oldnewthing/20230807-00/?p=108562
// https://github.com/microsoft/STL/blob/main/stl/inc/xhash

const MIN_BUCKET_COUNT: usize = 8;

#[repr(C)]
#[derive(Debug)]
pub struct KeyEqual(f32); // load factor

#[repr(C)]
pub struct HashTable<H, T0, T1, A = Global>
where H: Hasher + HasherInit,
      T0: Hash + PartialEq<T1> + PartialEq, 
      T1: Hash,
      A: Allocator + Clone
{
    _traits_obj: KeyEqual, // (key_eq)
    list: List<ListNode<T0, A>, T0, A>, // list of elements
    buckets: Vector<*mut ListNode<T0, A>, A>,
    mask: usize,
    max_index: usize,
    _allocator: A,
    _hash: PhantomData<H>,
    _key_ty: PhantomData<T1>
}

impl<H, T0, T1, A> HashTable<H, T0, T1, A>
where H: Hasher + HasherInit,
      T0: Hash + PartialEq<T1> + PartialEq, 
      T1: Hash,
      A: Allocator + Clone
{
    pub fn len(&self) -> usize { self.list.len() }
    pub fn is_empty(&self) -> bool { self.list.is_empty() }

    pub fn new_inner(alloc: A) -> Self {
        assert!(std::mem::size_of::<A>() == 0, "Allocator must be zero-sized!");
        let list = List::new_in(alloc.clone());
        let mut buckets = Vector::new_in(alloc.clone());
        buckets.resize(MIN_BUCKET_COUNT * 2);
        for _ in 0..buckets.cap() { buckets.push(list.get_nil()) }
        Self {
            _traits_obj: KeyEqual(1.0),
            list,
            buckets,
            mask: MIN_BUCKET_COUNT - 1,
            max_index: MIN_BUCKET_COUNT,
            _allocator: alloc,
            _hash: PhantomData,
            _key_ty: PhantomData
        }
    }

    fn max_load(&self) -> f32 { self._traits_obj.0 }
    fn load_factor(&self) -> f32 { self.list.len() as f32 / self.max_index as f32 }

    pub fn bucket_count(&self) -> usize { self.max_index }
    fn max_bucket_count(&self) -> usize { (isize::MAX >> 1) as usize }

    pub fn find_node_by_key(&self, key: &T1) -> Option<*mut ListNode<T0, A>> {
        let bucket = H::get_hash(key) as usize;
        let mut curr = self.buckets[bucket << 1];
        loop {
            if unsafe { (&*curr).value() == key } { return Some(curr) }
            curr = match unsafe { (&*curr).next(self.list.get_nil()) } {
                Some(v) => &raw const *v as *mut ListNode<T0, A>,
                None => return None
            };
        }
    }
    pub fn find_node_by_value(&self, value: &T0) -> Option<*mut ListNode<T0, A>> {
        let bucket = H::get_hash(&value) as usize;
        let mut curr = self.buckets[bucket << 1];
        loop {
            if unsafe { (&*curr).value() == value } { return Some(curr) }
            curr = match unsafe { (&*curr).next(self.list.get_nil()) } {
                Some(v) => &raw const *v as *mut ListNode<T0, A>,
                None => return None
            };
        }
    }

    pub fn insert(&mut self, value: T0) -> bool {
        // if (self.list.len() as isize) < 0 {
        //     panic!("unordered_map/set too long");
        // }
        // _Hashval = _Traitsobj(_Keyval)
        if self.find_node_by_value(&value).is_some() { return false; }
        let hash_val = H::get_hash(&value);
        let bucket_index = hash_val as usize & self.mask;
        let newnode = ListNode::new(value, self._allocator.clone(), self.list.get_nil());
        // Duplicate entries are not allowed
        // this bucket contains at least one entry
        if self.buckets[bucket_index * 2 + 1] != self.list.get_nil() {

        } else {
            self.buckets[bucket_index * 2] = newnode;
        }
        self.buckets[bucket_index * 2 + 1] = newnode;
        true
    }
}

impl<H, T0, T1, A> HashTable<H, T0, T1, A>
where H: Hasher + HasherInit,
      T0: Hash + PartialEq<T1> + PartialEq, 
      T1: Hash,
      A: Allocator + Clone
{
    pub(super) fn internal_get_mask(&self) -> usize { self.mask }
}
#[repr(C)]
pub struct Set<H, T, A = Global>
where H: Hasher + HasherInit,
      T: Hash + PartialEq, 
      A: Allocator + Clone
{
    _impl: HashTable<H, T, T, A>
}

#[repr(C)]
pub struct Map<H, K, V, A = Global>
where H: Hasher + HasherInit,
      K: PartialEq + PartialOrd + Hash,
      A: Allocator + Clone
{
    _impl: HashTable<H, super::tree::MapPair<K, V>, K, A>
}

#[cfg(test)]
pub mod tests {
    use allocator_api2::alloc::{ Allocator, Global };
    use crate::msvc::hash::FNV1A;
    use super::HashTable;
    use std::error::Error;
    type TestReturn = Result<(), Box<dyn Error>>;

    #[test]
    pub fn create_blank_hash_table() -> TestReturn {
        let new: HashTable<FNV1A, u32, u32, Global> = HashTable::new_inner(Global);
        assert!(new.len() == 0, "New hash table should be empty");
        assert!(new.bucket_count() == 8, "New hash table should have 8 buckets allocated");
        assert!(new.is_empty(), "New hash table should be reporting as empty");
        assert!(new.internal_get_mask() == 7, "Mask field for new table should be 7");
        assert!(new.bucket_count() == 8, "Bucket count for new table should be 8");
        Ok(())
    }

    #[test]
    pub fn insert_into_hash_table() -> TestReturn {

        Ok(())
    }
} 
