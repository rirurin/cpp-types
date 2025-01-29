#![allow(dead_code, unused_imports)]
use allocator_api2::alloc::{ Allocator, Global };
use crate::msvc::{
    hash::{ HasherInit, FNV1A },
    list::{ 
        List, ListDoubleNode, ListSingleNode, 
        ListNode, ListIterator, ListIteratorMut 
    },
    vector::Vector
};
use std::{
    fmt::{ Debug, Display },
    hash::{ Hash, Hasher },
    marker::PhantomData,
    mem::ManuallyDrop,
    ops::{ Index, IndexMut }
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

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AttachType { Before, After, Empty }

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
        buckets.resize(MIN_BUCKET_COUNT << 1);
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

    fn get_max_load(&self) -> f32 { self._traits_obj.0 }
    fn get_load_factor(&self) -> f32 { self.list.len() as f32 / self.max_index as f32 }

    pub fn bucket_count(&self) -> usize { self.max_index }
    fn max_bucket_count(&self) -> usize { (isize::MAX >> 1) as usize }

    fn find_node_by_key(&self, key: &T1) -> Option<*mut ListNode<T0, A>> {
        let bucket = (H::get_hash(key) as usize) & self.mask;
        let mut curr = self.buckets[bucket << 1];
        loop {
            if unsafe { (&*curr).value() == key } { return Some(curr) }
            curr = match unsafe { (&*curr).next(self.list.get_nil()) } {
                Some(v) => &raw const *v as *mut ListNode<T0, A>,
                None => return None
            };
        }
    }
    fn find_node_by_value(&self, value: &T0) -> Option<*mut ListNode<T0, A>> {
        let bucket = (H::get_hash(value) as usize) & self.mask;
        let mut curr = self.buckets[bucket << 1];
        loop {
            if unsafe { (&*curr).value() == value } { return Some(curr) }
            curr = match unsafe { (&*curr).next(self.list.get_nil()) } {
                Some(v) => &raw const *v as *mut ListNode<T0, A>,
                None => return None
            };
        }
    }

    fn find(&self, key: &T1) -> Option<&T0> {
        self.find_node_by_key(key).map(|n| unsafe { (&*n).value() })
    }
    fn find_mut(&mut self, key: &T1) -> Option<&mut T0> {
        self.find_node_by_key(key).map(|n| unsafe { (&mut *n).value_mut() })
    }
    fn contains(&self, key: &T1) -> bool { self.find_node_by_key(key).is_some() }

    fn iter(&self) -> ListIterator<'_, ListNode<T0, A>, T0, A> { self.into_iter() }
    fn iter_mut(&mut self) -> ListIteratorMut<'_, ListNode<T0, A>, T0, A> { self.into_iter() }
}
impl<H, T0, T1, A> HashTable<H, T0, T1, A>
where H: Hasher + HasherInit,
      T0: Hash + PartialEq<T1> + PartialEq, 
      T1: Hash,
      A: Allocator + Clone
{
    fn get_first_node_for_bucket(&self, value: &T0, bucket: usize) -> Result<Option<*mut ListNode<T0, A>>, ()> {
        let mut curr = self.buckets[(bucket << 1) + 1];
        let first = self.buckets[bucket << 1];
        // bucket has no entries, so this is allowed, but no node exists
        if curr == self.list.get_nil() { return Ok(None) }
        loop {
            // don't allow this, duplicate entries aren't allowed
            if unsafe { (&*curr).value() == value } { return Err(()) }
            unsafe { if std::ptr::eq(curr, first) { return Ok(Some(&raw const *curr as *mut ListNode<T0, A>))} }
            curr = match unsafe { (&*curr).prev(self.list.get_nil()) } {
                // bucket has an entry and a node to attach onto
                Some(v) => &raw const *v as *mut ListNode<T0, A>,
                // we reached the beginning of the linked list, stop here
                None => return Ok(Some(curr))
            };
        }
    }

    pub fn insert(&mut self, value: T0) -> bool {
        // if (self.list.len() as isize) < 0 {
        //     panic!("unordered_map/set too long");
        // }
        // _Hashval = _Traitsobj(_Keyval)
        let required_buckets = (self.max_index as f32 * self.get_max_load()) as usize;
        if self.list.len() >= required_buckets {
            let new_bucket_count = if self.max_index < 512 { self.max_index * 8 } else { self.max_index * 2 };
            self.resize(new_bucket_count);
        }
        let bucket = H::get_hash(&value) as usize & self.mask;
        let newnode = unsafe { &mut *ListNode::new(value, self._allocator.clone(), self.list.get_nil()) };
        self.add_to_bucket_list(newnode, bucket, true)
    }
    fn add_to_bucket_list(&mut self, node: &mut ListNode<T0, A>, bucket: usize, not_in_list: bool) -> bool {
        // attach the new node into the linked list
        let (mode, attach_node) = match self.get_first_node_for_bucket(node.value(), bucket) {
            Ok(v) => match v {
                Some(v) => (AttachType::Before, Some(v)),
                // if there's no bucket, attach to the end of the linked list
                None => match self.list.last_mut() {
                    Some(v) => (AttachType::After, Some(&raw mut *v)),
                    None => (AttachType::Empty, None)
                }
            }
            Err(_) => return false
        };
        if not_in_list {
            // manually set pointers for bucket list
            match mode {
                AttachType::Before => unsafe { self.list.link_before(&mut *attach_node.unwrap(), node) },
                AttachType::After => unsafe { self.list.link_after(&mut *attach_node.unwrap(), node) },
                AttachType::Empty => self.list.link_first(node),
            };
            unsafe { self.list.set_len(self.list.len() + 1); }
        }
        if mode == AttachType::After || mode == AttachType::Empty {
            self.buckets[(bucket << 1) + 1] = node;
        }
        self.buckets[bucket << 1] = node;
        true

    }
    pub fn resize(&mut self, newsize: usize) {
        self.max_index = newsize;
        self.mask = newsize - 1;
        self.buckets.resize(newsize << 1);
        for i in 0..(newsize << 1) {
            unsafe { std::ptr::write(self.buckets.as_mut_ptr().add(i), self.list.get_nil()); }
        }
        unsafe { self.buckets.set_len(newsize << 1); }
        // force borrow to split
        let mut curr = self.list.first_mut().map(|f| unsafe { &mut *(&raw mut *f) });
        while let Some(n) = curr {
            let bucket = H::get_hash(n.value()) as usize & self.mask;
            self.add_to_bucket_list(n, bucket, false);
            curr = n.next_mut(self.list.get_nil());
        }
    }

    pub fn clear(&mut self) {
        self.list.clear();
        for i in 0..(self.max_index << 1) {
            unsafe { std::ptr::write(self.buckets.as_mut_ptr().add(i), self.list.get_nil()); }
        }
    }
}

// For debugging
impl<H, T0, T1, A> HashTable<H, T0, T1, A>
where H: Hasher + HasherInit,
      T0: Hash + PartialEq<T1> + PartialEq, 
      T1: Hash,
      A: Allocator + Clone
{
    pub(super) fn get_bucket_mask(&self) -> usize { self.mask }
    pub(super) unsafe fn get_bucket_first(&self, bucket: usize) -> Option<&T0> {
        let bp = self.buckets[bucket << 1];
        if bp == self.list.get_nil() { return None }
        Some((&mut *bp).value())
    }
    pub(super) unsafe fn get_bucket_node_first(&self, bucket: usize) -> Option<&ListNode<T0, A>> {
        let bp = self.buckets[bucket << 1];
        if bp == self.list.get_nil() { return None }
        Some(&mut *bp)
    }
    pub(super) unsafe fn get_bucket_last(&self, bucket: usize) -> Option<&T0> {
        let bp = self.buckets[(bucket << 1) + 1];
        if bp == self.list.get_nil() { return None }
        Some((&mut *bp).value())
    }
    pub(super) unsafe fn get_bucket_node_last(&self, bucket: usize) -> Option<&ListNode<T0, A>> {
        let bp = self.buckets[(bucket << 1) + 1];
        if bp == self.list.get_nil() { return None }
        Some(&mut *bp)
    }
    pub(super) unsafe fn get_count_in_bucket(&self, bucket: usize) -> usize {
        let mut count = 0;
        let mut curr = if self.buckets[bucket << 1] == self.list.get_nil() {
            return count } else { Some(&*self.buckets[bucket << 1]) };
        while let Some(node) = curr {
            count += 1;
            curr = node.next(self.list.get_nil());
        }
        count
    }
    pub(super) fn get_hasher(&self) -> H { H::new() }
}

impl<'a, H, T0, T1, A> IntoIterator for &'a HashTable<H, T0, T1, A>
where H: Hasher + HasherInit,
      T0: Hash + PartialEq<T1> + PartialEq, 
      T1: Hash,
      A: Allocator + Clone
{
    type Item = &'a T0;
    type IntoIter = ListIterator<'a, ListNode<T0, A>, T0, A>;
    fn into_iter(self) -> Self::IntoIter { self.list.iter() }
}

impl<'a, H, T0, T1, A> IntoIterator for &'a mut HashTable<H, T0, T1, A>
where H: Hasher + HasherInit,
      T0: Hash + PartialEq<T1> + PartialEq, 
      T1: Hash,
      A: Allocator + Clone
{
    type Item = &'a mut T0;
    type IntoIter = ListIteratorMut<'a, ListNode<T0, A>, T0, A>;
    fn into_iter(self) -> Self::IntoIter { self.list.iter_mut() }
}

impl<'a, H, T0: 'a, T1, A> Index<&T1> for HashTable<H, T0, T1, A>
where H: Hasher + HasherInit,
      T0: Hash + PartialEq<T1> + PartialEq, 
      T1: Hash,
      A: Allocator + Clone
{
    type Output = T0;
    fn index(&self, index: &T1) -> &Self::Output { self.find(index).unwrap() }
}

impl<'a, H, T0: 'a, T1, A> IndexMut<&T1> for HashTable<H, T0, T1, A>
where H: Hasher + HasherInit,
      T0: Hash + PartialEq<T1> + PartialEq, 
      T1: Hash,
      A: Allocator + Clone
{
    fn index_mut(&mut self, index: &T1) -> &mut Self::Output { self.find_mut(index).unwrap() }
}

// ========================================================

// https://en.cppreference.com/w/cpp/container/unordered_set
#[repr(C)]
pub struct Set<H, T, A = Global>
where H: Hasher + HasherInit,
      T: Hash + PartialEq, 
      A: Allocator + Clone
{ _impl: HashTable<H, T, T, A> }

// C++ API
impl<T> Set<FNV1A, T, Global>
where T: Hash + PartialEq
{
    pub fn new() -> Self { Self::new_in(Global) }
}

impl<H, T, A> Set<H, T, A>
where H: Hasher + HasherInit,
      T: Hash + PartialEq, 
      A: Allocator + Clone
{
    /// Constructs the unordered_set
    pub fn new_in(alloc: A) -> Self {
        Self { _impl: HashTable::new_inner(alloc) }
    }
    /// Checks if the container has no elements
    pub fn empty(&self) -> bool { self._impl.is_empty() }
    /// Returns the number of elements in the container,
    pub fn size(&self) -> usize { self._impl.len() }
    /// Returns the maximum number of elements the container is able to hold due to system 
    /// or library implementation limitations
    pub fn max_size(&self) -> usize { self._impl.buckets.max_size() }
    /// Erases all elements from the container. After this call, size() returns zero.
    pub fn clear(&mut self) { self._impl.clear() }
    /// Inserts an element into the container, if the container doesn't already contain an 
    /// element with an equivalent key. This returns a bool that notes if the insertion took place
    pub fn insert(&mut self, value: T) -> bool { self._impl.insert(value) }
}

impl<'a, H, T, A> IntoIterator for &'a Set<H, T, A>
where H: Hasher + HasherInit,
      T: Hash + PartialEq, 
      A: Allocator + Clone
{
    type Item = &'a T;
    type IntoIter = ListIterator<'a, ListNode<T, A>, T, A>;
    fn into_iter(self) -> Self::IntoIter { self._impl.iter() }
}

impl<'a, H, T, A> IntoIterator for &'a mut Set<H, T, A>
where H: Hasher + HasherInit,
      T: Hash + PartialEq, 
      A: Allocator + Clone
{
    type Item = &'a mut T;
    type IntoIter = ListIteratorMut<'a, ListNode<T, A>, T, A>;
    fn into_iter(self) -> Self::IntoIter { self._impl.iter_mut() }
}

impl<'a, H, T: 'a, A> Index<&T> for Set<H, T, A>
where H: Hasher + HasherInit,
      T: Hash + PartialEq, 
      A: Allocator + Clone
{
    type Output = T;
    fn index(&self, index: &T) -> &Self::Output { &self._impl[index] }
}

impl<'a, H, T: 'a, A> IndexMut<&T> for Set<H, T, A>
where H: Hasher + HasherInit,
      T: Hash + PartialEq, 
      A: Allocator + Clone
{
    fn index_mut(&mut self, index: &T) -> &mut Self::Output { &mut self._impl[index] }
}

// https://en.cppreference.com/w/cpp/container/unordered_map

#[repr(C)]
pub struct MapPair<K, V>
where K: PartialEq + Hash
{
    key: K,
    value: V
}

impl<K, V> MapPair<K, V>
where K: PartialEq + Hash
{
    pub fn new(key: K, value: V) -> Self {
        Self { key, value }
    }
    pub fn get_key(&self) -> &K { &self.key }
    pub fn get_value(&self) -> &V { &self.value }
    pub fn get_key_mut(&mut self) -> &mut K { &mut self.key }
    pub fn get_value_mut(&mut self) -> &mut V { &mut self.value }
}

impl<K, V> MapPair<K, V>
where K: PartialEq + Copy + Hash
{
    pub fn get_key_copy(&self) -> K { self.key }
}

impl<K, V> Hash for MapPair<K, V>
where K: PartialEq + Hash
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state)
    }
}

impl<K, V> PartialEq for MapPair<K, V>
where K: PartialEq + Hash
{
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<K, V> PartialEq<K> for MapPair<K, V>
where K: PartialEq + Hash
{
    fn eq(&self, other: &K) -> bool {
        self.key == *other
    }
}

#[repr(C)]
pub struct Map<H, K, V, A = Global>
where H: Hasher + HasherInit,
      K: PartialEq + Hash,
      A: Allocator + Clone
{ _impl: HashTable<H, MapPair<K, V>, K, A> }
// C++ API
impl<K, V> Map<FNV1A, K, V, Global>
where K: PartialEq + Hash
{
    pub fn new() -> Self { Self::new_in(Global) }
}

impl<H, K, V, A> Map<H, K, V, A>
where H: Hasher + HasherInit,
      K: PartialEq + Hash,
      A: Allocator + Clone
{
    /// Constructs the unordered_map
    pub fn new_in(alloc: A) -> Self {
        Self { _impl: HashTable::new_inner(alloc) }
    }
    /// Checks if the container has no elements
    pub fn empty(&self) -> bool { self._impl.is_empty() }
    /// Returns the number of elements in the container,
    pub fn size(&self) -> usize { self._impl.len() }
    /// Returns the maximum number of elements the container is able to hold due to system 
    /// or library implementation limitations
    pub fn max_size(&self) -> usize { self._impl.buckets.max_size() }
    /// Erases all elements from the container. After this call, size() returns zero.
    pub fn clear(&mut self) { self._impl.clear() }
    /// Inserts an element into the container, if the container doesn't already contain an 
    /// element with an equivalent key. This returns a bool that notes if the insertion took place
    pub fn insert(&mut self, key: K, value: V) -> bool {
        let pair = MapPair::new(key, value);
        self._impl.insert(pair) 
    }
    pub fn iter(&self) -> ListIterator<'_, ListNode<MapPair<K, V>, A>, MapPair<K, V>, A> { self.into_iter() }
    pub fn iter_mut(&mut self) -> ListIteratorMut<'_, ListNode<MapPair<K, V>, A>, MapPair<K, V>, A> { self.into_iter() }

    pub fn find(&self, key: &K) -> Option<&MapPair<K, V>> {
        self._impl.find(key)
    }
    pub fn find_mut(&mut self, key: &K) -> Option<&mut MapPair<K, V>> {
        self._impl.find_mut(key)
    }
    pub fn contains(&self, key: &K) -> bool { self._impl.contains(key) }
}

impl<'a, H, K, V, A> IntoIterator for &'a Map<H, K, V, A>
where H: Hasher + HasherInit,
      K: PartialEq + Hash,
      A: Allocator + Clone
{
    type Item = &'a MapPair<K, V>;
    type IntoIter = ListIterator<'a, ListNode<MapPair<K, V>, A>, MapPair<K, V>, A>;
    fn into_iter(self) -> Self::IntoIter { self._impl.iter() }
}

impl<'a, H, K, V, A> IntoIterator for &'a mut Map<H, K, V, A>
where H: Hasher + HasherInit,
      K: PartialEq + Hash,
      A: Allocator + Clone
{
    type Item = &'a mut MapPair<K, V>;
    type IntoIter = ListIteratorMut<'a, ListNode<MapPair<K, V>, A>, MapPair<K, V>, A>;
    fn into_iter(self) -> Self::IntoIter { self._impl.iter_mut() }
}

impl<'a, H, K, V, A> Index<&K> for Map<H, K, V, A>
where H: Hasher + HasherInit,
      K: PartialEq + Hash,
      A: Allocator + Clone
{
    type Output = MapPair<K, V>;
    fn index(&self, index: &K) -> &Self::Output { &self._impl[index] }
}

impl<'a, H, K, V, A> IndexMut<&K> for Map<H, K, V, A>
where H: Hasher + HasherInit,
      K: PartialEq + Hash,
      A: Allocator + Clone
{
    fn index_mut(&mut self, index: &K) -> &mut Self::Output { &mut self._impl[index] }
}

#[cfg(test)]
pub mod tests {
    use allocator_api2::alloc::{ Allocator, Global };
    use crate::msvc::{
        hash::FNV1A,
        list::{ List, ListSingleNode, ListDoubleNode, ListNode },
        string::String
    };
    use std::{
        fmt::{ Debug, Display },
        hash::{ Hash, Hasher }
    };
    use super::{ HasherInit, HashTable };
    use std::error::Error;
    type TestReturn = Result<(), Box<dyn Error>>;

impl HashTable<FNV1A, String<u8, Global>, String<u8, Global>, Global>
{
    unsafe fn check_string_hash_table_insertion(&mut self, new: &str, tgt_bucket: usize) {
        let cpp_str = String::from_str_in(new, Global);
        let test_bucket = FNV1A::get_hash(&cpp_str) as usize & self.get_bucket_mask();
        assert!(tgt_bucket == test_bucket, "Calculated bucket index should be {} instead of {}", tgt_bucket, test_bucket);
        self.insert(cpp_str);
        assert!(self.get_bucket_first(tgt_bucket).is_some(), "Bucket list beginning should not be nil");
        assert!(self.get_bucket_last(tgt_bucket).is_some(), "Bucket list beginning should not be nil");
        let new_str: &str = self.get_bucket_first(tgt_bucket).unwrap().into();
        assert!(new_str == new, "Expected latest entry in bucket to be \"{}\", got {} instead", new, new_str);
    }

    unsafe fn check_string_hash_table_existing(&mut self, item: &str, check_bucket: usize) {
        let cpp_str = String::from_str_in(item, Global);
        match self.find_node_by_key(&cpp_str) {
            Some(v) => {
                let mut node = self.get_bucket_node_last(check_bucket);
                while let Some(n) = node {
                    if n.value() == (&mut *v).value() { return; }
                    node = n.prev(self.list.get_nil());
                }
                panic!("String \"{}\" was saved into the wrong bucket", item);
            },
            None => panic!("Couldn't find node in list with string \"{}\"", item)
        };
    }
}

    #[test]
    pub fn create_blank_hash_table() -> TestReturn {
        let new: HashTable<FNV1A, u32, u32, Global> = HashTable::new_inner(Global);
        assert!(new.len() == 0, "New hash table should be empty");
        assert!(new.bucket_count() == 8, "New hash table should have 8 buckets allocated");
        assert!(new.is_empty(), "New hash table should be reporting as empty");
        assert!(new.get_bucket_mask() == 7, "Mask field for new table should be 7");
        assert!(new.bucket_count() == 8, "Bucket count for new table should be 8");
        Ok(())
    }

    #[test]
    pub fn insert_into_hash_table() -> TestReturn {
        let mut new: HashTable<FNV1A, String<u8, Global>, String<u8, Global>, Global> = HashTable::new_inner(Global);
        // Insert "Player" CppString into new with bucket size 8
        // Hash: 0x333DC56DDFFD8EA0. Hash & 7 == 0
        unsafe { new.check_string_hash_table_insertion("Player", 0); }
        // Insert "Enemy0" CppString into new with bucket size 8
        // Hash: 0xE24F0CA51E957E61. Hash & 7 == 1
        unsafe { new.check_string_hash_table_insertion("Enemy0", 1); }
        // Insert "Enemy1" CppString into new with bucket size 8
        // Hash: 0xE24F0BA51E957CAE. Hash & 7 == 6
        unsafe { new.check_string_hash_table_insertion("Enemy1", 6); }
        // Insert "Enemy2" CppString into new with bucket size 8
        // Hash: 0xE24F0AA51E957AFB. Hash & 7 == 3
        unsafe { new.check_string_hash_table_insertion("Enemy2", 3); }
        // Insert "Enemy3" CppString into new with bucket size 8
        // Hash: 0xE24F09A51E957948. Hash & 7 == 0
        unsafe { new.check_string_hash_table_insertion("Enemy3", 0); }
        // Insert "Enemy4" CppString into new with bucket size 8
        // Hash: 0xE24F10A51E95852D. Hash & 7 == 5
        unsafe { new.check_string_hash_table_insertion("Enemy4", 5); }
        // Insert "Chest" CppString into new with bucket size 8
        // Hash: 0x4295BDDCA90BEC76. Hash & 7 == 6
        unsafe { new.check_string_hash_table_insertion("Chest", 6); }
        // Insert "Door" CppString into new with bucket size 8
        // Hash: 0x37CF773608CE6C9. Hash & 7 == 1
        unsafe { new.check_string_hash_table_insertion("Door", 1); }
        new.list.check_list_iterator_delegate(|e, v| {
            let e_str: &str = e.into();
            *v == e_str
        }, &["Enemy3", "Player", "Door", "Enemy0", "Chest", "Enemy1", "Enemy2", "Enemy4"]);
        // Adding a new value will expand the bucket list, requiring a rehash of every element.
        // Rehash "Player", Hash: 0x333DC56DDFFD8EA0. Hash & 0x3f == 0x20
        // Rehash "Enemy0", Hash: 0xE24F0CA51E957E61. Hash & 0x3f == 0x21
        // Rehash "Enemy1", Hash: 0xE24F0BA51E957CAE. Hash & 0x3f == 0x2e
        // Rehash "Enemy2", Hash: 0xE24F0AA51E957AFB. Hash & 0x3f == 0x3b
        // Rehash "Enemy3", Hash: 0xE24F09A51E957948. Hash & 0x3f == 0x08
        // Rehash "Enemy4", Hash: 0xE24F10A51E95852D. Hash & 0x3f == 0x2d
        // Rehash "Chest", Hash: 0x4295BDDCA90BEC76. Hash & 0x3f == 0x36
        // Rehash "Door", Hash: 0x37CF773608CE6C9. Hash & 0x3f == 0x09
        //
        // Insert "Door2" CppString into new with bucket size 64
        // Hash: 0x7A3F740D0F6C7C81. Hash & 0x3f == 0x01
        unsafe { new.check_string_hash_table_insertion("Door2", 1); }
        unsafe { new.check_string_hash_table_existing("Player", 0x20); }
        unsafe { new.check_string_hash_table_existing("Enemy0", 0x21); }
        unsafe { new.check_string_hash_table_existing("Enemy1", 0x2e); }
        unsafe { new.check_string_hash_table_existing("Enemy2", 0x3b); }
        unsafe { new.check_string_hash_table_existing("Enemy3", 0x08); }
        unsafe { new.check_string_hash_table_existing("Enemy4", 0x2d); }
        unsafe { new.check_string_hash_table_existing("Chest", 0x36); }
        unsafe { new.check_string_hash_table_existing("Door", 0x09); }
        new.list.check_list_iterator_delegate(|e, v| {
            let e_str: &str = e.into();
            *v == e_str
        }, &["Enemy3", "Player", "Door", "Enemy0", "Chest", "Enemy1", "Enemy2", "Enemy4", "Door2"]);
        Ok(())
    }

    #[test]
    pub fn find_in_hash_table() -> TestReturn {
        let mut new: HashTable<FNV1A, String<u8, Global>, String<u8, Global>, Global> = HashTable::new_inner(Global);
        new.insert(String::from_str("Player"));
        new.insert(String::from_str("Enemy0"));
        new.insert(String::from_str("Enemy1"));
        new.insert(String::from_str("Enemy2"));
        new.insert(String::from_str("Enemy3"));
        new.insert(String::from_str("Enemy4"));
        new.insert(String::from_str("Chest"));
        new.insert(String::from_str("Door"));
        new.insert(String::from_str("Door2"));
        assert!(new.contains(&String::from_str("Enemy2")), "Hash table should contain a string entry for Enemy2");
        assert!(!new.contains(&String::from_str("Ally")), "Hash table should not contain a string entry for Ally");
        let chest_str: &str = new.find(&String::from_str("Chest")).unwrap().into();
        assert!(chest_str == "Chest", "Couldn't find the hash table entry for Chest");
        assert!(new.find(&String::from_str("Gimmick")).is_none(), "Hash table should not have found an entry for Gimmick");
        let door_str: &str = (&new[&String::from_str("Door")]).into();
        assert!(door_str == "Door", "Couldn't find the hash table entry for Door");
        Ok(())
    }
}
