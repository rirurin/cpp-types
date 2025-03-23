use std::marker::PhantomData;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FunctionImplVtable {
    Copy = 0,
    Move = 1,
    DoCall = 2,
    GetRTTIType = 3,
    Delete = 4,
    Get = 5
}

#[repr(C)]
#[derive(Debug)]
pub struct FunctionImpl<T, P> {
    vtable: *const usize,
    value: *mut T,
    field10: [usize; 5],
    ptr: *mut Self,
    _param_type: PhantomData<P>,
}

impl<T, P> FunctionImpl<T, P> {
    // this->_Do_call
    pub fn call(&self, value: P) {
        let a = unsafe { &*(self.vtable.add(FunctionImplVtable::DoCall as usize) as *const fn(&Self, &P) -> ()) };
        a(self, &value)
    }
    pub fn get(&self) -> Option<&Self> {
        if self.ptr != std::ptr::null_mut() {
            Some(unsafe { &*self.ptr } )
        } else {
            None
        }
    }
}
