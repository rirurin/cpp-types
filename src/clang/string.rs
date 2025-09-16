#![allow(dead_code, unused_imports)]
use allocator_api2::alloc::{ Allocator, Global };
use crate::generic::string::CharBehavior;
use std::{
    alloc::Layout,
    marker::PhantomData,
    mem::{ ManuallyDrop, MaybeUninit },
    ptr::NonNull
};

const MAX_STORAGE_SIZE: usize = 0x17;

#[repr(C)]
pub struct LargeString<T = u8, A = Global>
where T: CharBehavior + PartialEq,
      A: Allocator + Clone
{
    capacity: usize,
    size: usize,
    ptr: NonNull<T>,
    _allocator: A
}

#[repr(C)]
pub struct SmallString<T = u8, A = Global>
where T: CharBehavior + PartialEq,
A: Allocator + Clone
{
    size: u8,
    storage: MaybeUninit<[u8; MAX_STORAGE_SIZE]>,
    _allocator: A,
    _type_marker: PhantomData<T>
}

pub union StringImpl<T = u8, A = Global>
where T: CharBehavior + PartialEq,
      A: Allocator + Clone
{
    large: ManuallyDrop<LargeString<T, A>>,
    small: ManuallyDrop<SmallString<T, A>>,
}

#[repr(C)]
pub struct String<T = u8, A = Global>
where T: CharBehavior + PartialEq,
      A: Allocator + Clone
{
    ptr: Option<NonNull<T>>,
    size: usize,
    storage: MaybeUninit<[u8; MAX_STORAGE_SIZE]>,
    _allocator: A,
}

impl<T, A> String<T, A>
where T: CharBehavior + PartialEq,
      A: Allocator + Clone
{

}

#[cfg(test)]
pub mod tests {

}