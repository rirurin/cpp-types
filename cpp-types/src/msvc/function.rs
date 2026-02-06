use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ptr::NonNull;
use cpp_types_macro::create_function_param_structs;

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

#[deprecated(since = "0.3.0", note = "Use function::Function instead")]
#[repr(C)]
#[derive(Debug)]
/// Old, partial implementation of std::function from pre-0.3. This assumes that the function's
/// local data only contains a single pointer value and the function itself only uses one parameter
/// without returning anything. See function::Function for a more complete implementation
pub struct FunctionImpl<T, P> {
    vtable: *const usize,
    value: *mut T,
    field10: [usize; 5],
    ptr: Option<NonNull<Self>>,
    _value_type_old: PhantomData<T>,
    _param_type: PhantomData<P>,
}

#[allow(deprecated)]
impl<T, P> FunctionImpl<T, P> {
    // this->_Do_call
    pub fn call(&self, value: P) {
        let a = unsafe { &*(self.vtable.add(FunctionImplVtable::DoCall as usize) as *const fn(&Self, &P) -> ()) };
        a(self, &value)
    }
    pub fn get(&self) -> Option<&Self> {
        self.ptr.map(|v| unsafe { v.as_ref() })
    }

    pub fn get_value(&self) -> *mut T {
        self.value
    }
}

pub trait FunctionParams<R> where R: Sized {
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R;
}

// create_function_param_structs!{0, 20}

#[repr(C)]
#[derive(Debug)]
pub struct Function<V, P, R>
where V: Sized, P: FunctionParams<R>, R: Sized
{
    vtable: *const usize,
    local_data: MaybeUninit<[u8; 0x30]>,
    ptr: Option<NonNull<Self>>,
    _value_type: PhantomData<V>,
    _param_type: PhantomData<P>,
    _return_type: PhantomData<R>,
}

impl<V, P, R> Function<V, P, R>
where V: Sized, P: FunctionParams<R>, R: Sized
{
    pub fn get_local_data(&self) -> &V {
        assert!(size_of::<V>() <= 0x30);
        unsafe { &*(self.local_data.as_ptr() as *const V) }
    }
    pub fn get_local_data_mut(&mut self) -> &mut V {
        assert!(size_of::<V>() <= 0x30);
        unsafe { &mut *(self.local_data.as_ptr() as *mut V) }
    }

    pub fn call(&self, params: P) -> R {
        params.invoke(self, unsafe { self.vtable.add(FunctionImplVtable::DoCall as usize) })
    }
}