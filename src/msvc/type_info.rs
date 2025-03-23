use std::{
    ffi::{ c_char, CStr },
    fmt::Debug,
    hash::{ Hash, Hasher }
};

#[repr(C)]
pub struct TypeInfo {
    cpp_vtable: *const u8,
    undecorated_name: *const u8
}

impl TypeInfo {
    pub fn get_decorated_name(&self) -> &str {
        let start = unsafe { ((&raw const *self).add(1) as *mut c_char).add(1) };
        unsafe { CStr::from_ptr(start) }.to_str().unwrap()
    }
}

impl Debug for TypeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TypeInfo {{ decorated_name: {} }}", self.get_decorated_name())
    }
}

impl PartialEq for TypeInfo {
    // __std_type_info_compare
    fn eq(&self, other: &Self) -> bool {
        self.cpp_vtable == other.cpp_vtable &&
        self.undecorated_name == other.undecorated_name &&
        self.get_decorated_name() == other.get_decorated_name()
    }
}

unsafe impl Send for TypeInfo {}
unsafe impl Sync for TypeInfo {}

impl Hash for TypeInfo {
    // __std_type_info_hash when H = FNV1ARTTI
    fn hash<H>(&self, state: &mut H) 
    where H: Hasher
    {
        self.get_decorated_name().as_bytes().iter().for_each(|b| (*b).hash(state))
    }
}
