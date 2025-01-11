#[repr(C)]
// MSVC STL: std_ref_count_obj
pub struct RefCountObject<T> {
    _cpp_vtable: *const u8,
    uses: u32,
    weaks: u32,
    _data: std::marker::PhantomData<T>
}

#[repr(C)]
pub struct SharedPtr<T> {
    _ptr: *mut T,
    _rep: *mut RefCountObject<T>
}
