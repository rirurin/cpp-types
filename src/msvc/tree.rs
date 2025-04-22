#![allow(dead_code, unused_imports)]
use allocator_api2::alloc::{ Allocator, Global };
use std::{
    alloc::Layout,
    fmt::{ Display, Debug },
    hash::{ Hash, Hasher },
    mem::{ align_of, size_of },
    marker::{ PhantomData, PhantomPinned }
};

// See https://devblogs.microsoft.com/oldnewthing/20230807-00/?p=108562
// https://github.com/microsoft/STL/blob/main/stl/inc/xtree

#[repr(C)]
pub struct Tree<C, T0, T1, A = Global>
where C: TreeCompare<T0, T1>,
      T0: PartialEq + PartialEq<T1> + PartialOrd + PartialOrd<T1>,
      // T1: PartialEq + PartialEq<T0> + PartialOrd + PartialOrd<T0>,
      T1: PartialEq + PartialOrd,
      A: Allocator + Clone
{
    head: *mut TreeNode<T0, A>,
    size: usize,
    _allocator: A,
    _comparison: PhantomData<C>,
    _key_ty: PhantomData<T1>
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeColor {
    Red = 0,
    Black
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NodeDirection {
    Left = 0,
    Right
}

#[repr(C)]
pub struct TreeNode<T, A = Global>
where T: PartialEq + PartialOrd,
      A: Allocator + Clone
{
    left: *mut TreeNode<T, A>,
    parent: *mut TreeNode<T, A>,
    right: *mut TreeNode<T, A>,
    color: NodeColor,
    nil: bool,
    data: T,
    _allocator: A
}

pub trait TreeCompare<A, B>
where A: PartialEq + PartialEq<B> + PartialOrd + PartialOrd<B>,
      // B: PartialEq<A> + PartialOrd<A>
{
    fn compare_aa(d0: &A, d1: &A) -> bool;
    fn compare_ab(d0: &A, d1: &B) -> bool;
    // fn compare_ba(d0: &B, d1: &A) -> bool;
}

pub struct CompareLess; // std::less
impl<A, B> TreeCompare<A, B> for CompareLess
where A: PartialEq + PartialEq<B> + PartialOrd + PartialOrd<B>,
      // B: PartialEq<A> + PartialOrd<A>
{
    fn compare_aa(d0: &A, d1: &A) -> bool { d0 < d1 }
    fn compare_ab(d0: &A, d1: &B) -> bool { d0 < d1 }
    // fn compare_ba(d0: &B, d1: &A) -> bool { d0 < d1 }
}

pub struct CompareGreater; // std::greater
impl<A, B> TreeCompare<A, B> for CompareGreater
where A: PartialEq + PartialEq<B> + PartialOrd + PartialOrd<B>,
      // B: PartialEq<A> + PartialOrd<A>
{
    fn compare_aa(d0: &A, d1: &A) -> bool { d0 > d1 }
    fn compare_ab(d0: &A, d1: &B) -> bool { d0 > d1 }
    // fn compare_ba(d0: &B, d1: &A) -> bool { d0 > d1 }
}

impl<C, T0, T1, A> Tree<C, T0, T1, A>
where C: TreeCompare<T0, T1>,
      T0: PartialEq + PartialEq<T1> + PartialOrd + PartialOrd<T1>,
      // T1: PartialEq + PartialEq<T0> + PartialOrd + PartialOrd<T0>, 
      T1: PartialEq + PartialOrd,
      A: Allocator + Clone
{
    pub fn new_inner(alloc: A) -> Self {
        let head = unsafe { TreeNode::new_head(alloc.clone()) };
        Self { head, size: 0, _allocator: alloc, _comparison: PhantomData, _key_ty: PhantomData }
    }
    pub fn len(&self) -> usize { self.size }
    pub fn is_empty(&self) -> bool { self.size == 0 }

    unsafe fn new_node(&self, value: T0) -> *mut TreeNode<T0, A> {
        let node = TreeNode::new_node(self._allocator.clone(), self.head); 
        std::ptr::write((&raw mut (&mut *node).data), value);
        node
    }
    // SAFETY: self.head always points to the leaf/sentinel node
    fn get_head(&self) -> &TreeNode<T0, A> { unsafe { &*self.head } }
    fn get_head_mut(&mut self) -> &mut TreeNode<T0, A> { unsafe { &mut *self.head } }

    fn get_root(&self) -> Option<&TreeNode<T0, A>> { self.get_head().get_parent() }
    fn get_root_mut(&mut self) -> Option<&mut TreeNode<T0, A>> { self.get_head_mut().get_parent_mut() }
    // SAFETY: At least one element must be added to the tree or these methods will panic
    unsafe fn get_root_unchecked(&self) -> &TreeNode<T0, A> { self.get_head().get_parent().unwrap() }
    unsafe fn get_root_unchecked_mut(&mut self) -> &mut TreeNode<T0, A> { self.get_head_mut().get_parent_mut().unwrap() }
    unsafe fn get_root_ptr(&self) -> *const TreeNode<T0, A> { self.get_head().parent }
    unsafe fn get_root_ptr_mut(&self) -> *mut TreeNode<T0, A> { self.get_head().parent }

    unsafe fn make_initial_insertion(&self, node: &mut TreeNode<T0, A>) -> Option<(&mut TreeNode<T0, A>, NodeDirection)> {
        let mut curr_node: *mut TreeNode<T0, A> = (&mut *self.head).parent;
        loop {
            // Duplicate entries are not allowed
            let node_ref = &mut *curr_node;
            if node.data == node_ref.data { return None; } 
            let dir = if C::compare_aa(&node.data, &node_ref.data) { NodeDirection::Left } else { NodeDirection::Right };
            let next = match dir {
                NodeDirection::Left => node_ref.get_left_mut(),
                NodeDirection::Right => node_ref.get_right_mut()
            };
            curr_node = match next {
                Some(v) => v,
                None => {
                    node.parent = curr_node;
                    return Some((node_ref, dir))
                }
            };
        }
    }

    //
    //      p           p
    //     /           /
    //    n           r
    //   / \    =>   / \
    //  x   r       n   y
    //     / \     / \
    //    o  y    x   o
    //
    // NOTE: Assume that r and p are valid.
    unsafe fn rotate_left(&mut self, n: *mut TreeNode<T0, A>) {
        let n = &mut *n;
        let p = n.parent;
        let r = &mut *n.right;
        let o = r.left;
        n.right = o;
        if !(&*o).nil { (&mut*o).parent = n }
        r.left = n;
        n.parent = r;
        r.parent = p;
        if std::ptr::eq(p, self.head) { 
            (&mut *self.head).parent = r 
        } else {
            match (&*p).right == n {
                true => (&mut*p).right = r,
                false => (&mut*p).left = r
            }
        }
    }

    //
    //        p           p
    //       /           /
    //      n           r
    //     / \    =>   / \
    //    r   y       x   n
    //   / \             / \
    //  x  o            o  y
    //
    // NOTE: Assume that r and p are valid.
    unsafe fn rotate_right(&mut self, n: *mut TreeNode<T0, A>) {
        let n = &mut *n;
        let p = n.parent;
        let r = &mut *n.left;
        let o = r.right;
        n.left = o;
        if !(&*o).nil { (&mut*o).parent = n }
        r.right = n;
        n.parent = r;
        r.parent = p;
        if std::ptr::eq(p, self.head) { 
            (&mut *self.head).parent = r 
        } else {
            match (&*p).right == n {
                true => (&mut*p).right = r,
                false => (&mut*p).left = r
            }
        }
    }
    // NOTE: Assume that n->parent is valid
    unsafe fn get_sibling(&self, n: *mut TreeNode<T0, A>) -> *mut TreeNode<T0, A> {
        let node = &*n;
        match node.get_parent().unwrap().left == n {
            true => (&mut *node.parent).right,
            false => (&mut *node.parent).left
        }
    }

    unsafe fn get_direction(&self, n: *mut TreeNode<T0, A>) -> NodeDirection {
        let node = &*n;
        match node.get_parent().unwrap().right == n {
            true => NodeDirection::Right,
            false => NodeDirection::Left
        }
    }
    // NOTE: Assume that grandparent (node->parent->parent) is valid when starting (only call this
    // if tree's height >= 2)
    unsafe fn post_insert_maintain_rbt(&mut self, n: *mut TreeNode<T0, A>) {
        let mut node = n;
        loop {
            // retuns if a parent exists and is red, since that violates rb-tree rules
            let mut parent = match (&mut *node).get_parent_mut() {
                Some(v) => {
                    if v.color == NodeColor::Black { return }
                    v
                },
                None => return
            };
            let grandparent = match parent.parent.as_mut() {
                Some(v) => v,
                None => return
            };
            let uncle = self.get_sibling(&raw mut *parent);
            let unc_dir = self.get_direction(&raw mut *parent);
            if !(&*uncle).nil && (&*uncle).color == NodeColor::Red {
                (&mut *parent).color = NodeColor::Black;
                assert!((&*node).color == NodeColor::Red, "Node should be red");
                (&mut *uncle).color = NodeColor::Black;
                (&mut *grandparent).color = NodeColor::Red;
                // travel up 2 tree levels
                node = grandparent;
            } else {
                let inner_grandchild = match unc_dir {
                    NodeDirection::Left => parent.right,
                    NodeDirection::Right => parent.left
                };
                if std::ptr::eq(inner_grandchild, node) {
                    parent = match unc_dir {
                        NodeDirection::Left => {
                            self.rotate_left(&raw mut *parent);
                            &mut *((&mut *grandparent).left)
                        },
                        NodeDirection::Right => {
                            self.rotate_right(&raw mut *parent);
                            &mut *((&mut *grandparent).right)
                        },
                    } 
                }
                match unc_dir {
                    NodeDirection::Left => self.rotate_right(&raw mut *grandparent),
                    NodeDirection::Right => self.rotate_left(&raw mut *grandparent),
                }
                parent.color = NodeColor::Black;
                grandparent.color = NodeColor::Red;
                return;
            }
        }
    }

    pub fn insert(&mut self, value: T0) -> bool {
        let count = self.len();
        self.size += 1;
        let node = unsafe { self.new_node(value) };
        let mut head = self.get_head_mut();
        if count == 0 {
            // We are the only node!
            head.left = node;
            head.parent = node;
            head.right = node;
            unsafe { (&mut *node).color = NodeColor::Black };
            return true;
        }
        // traverse BST, starting from root (head->parent), then add as leaf
        match unsafe { self.make_initial_insertion(&mut *node) } {
            Some((n, d)) => {
                match d {
                    NodeDirection::Left => n.left = node,
                    NodeDirection::Right => n.right = node
                };
            }
            // this fails if attempting to add a duplicate key
            None => return false
        };
        // msvc tree specific:
        // set head node left/right if smallest or largest value
        head = self.get_head_mut();
        unsafe {
            if C::compare_aa(&(&mut *node).data, &head.get_left().unwrap().data) {
                head.left = node;
            }
            if C::compare_aa(&head.get_left().unwrap().data, &(&mut *node).data) {
                head.right = node;
            }
        }
        // maintain red-black tree property
        let parent = unsafe { (&mut *node).get_parent_mut().unwrap() };
        match parent.get_parent_mut() { // has grandparent (height >= 2)
            Some(_) => unsafe {
                // do some cleaning up
                self.post_insert_maintain_rbt(node);
                self.get_root_unchecked_mut().color = NodeColor::Black;
            }, // height = 1, do nothing. rb-tree properties are maintained
            None => (),
        };
        true
    }

    pub fn contains(&self, value: T1) -> bool {
        let mut current = self.get_root();
        while let Some(n) = current {
            if n.data == value { return true; }
            if C::compare_ab(&n.data, &value) {
                current = n.get_right();
            } else {
                current = n.get_left();
            }
        }
        false
    }

    pub fn find(&self, value: T1) -> Option<&T0> {
        let mut current = self.get_root();
        while let Some(n) = current {
            if n.data == value { return Some(&n.data); }
            if C::compare_ab(&n.data, &value) {
                current = n.get_right();
            } else {
                current = n.get_left();
            }
        }
        None
    }

    pub fn find_mut(&mut self, value: T1) -> Option<&mut T0> {
        let mut current = self.get_root_mut();
        while let Some(n) = current {
            if n.data == value { return Some(&mut n.data); }
            if C::compare_ab(&n.data, &value) {
                current = n.get_right_mut();
            } else {
                current = n.get_left_mut();
            }
        }
        None
    }
    // remove type can be different from storage type (e.g for maps, store as MapNode(MapKey,
    // MapValue), but find based on MapKey which is PartialEq<MapNode>
    pub fn remove(&mut self, value: T1) -> bool
    {
        let target = match self.find_mut(value) {
            Some(v) => v,
            None => return false
        };
        self.size -= 1;
        true
    }

    fn traverse(&self) -> Vec<&TreeNode<T0, A>> {
        let mut entries = vec![];
        let mut stack: Vec<&TreeNode<T0, A>> = vec![];
        let mut current = match self.get_root() {
            Some(v) => v,
            None => return entries
        };
        loop {
            while !current.nil {
                stack.push(current);
                current = unsafe { &*current.left };
            }
            let out = match stack.pop() {
                Some(v) => {
                    entries.push(v);
                    v
                },
                None => break
            };
            current = unsafe { &*out.right };
        }
        entries
    }
}

impl<C, T0, T1, A> Tree<C, T0, T1, A>
where C: TreeCompare<T0, T1>,
      T0: PartialEq + PartialEq<T1> + PartialOrd + PartialOrd<T1> + Debug,
      // T1: PartialEq + PartialEq<T0> + PartialOrd + PartialOrd<T0>,
T1: PartialEq + PartialOrd,
      A: Allocator + Clone
{
    pub(super) fn traverse_test(&self) {
        let head = self.get_head();
        println!("Nil @ 0x{:x}: <l:0x{:x} p:0x{:x} r:0x{:x}>", 
        &raw const *head as usize, head.left as usize, head.parent as usize, head.right as usize);
        if !self.is_empty() {
            self.get_head().get_parent().unwrap().traverse_printf();
        }
    }

    pub(super) fn traverse_debug(&self) -> Vec<&TreeNode<T0, A>> {
        self.traverse()
    }

    pub fn insert_print(&mut self, value: T0) -> bool {
        println!("insert {:?}", value);
        self.insert(value)
    }
}

impl<T, A> TreeNode<T, A>
where T: PartialEq + PartialOrd,
      A: Allocator + Clone
{
    // _Tree_node::_Buyheadnode, for root node
    unsafe fn new_head(alloc: A) -> *mut Self {
        let new = &mut *(alloc.allocate(Layout::new::<Self>()).unwrap().as_ptr() as *mut Self);
        let p_new = &raw mut *new;
        new.left = p_new;
        new.parent = p_new;
        new.right = p_new;
        new.color = NodeColor::Black;
        new.nil = true;
        new._allocator = alloc.clone();
        p_new
    }
    // _Tree_node::_Buynode, for child nodes
    unsafe fn new_node(alloc: A, head: *mut Self) -> *mut Self {
        let new = &mut *(alloc.allocate(Layout::new::<Self>()).unwrap().as_ptr() as *mut Self);
        let p_new = &raw mut *new;
        new.left = head;
        new.parent = head;
        new.right = head;
        new.color = NodeColor::Red;
        new.nil = false;
        p_new
    }

    fn get_parent(&self) -> Option<&Self> { 
        let parent = unsafe { &*self.parent };
        match parent.nil {
            true => None,
            false => Some(parent)
        }
    }
    fn get_left(&self) -> Option<&Self> {
        let child = unsafe { &*self.left };
        match child.nil {
            true => None,
            false => Some(child)
        }
    }
    fn get_right(&self) -> Option<&Self> {
        let child = unsafe { &*self.right };
        match child.nil {
            true => None,
            false => Some(child)
        }
    }

    fn get_parent_mut(&mut self) -> Option<&mut Self> { 
        let parent = unsafe { &mut *self.parent };
        match parent.nil {
            true => None,
            false => Some(parent)
        }
    }
    fn get_left_mut(&mut self) -> Option<&mut Self> {
        let child = unsafe { &mut *self.left };
        match child.nil {
            true => None,
            false => Some(child)
        }
    }
    fn get_right_mut(&mut self) -> Option<&mut Self> {
        let child = unsafe { &mut *self.right };
        match child.nil {
            true => None,
            false => Some(child)
        }
    }
}

impl<C, T0, T1, A> Drop for Tree<C, T0, T1, A>
where C: TreeCompare<T0, T1>,
      T0: PartialEq + PartialEq<T1> + PartialOrd + PartialOrd<T1>,
      //T1: PartialEq + PartialEq<T0> + PartialOrd + PartialOrd<T0>,
      T1: PartialEq + PartialOrd,
      A: Allocator + Clone
{
    fn drop(&mut self) {
        let nodes: Vec<&TreeNode<T0, A>> = self.traverse();
        for n in nodes {
            // SAFETY: This is the last time that any tree node can be accessed
            unsafe { std::ptr::drop_in_place(&raw const *n as *mut TreeNode<T0, A>); }
        }
    }
}

impl<T, A> TreeNode<T, A>
where T: PartialEq + PartialOrd + Debug,
      A: Allocator + Clone
{
    fn traverse_printf(&self) {
        if let Some(l) = self.get_left() { l.traverse_printf() }
        let ptr = &raw const *self as usize;
        println!("Node @ 0x{:x}: <l:0x{:x} p:0x{:x} r:0x{:x}> [{:?}, {:?}]", 
        ptr, self.left as usize, self.parent as usize, self.right as usize, self.data, self.color);
        if let Some(r) = self.get_right() { r.traverse_printf() }
    }
}

impl<'a, C, T0, T1, A> IntoIterator for &'a Tree<C, T0, T1, A>
where C: TreeCompare<T0, T1>,
      T0: PartialEq + PartialEq<T1> + PartialOrd + PartialOrd<T1>,
      // T1: PartialEq + PartialEq<T0> + PartialOrd + PartialOrd<T0>,
    T1: PartialEq + PartialOrd,
      A: Allocator + Clone
{
    type Item = &'a T0;
    type IntoIter = TreeIterator<'a, T0, A>;
    fn into_iter(self) -> Self::IntoIter {
        // inorder traversal, so get leftmost node
        unsafe {
            let mut stack = vec![];
            let current = match (&*self.head).get_parent() {
                Some(v) => {
                    let mut c = v;
                    while !c.nil {
                        stack.push(c);
                        c = &*c.left;
                    }
                    Some(c)
                },
                None => None
            };
            Self::IntoIter {
                current,
                stack
            }
        }
    }
}

pub struct TreeIterator<'a, T, A>
where T: PartialEq + PartialOrd,
      A: Allocator + Clone
{
    current: Option<&'a TreeNode<T, A>>,
    stack: Vec<&'a TreeNode<T, A>>
}

impl<'a, T, A> Iterator for TreeIterator<'a, T, A>
where T: PartialEq + PartialOrd,
      A: Allocator + Clone
{
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        match self.current {
            Some(_) => {
                unsafe {
                    while !self.current.unwrap().nil {
                        self.stack.push(self.current.unwrap());
                        self.current = Some(&*self.current.unwrap().left);
                    }
                }
            },
            None => return None
        };
        let out = match self.stack.pop() {
            Some(v) => v,
            None => return None
        };
        self.current = Some(unsafe { &*out.right });
        Some(&out.data)
    }
}

#[repr(C)]
pub struct Set<C, T, A = Global>
where C: TreeCompare<T, T>,
      T: PartialEq + PartialOrd,
      A: Allocator + Clone
{
    _impl: Tree<C, T, T, A>
}

impl<C, T, A> Set<C, T, A>
where C: TreeCompare<T, T>,
      T: PartialEq + PartialOrd,
      A: Allocator + Clone
{
    pub fn new_inner(alloc: A) -> Self { Self { _impl: Tree::new_inner(alloc) }}
    pub fn insert(&mut self, value: T) -> bool { self._impl.insert(value) }
    pub fn len(&self) -> usize { self._impl.len() }
    pub fn is_empty(&self) -> bool { self._impl.is_empty() }
    pub fn contains(&self, value: T) -> bool { self._impl.contains(value) }
    pub fn find(&self, value: T) -> Option<&T> { self._impl.find(value) }
    pub fn find_mut(&mut self, value: T) -> Option<&mut T> { self._impl.find_mut(value) }
}

#[repr(C)]
pub struct MapPair<K, V>
where K: PartialEq + PartialOrd
{
    key: K,
    value: V
}

impl<K, V> MapPair<K, V>
where K: PartialEq + PartialOrd
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
where K: PartialEq + PartialOrd + Copy + Hash
{
    pub fn get_key_copy(&self) -> K { self.key }
}

impl<K, V> PartialEq for MapPair<K, V>
where K: PartialEq + PartialOrd + Hash
{
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<K, V> PartialEq<K> for MapPair<K, V>
where K: PartialEq + PartialOrd + Hash
{
    fn eq(&self, other: &K) -> bool {
        self.key == *other
    }
}

impl<K, V> PartialOrd for MapPair<K, V>
where K: PartialEq + PartialOrd + Hash
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.key.partial_cmp(&other.key)
    }
}

impl<K, V> PartialOrd<K> for MapPair<K, V>
where K: PartialEq + PartialOrd + Hash
{
    fn partial_cmp(&self, other: &K) -> Option<std::cmp::Ordering> {
        self.key.partial_cmp(other)
    }
}

#[repr(C)]
pub struct Map<C, K, V, A>
where C: TreeCompare<MapPair<K, V>, K>,
      K: PartialEq + PartialOrd + Hash,
      A: Allocator + Clone
{
    _impl: Tree<C, MapPair<K, V>, K, A>
}
impl<C, K, V, A> Map<C, K, V, A>
where C: TreeCompare<MapPair<K, V>, K>,
      K: PartialEq + PartialOrd + Hash,
      A: Allocator + Clone
{
    pub fn new_inner(alloc: A) -> Self { Self { _impl: Tree::new_inner(alloc) }}
    pub fn insert(&mut self, key: K, value: V) -> bool { 
        let pair = MapPair::new(key, value);
        self._impl.insert(pair)
    }
    pub fn len(&self) -> usize { self._impl.len() }
    pub fn is_empty(&self) -> bool { self._impl.is_empty() }
    pub fn find(&self, value: K) -> Option<&V> { 
        self._impl.find(value).map(|v| v.get_value())
    }
    pub fn find_mut(&mut self, value: K) -> Option<&mut V> { 
        self._impl.find_mut(value).map(|v| v.get_value_mut())
    }

    pub fn iter(&self) -> TreeIterator<MapPair<K, V>, A> {
        self._impl.into_iter()
    }
}

#[cfg(test)]
pub mod tests {
    use super::{
        CompareLess, 
        NodeColor,
        TreeCompare,
        Tree, 
        TreeNode
    };

    use allocator_api2::alloc::Global;
    use std::error::Error;

    type TestReturn = Result<(), Box<dyn Error>>;
    type Node1 = TreeNode<u32, Global>;

    #[test]
    pub fn create_blank_tree() -> TestReturn {
        let tree: Tree<CompareLess, u32, u32, Global> = Tree::new_inner(Global);
        assert!(tree.len() == 0, "Length for new tree should be zero");
        assert!(tree.is_empty(), "is_empty should be true for new tree");
        Ok(())
    }

    pub struct TreeAssertion {
        leaf: *const Node1
    }
    impl TreeAssertion {
        fn new(tree: &Tree<CompareLess, u32, u32, Global>) -> Self {
            Self { leaf: &raw const *tree.get_head() }
        }
        fn check_node(&self, n: &Node1, color: NodeColor, left: Option<&Node1>, 
            parent: Option<&Node1>, right: Option<&Node1>) {
            assert!(n.color == color, "<{}> should be *{:?}* instead of {:?}", n.data, color, n.color);
            match left {
                Some(v) => assert!(std::ptr::eq(n.left, v), "Left child for <{}> should be <{}>", n.data, v.data),
                None => assert!(std::ptr::eq(n.left, self.leaf), "Left child for <{}> should be nil", n.data)
            }
            match parent {
                Some(v) => assert!(std::ptr::eq(n.parent, v), "Parent node for <{}> should be <{}>", n.data, v.data),
                None => assert!(std::ptr::eq(n.parent, self.leaf), "Parent node for <{}> should be nil", n.data)
            }
            match right {
                Some(v) => assert!(std::ptr::eq(n.right, v), "Right child for <{}> should be <{}>", n.data, v.data),
                None => assert!(std::ptr::eq(n.right, self.leaf), "Right child for <{}> should be nil", n.data)
            }
        }
    }

    #[test]
    pub fn tree_insert_entries() -> TestReturn {
        let mut tree: Tree<CompareLess, u32, u32, Global> = Tree::new_inner(Global);
        let asserter = TreeAssertion::new(&tree);
        // Final result:
        //
        //      4
        //     / \
        //    2   6
        //  / \  / \
        // 1  3 5  8
        //        / \
        //       7  9
        //
        // --------------
        // Insert 1:
        //
        //      1B
        tree.insert(1);
        for n in tree.traverse_debug() {
            match n.data {
                1 => asserter.check_node(n, NodeColor::Black, None, None, None),
                _ => assert!(false, "Value {} should not be in the tree", n.data)
            }
        }
        // Insert 4
        //      1B
        //       \
        //        4R
        tree.insert(4);
        {
            let values = tree.traverse_debug();
            let node1 = values.iter().find(|f| f.data == 1).unwrap();
            let node4 = values.iter().find(|f| f.data == 4).unwrap();
            for n in &values {
                match n.data {
                    1 => asserter.check_node(n, NodeColor::Black, None, None, Some(node4)),
                    4 => asserter.check_node(n, NodeColor::Red, None, Some(node1), None),
                    _ => assert!(false, "Value {} should not be in the tree", n.data)
                }
            }
        }
        // Insert 6
        //      4B
        //     /  \
        //   1R   6R
        tree.insert(6);
        {
            let values = tree.traverse_debug();
            let node1 = values.iter().find(|f| f.data == 1).unwrap();
            let node4 = values.iter().find(|f| f.data == 4).unwrap();
            let node6 = values.iter().find(|f| f.data == 6).unwrap();
            for n in &values {
                match n.data {
                    1 => asserter.check_node(n, NodeColor::Red, None, Some(node4), None),
                    4 => asserter.check_node(n, NodeColor::Black, Some(node1), None, Some(node6)),
                    6 => asserter.check_node(n, NodeColor::Red, None, Some(node4), None),
                    _ => assert!(false, "Value {} should not be in the tree", n.data)
                }
            }
        }
        // Insert 3
        //      4B
        //     /  \
        //   1B   6B
        //    \
        //    3R
        tree.insert(3);
        {
            let values = tree.traverse_debug();
            let node1 = values.iter().find(|f| f.data == 1).unwrap();
            let node3 = values.iter().find(|f| f.data == 3).unwrap();
            let node4 = values.iter().find(|f| f.data == 4).unwrap();
            let node6 = values.iter().find(|f| f.data == 6).unwrap();
            for n in &values {
                match n.data {
                    1 => asserter.check_node(n, NodeColor::Black, None, Some(node4), Some(node3)),
                    3 => asserter.check_node(n, NodeColor::Red, None, Some(node1), None),
                    4 => asserter.check_node(n, NodeColor::Black, Some(node1), None, Some(node6)),
                    6 => asserter.check_node(n, NodeColor::Black, None, Some(node4), None),
                    _ => assert!(false, "Value {} should not be in the tree", n.data)
                }
            }
        }
        // Insert 5
        //      4B
        //     /  \
        //   1B    6B
        //    \   /
        //    3R 5R
        tree.insert(5);
        {
            let values = tree.traverse_debug();
            let node1 = values.iter().find(|f| f.data == 1).unwrap();
            let node3 = values.iter().find(|f| f.data == 3).unwrap();
            let node4 = values.iter().find(|f| f.data == 4).unwrap();
            let node5 = values.iter().find(|f| f.data == 5).unwrap();
            let node6 = values.iter().find(|f| f.data == 6).unwrap();
            for n in &values {
                match n.data {
                    1 => asserter.check_node(n, NodeColor::Black, None, Some(node4), Some(node3)),
                    3 => asserter.check_node(n, NodeColor::Red, None, Some(node1), None),
                    4 => asserter.check_node(n, NodeColor::Black, Some(node1), None, Some(node6)),
                    5 => asserter.check_node(n, NodeColor::Red, None, Some(node6), None),
                    6 => asserter.check_node(n, NodeColor::Black, Some(node5), Some(node4), None),
                    _ => assert!(false, "Value {} should not be in the tree", n.data)
                }
            }
        }
        // Insert 7
        //      4B
        //     /  \
        //   1B    6B
        //    \   /  \
        //    3R 5R  7R
        tree.insert(7);
        {
            let values = tree.traverse_debug();
            let node1 = values.iter().find(|f| f.data == 1).unwrap();
            let node3 = values.iter().find(|f| f.data == 3).unwrap();
            let node4 = values.iter().find(|f| f.data == 4).unwrap();
            let node5 = values.iter().find(|f| f.data == 5).unwrap();
            let node6 = values.iter().find(|f| f.data == 6).unwrap();
            let node7 = values.iter().find(|f| f.data == 7).unwrap();
            for n in &values {
                match n.data {
                    1 => asserter.check_node(n, NodeColor::Black, None, Some(node4), Some(node3)),
                    3 => asserter.check_node(n, NodeColor::Red, None, Some(node1), None),
                    4 => asserter.check_node(n, NodeColor::Black, Some(node1), None, Some(node6)),
                    5 => asserter.check_node(n, NodeColor::Red, None, Some(node6), None),
                    6 => asserter.check_node(n, NodeColor::Black, Some(node5), Some(node4), Some(node7)),
                    7 => asserter.check_node(n, NodeColor::Red, None, Some(node6), None),
                    _ => assert!(false, "Value {} should not be in the tree", n.data)
                }
            }
        }
        // Insert 8
        //      4B
        //     /  \
        //   1B    6R
        //    \   /  \
        //    3R 5B  7B
        //            \
        //             8R
        tree.insert(8);
        {
            let values = tree.traverse_debug();
            let node1 = values.iter().find(|f| f.data == 1).unwrap();
            let node3 = values.iter().find(|f| f.data == 3).unwrap();
            let node4 = values.iter().find(|f| f.data == 4).unwrap();
            let node5 = values.iter().find(|f| f.data == 5).unwrap();
            let node6 = values.iter().find(|f| f.data == 6).unwrap();
            let node7 = values.iter().find(|f| f.data == 7).unwrap();
            let node8 = values.iter().find(|f| f.data == 8).unwrap();
            for n in &values {
                match n.data {
                    1 => asserter.check_node(n, NodeColor::Black, None, Some(node4), Some(node3)),
                    3 => asserter.check_node(n, NodeColor::Red, None, Some(node1), None),
                    4 => asserter.check_node(n, NodeColor::Black, Some(node1), None, Some(node6)),
                    5 => asserter.check_node(n, NodeColor::Black, None, Some(node6), None),
                    6 => asserter.check_node(n, NodeColor::Red, Some(node5), Some(node4), Some(node7)),
                    7 => asserter.check_node(n, NodeColor::Black, None, Some(node6), Some(node8)),
                    8 => asserter.check_node(n, NodeColor::Red, None, Some(node7), None),
                    _ => assert!(false, "Value {} should not be in the tree", n.data)
                }
            }
        }
        // Insert 2
        //      4B
        //     /  \
        //   2B    6R
        //  / \   /  \
        // 1R 3R 5B  7B
        //            \
        //             8R
        tree.insert(2);
        {
            let values = tree.traverse_debug();
            let node1 = values.iter().find(|f| f.data == 1).unwrap();
            let node2 = values.iter().find(|f| f.data == 2).unwrap();
            let node3 = values.iter().find(|f| f.data == 3).unwrap();
            let node4 = values.iter().find(|f| f.data == 4).unwrap();
            let node5 = values.iter().find(|f| f.data == 5).unwrap();
            let node6 = values.iter().find(|f| f.data == 6).unwrap();
            let node7 = values.iter().find(|f| f.data == 7).unwrap();
            let node8 = values.iter().find(|f| f.data == 8).unwrap();
            for n in &values {
                match n.data {
                    1 => asserter.check_node(n, NodeColor::Red, None, Some(node2), None),
                    2 => asserter.check_node(n, NodeColor::Black, Some(node1), Some(node4), Some(node3)),
                    3 => asserter.check_node(n, NodeColor::Red, None, Some(node2), None),
                    4 => asserter.check_node(n, NodeColor::Black, Some(node2), None, Some(node6)),
                    5 => asserter.check_node(n, NodeColor::Black, None, Some(node6), None),
                    6 => asserter.check_node(n, NodeColor::Red, Some(node5), Some(node4), Some(node7)),
                    7 => asserter.check_node(n, NodeColor::Black, None, Some(node6), Some(node8)),
                    8 => asserter.check_node(n, NodeColor::Red, None, Some(node7), None),
                    _ => assert!(false, "Value {} should not be in the tree", n.data)
                }
            }
        }
        // Insert 9
        //      4B
        //     /  \
        //   2B    6R
        //  / \   /  \
        // 1R 3R 5B  8B
        //          /  \
        //         7R  9R
        tree.insert(9);
        {
            let values = tree.traverse_debug();
            let node1 = values.iter().find(|f| f.data == 1).unwrap();
            let node2 = values.iter().find(|f| f.data == 2).unwrap();
            let node3 = values.iter().find(|f| f.data == 3).unwrap();
            let node4 = values.iter().find(|f| f.data == 4).unwrap();
            let node5 = values.iter().find(|f| f.data == 5).unwrap();
            let node6 = values.iter().find(|f| f.data == 6).unwrap();
            let node7 = values.iter().find(|f| f.data == 7).unwrap();
            let node8 = values.iter().find(|f| f.data == 8).unwrap();
            let node9 = values.iter().find(|f| f.data == 9).unwrap();
            for n in &values {
                match n.data {
                    1 => asserter.check_node(n, NodeColor::Red, None, Some(node2), None),
                    2 => asserter.check_node(n, NodeColor::Black, Some(node1), Some(node4), Some(node3)),
                    3 => asserter.check_node(n, NodeColor::Red, None, Some(node2), None),
                    4 => asserter.check_node(n, NodeColor::Black, Some(node2), None, Some(node6)),
                    5 => asserter.check_node(n, NodeColor::Black, None, Some(node6), None),
                    6 => asserter.check_node(n, NodeColor::Red, Some(node5), Some(node4), Some(node8)),
                    7 => asserter.check_node(n, NodeColor::Red, None, Some(node8), None),
                    8 => asserter.check_node(n, NodeColor::Black, Some(node7), Some(node6), Some(node9)),
                    9 => asserter.check_node(n, NodeColor::Red, None, Some(node8), None),
                    _ => assert!(false, "Value {} should not be in the tree", n.data)
                }
            }
        }
        Ok(())
    }

    #[test]
    pub fn tree_find_entries() -> TestReturn {
        Ok(())
    }

    #[test]
    pub fn tree_remove_entries() -> TestReturn {
        Ok(())
    }

    #[test]
    pub fn tree_as_set() -> TestReturn {
        Ok(())
    }

    #[test]
    pub fn tree_as_map() -> TestReturn {
        Ok(())
    }
}
