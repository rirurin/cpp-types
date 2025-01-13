#![allow(dead_code, unused_imports)]
use allocator_api2::alloc::{ Allocator, Global };
use std::{
    alloc::Layout,
    fmt::{ Display, Debug },
    mem::{ align_of, size_of },
    marker::{ PhantomData, PhantomPinned }
};

// See https://devblogs.microsoft.com/oldnewthing/20230807-00/?p=108562
// https://github.com/microsoft/STL/blob/main/stl/inc/xtree

#[repr(C)]
pub struct Tree<C, T, A = Global>
where C: TreeCompare<T>,
      T: PartialEq + PartialOrd,
      A: Allocator + Clone
{
    head: *mut TreeNode<T, A>,
    size: usize,
    _allocator: A,
    _comparison: PhantomData<C>
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

pub trait TreeCompare<T>
where T: PartialEq + PartialOrd
{
    fn compare(d0: &T, d1: &T) -> bool;
}

pub struct CompareLess; // std::less
impl<T> TreeCompare<T> for CompareLess
where T: PartialEq + PartialOrd
{
    fn compare(d0: &T, d1: &T) -> bool { d0 < d1 }
}

pub struct CompareGreater; // std::greater
impl<T> TreeCompare<T> for CompareGreater
where T: PartialEq + PartialOrd
{
    fn compare(d0: &T, d1: &T) -> bool { d0 < d1 }
}

// Notes: single node is black
//
// Add 50: 50 (Black)
// Add 30: 30 (Red)
// Add 20: 20 (Black)
// Add 40: 30 becomes black, 40 (Black)

impl<C, T, A> Tree<C, T, A>
where C: TreeCompare<T>,
      T: PartialEq + PartialOrd,
      A: Allocator + Clone
{
    pub fn new_inner(alloc: A) -> Self {
        let head = unsafe { TreeNode::new_head(alloc.clone()) };
        Self { head, size: 0, _allocator: alloc, _comparison: PhantomData }
    }
    pub fn len(&self) -> usize { self.size }
    pub fn is_empty(&self) -> bool { self.size == 0 }

    unsafe fn new_node(&self, value: T) -> *mut TreeNode<T, A> {
        let node = TreeNode::new_node(self._allocator.clone(), self.head); 
        std::ptr::write((&raw mut (&mut *node).data), value);
        node
    }

    unsafe fn get_head(&self) -> &TreeNode<T, A> { &*self.head }
    unsafe fn get_head_mut(&mut self) -> &mut TreeNode<T, A> { &mut *self.head }

    unsafe fn get_root(&self) -> &TreeNode<T, A> { self.get_head().get_parent().unwrap() }
    unsafe fn get_root_mut(&mut self) -> &mut TreeNode<T, A> { self.get_head_mut().get_parent_mut().unwrap() }
    unsafe fn get_root_ptr(&self) -> *const TreeNode<T, A> { self.get_head().parent }
    unsafe fn get_root_ptr_mut(&self) -> *mut TreeNode<T, A> { self.get_head().parent }

    unsafe fn make_initial_insertion(&self, node: &mut TreeNode<T, A>) -> Option<(&mut TreeNode<T, A>, NodeDirection)> {
        let mut curr_node: *mut TreeNode<T, A> = (&mut *self.head).parent;
        loop {
            // Duplicate entries are not allowed
            let node_ref = &mut *curr_node;
            if node.data == node_ref.data { return None; } 
            let dir = if C::compare(&node.data, &node_ref.data) { NodeDirection::Left } else { NodeDirection::Right };
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
    unsafe fn rotate_left(&mut self, n: *mut TreeNode<T, A>) {
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
    unsafe fn rotate_right(&mut self, n: *mut TreeNode<T, A>) {
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
    unsafe fn get_sibling(&self, n: *mut TreeNode<T, A>) -> *mut TreeNode<T, A> {
        let node = &*n;
        match node.get_parent().unwrap().left == n {
            true => node.right,
            false => node.left
        }
    }

    unsafe fn get_direction(&self, n: *mut TreeNode<T, A>) -> NodeDirection {
        let node = &*n;
        match node.get_parent().unwrap().right == n {
            true => NodeDirection::Right,
            false => NodeDirection::Left
        }
    }
    // NOTE: Assume that grandparent (node->parent->parent) is valid when starting (only call this
    // if tree's height >= 2)
    unsafe fn post_insert_maintain_rbt(&mut self, n: *mut TreeNode<T, A>) {
        let mut node = n;
        loop {
            // retuns if a parent exists and is red, since that violates rb-tree rules
            let parent = match (&mut *node).get_parent_mut() {
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
                (&mut *uncle).color = NodeColor::Black;
                (&mut *grandparent).color = NodeColor::Red;
                // travel up 2 tree levels
                node = grandparent;
                // todo!("TODO: Case_I2");
            } else {
                let inner_grandchild = match unc_dir {
                    NodeDirection::Left => parent.right,
                    NodeDirection::Right => parent.left
                };
                if inner_grandchild == node {
                    todo!("TODO: Class_I5");
                }
                // println!("0x{:x}, 0x{:x}", node as usize, inner_grandchild as usize);
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

    pub fn insert(&mut self, value: T) -> bool {
        let count = self.len();
        self.size += 1;
        let node = unsafe { self.new_node(value) };
        let mut head = unsafe { self.get_head_mut() };
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
        head = unsafe { self.get_head_mut() };
        unsafe {
            if C::compare(&(&mut *node).data, &head.get_left().unwrap().data) {
                head.left = node;
            }
            if C::compare(&head.get_left().unwrap().data, &(&mut *node).data) {
                head.right = node;
            }
        }
        // maintain red-black tree property
        let parent = unsafe { (&mut *node).get_parent_mut().unwrap() };
        match parent.get_parent_mut() { // has grandparent (height >= 2)
            Some(_) => unsafe {
                // do some cleaning up
                self.post_insert_maintain_rbt(node);
                self.get_root_mut().color = NodeColor::Black;
            }, // height = 1, do nothing. rb-tree properties are maintained
            None => (),
        };
        true
    }
    /*
    // remove type can be different from storage type (e.g for maps, store as MapNode(MapKey,
    // MapValue), but find based on MapKey which is PartialEq<MapNode>
    pub fn remove<K>(&mut self, value: K)
    where K: PartialEq<T>
    {

    }
    */
}

impl<C, T, A> Tree<C, T, A>
where C: TreeCompare<T>,
      T: PartialEq + PartialOrd + Debug,
      A: Allocator + Clone
{
    pub fn traverse_test(&self) {
        let head = unsafe { self.get_head() };
        println!("Nil @ 0x{:x}: <l:0x{:x} p:0x{:x} r:0x{:x}>", 
        &raw const *head as usize, head.left as usize, head.parent as usize, head.right as usize);
        if !self.is_empty() {
            unsafe { self.get_head().get_parent().unwrap() }.traverse();
        }
    }
    pub fn insert_print(&mut self, value: T) -> bool {
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

impl<T, A> TreeNode<T, A>
where T: PartialEq + PartialOrd + Debug,
      A: Allocator + Clone
{
    fn traverse(&self) {
        if let Some(l) = self.get_left() { l.traverse() }
        let ptr = &raw const *self as usize;
        println!("Node @ 0x{:x}: <l:0x{:x} p:0x{:x} r:0x{:x}> [{:?}, {:?}]", 
        ptr, self.left as usize, self.parent as usize, self.right as usize, self.data, self.color);
        if let Some(r) = self.get_right() { r.traverse() }
    }
}

impl<'a, C, T, A> IntoIterator for &'a Tree<C, T, A>
where C: TreeCompare<T>,
      T: PartialEq + PartialOrd,
      A: Allocator + Clone
{
    type Item = &'a T;
    type IntoIter = TreeIterator<'a, T, A>;
    fn into_iter(self) -> Self::IntoIter {
        // inorder traversal, so get leftmost node
        unsafe {
            let mut stack = vec![];
            let mut current = self.get_root();
            while !current.nil {
                stack.push(current);
                current = &*current.left;
            }
            Self::IntoIter {
                root: self.get_root(),
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
    root: &'a TreeNode<T, A>,
    current: &'a TreeNode<T, A>,
    stack: Vec<&'a TreeNode<T, A>>
}

impl<'a, T, A> TreeIterator<'a, T, A>
where T: PartialEq + PartialOrd,
      A: Allocator + Clone
{
}

impl<'a, T, A> Iterator for TreeIterator<'a, T, A>
where T: PartialEq + PartialOrd,
      A: Allocator + Clone
{
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            while !self.current.nil {
                self.stack.push(self.current);
                self.current = &*self.current.left;
            }
        }
        let out = match self.stack.pop() {
            Some(v) => v,
            None => return None
        };
        self.current = unsafe { &*out.right };
        Some(&out.data)
    }
}

#[cfg(test)]
pub mod tests {
    use super::{
        CompareLess, TreeCompare,
        Tree, TreeNode
    };

    use allocator_api2::alloc::Global;
    use std::error::Error;

    type TestReturn = Result<(), Box<dyn Error>>;

    #[test]
    pub fn create_blank_tree() -> TestReturn {
        let tree: Tree<CompareLess, u32, Global> = Tree::new_inner(Global);
        assert!(tree.len() == 0, "Length for new tree should be zero");
        assert!(tree.is_empty(), "is_empty should be true for new tree");
        Ok(())
    }

    #[test]
    pub fn tree_insert_entries() -> TestReturn {
        let mut tree: Tree<CompareLess, u32, Global> = Tree::new_inner(Global);
        // tree.insert_print(50);
        // tree.insert_print(30);
        // tree.insert_print(20);
        // tree.insert_print(40);
        // tree.insert_print(70);
        // tree.insert_print(60);
        // tree.insert_print(80);
        //
        // tree.insert_print(10);
        // tree.insert_print(20);
        // tree.insert_print(30);
        // tree.insert_print(15);
        //
        // tree.insert_print(1);
        // tree.insert_print(4);
        // tree.insert_print(6);
        // tree.insert_print(3);
        // tree.insert_print(5);
        // tree.insert_print(7);
        // tree.insert_print(8);
        // tree.insert_print(2);
        // tree.insert_print(9);
        for i in 1..=10 { tree.insert_print(i * 2); }
        tree.traverse_test();
        for i in &tree {
            println!("iter: {}", *i);
        }
        // let node: *mut TreeNode<u32, Global> = unsafe { TreeNode::new_leaf(Global) };
        // let node2: *mut TreeNode<u32, Global> = unsafe { TreeNode::new_leaf(Global )};
        Ok(())
    }
}
