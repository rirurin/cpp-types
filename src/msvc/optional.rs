// https://en.cppreference.com/w/cpp/utility/optional.html
// Like Option<T> in Rust

use std::{
    fmt::{ Debug, Display },
    mem::MaybeUninit
};

#[repr(C)]
pub struct Optional<T> {
    value: MaybeUninit<T>,
    on: bool
}

impl<T> Optional<T> {

    pub fn new(value: Option<T>) -> Self { value.into() }

    fn new_inner(value: MaybeUninit<T>, on: bool) -> Self {
        Self { value, on }
    }
}

impl<T> Optional<T> {
    pub fn value(&self) -> Option<&T> {
        match self.on {
            true => Some(unsafe { self.value.assume_init_ref() }),
            false => None
        }
    }

    pub fn value_mut(&mut self) -> Option<&mut T> {
        match self.on {
            true => Some(unsafe { self.value.assume_init_mut() }),
            false => None
        }
    }
}

impl<T> Drop for Optional<T> {
    fn drop(&mut self) {
        if self.on {
            unsafe { std::ptr::drop_in_place(self.value.as_mut_ptr()) };
        }
    }
}

impl<T> From<Option<T>> for Optional<T> {
    fn from(value: Option<T>) -> Self {
        match value {
            Some(p) => Self::new_inner(MaybeUninit::new(p), true),
            None => Self::new_inner(MaybeUninit::uninit(), false)
        }
    } 
}

impl<T> Debug for Optional<T>
where T: Debug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.on {
            true => write!(f, "Some({:?})", unsafe { self.value.assume_init_ref() }),
            false => write!(f, "None"),
        }
    }
}

impl<T> Display for Optional<T>
where T: Display {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.on {
            true => write!(f, "Some({})", unsafe { self.value.assume_init_ref() }),
            false => write!(f, "None"),
        }
    }
}