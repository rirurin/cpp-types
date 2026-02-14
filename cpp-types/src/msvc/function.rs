use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ptr::NonNull;
// use cpp_types_macro::create_function_param_structs;

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
        params.invoke(self, self.get_call_ptr())
    }

    fn get_call_ptr(&self) -> *const usize {
        unsafe { self.vtable.add(FunctionImplVtable::DoCall as usize) }
    }

    fn get_call_mut_ptr(&mut self) -> *mut usize {
        unsafe { self.vtable.add(FunctionImplVtable::DoCall as usize) as *mut _ }
    }

    pub fn get_call<F>(&self) -> *const fn(&F) -> R {
        self.get_call_ptr() as _
    }

    pub fn get_call_mut<F>(&mut self) -> *mut fn(&F) -> R {
        self.get_call_mut_ptr() as _
    }

    pub fn set_call<F>(&mut self, function: fn(&F) -> R) {
        unsafe { *self.get_call_mut() = function }
    }
}

// Recursive expansion of create_function_param_structs! macro
// ============================================================

pub struct With0Params<R>
where
    R: Sized,
{
    _return_value: PhantomData<R>,
}
impl<R> FunctionParams<R> for With0Params<R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe { &*(ptr as *const fn(&F) -> R) })(function)
    }
}
impl<R> With0Params<R>
where
    R: Sized,
{
    pub fn new() -> Self {
        Self {
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With1Param<P0, R>
where
    R: Sized,
{
    _param0: P0,
    _return_value: PhantomData<R>,
}
impl<P0, R> FunctionParams<R> for With1Param<P0, R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe { &*(ptr as *const fn(&F, &P0) -> R) })(function, &self._param0)
    }
}
impl<P0, R> With1Param<P0, R>
where
    R: Sized,
{
    pub fn new(_param0: P0) -> Self {
        Self {
            _param0,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With2Params<P0, P1, R>
where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _return_value: PhantomData<R>,
}
impl<P0, P1, R> FunctionParams<R> for With2Params<P0, P1, R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe { &*(ptr as *const fn(&F, &P0, &P1) -> R) })(function, &self._param0, &self._param1)
    }
}
impl<P0, P1, R> With2Params<P0, P1, R>
where
    R: Sized,
{
    pub fn new(_param0: P0, _param1: P1) -> Self {
        Self {
            _param0,
            _param1,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With3Params<P0, P1, P2, R>
where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _param2: P2,
    _return_value: PhantomData<R>,
}
impl<P0, P1, P2, R> FunctionParams<R> for With3Params<P0, P1, P2, R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe { &*(ptr as *const fn(&F, &P0, &P1, &P2) -> R) })(
            function,
            &self._param0,
            &self._param1,
            &self._param2,
        )
    }
}
impl<P0, P1, P2, R> With3Params<P0, P1, P2, R>
where
    R: Sized,
{
    pub fn new(_param0: P0, _param1: P1, _param2: P2) -> Self {
        Self {
            _param0,
            _param1,
            _param2,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With4Params<P0, P1, P2, P3, R>
where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _param2: P2,
    _param3: P3,
    _return_value: PhantomData<R>,
}
impl<P0, P1, P2, P3, R> FunctionParams<R> for With4Params<P0, P1, P2, P3, R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe { &*(ptr as *const fn(&F, &P0, &P1, &P2, &P3) -> R) })(
            function,
            &self._param0,
            &self._param1,
            &self._param2,
            &self._param3,
        )
    }
}
impl<P0, P1, P2, P3, R> With4Params<P0, P1, P2, P3, R>
where
    R: Sized,
{
    pub fn new(_param0: P0, _param1: P1, _param2: P2, _param3: P3) -> Self {
        Self {
            _param0,
            _param1,
            _param2,
            _param3,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With5Params<P0, P1, P2, P3, P4, R>
where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _param2: P2,
    _param3: P3,
    _param4: P4,
    _return_value: PhantomData<R>,
}
impl<P0, P1, P2, P3, P4, R> FunctionParams<R> for With5Params<P0, P1, P2, P3, P4, R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe { &*(ptr as *const fn(&F, &P0, &P1, &P2, &P3, &P4) -> R) })(
            function,
            &self._param0,
            &self._param1,
            &self._param2,
            &self._param3,
            &self._param4,
        )
    }
}
impl<P0, P1, P2, P3, P4, R> With5Params<P0, P1, P2, P3, P4, R>
where
    R: Sized,
{
    pub fn new(_param0: P0, _param1: P1, _param2: P2, _param3: P3, _param4: P4) -> Self {
        Self {
            _param0,
            _param1,
            _param2,
            _param3,
            _param4,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With6Params<P0, P1, P2, P3, P4, P5, R>
where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _param2: P2,
    _param3: P3,
    _param4: P4,
    _param5: P5,
    _return_value: PhantomData<R>,
}
impl<P0, P1, P2, P3, P4, P5, R> FunctionParams<R> for With6Params<P0, P1, P2, P3, P4, P5, R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe { &*(ptr as *const fn(&F, &P0, &P1, &P2, &P3, &P4, &P5) -> R) })(
            function,
            &self._param0,
            &self._param1,
            &self._param2,
            &self._param3,
            &self._param4,
            &self._param5,
        )
    }
}
impl<P0, P1, P2, P3, P4, P5, R> With6Params<P0, P1, P2, P3, P4, P5, R>
where
    R: Sized,
{
    pub fn new(
        _param0: P0,
        _param1: P1,
        _param2: P2,
        _param3: P3,
        _param4: P4,
        _param5: P5,
    ) -> Self {
        Self {
            _param0,
            _param1,
            _param2,
            _param3,
            _param4,
            _param5,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With7Params<P0, P1, P2, P3, P4, P5, P6, R>
where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _param2: P2,
    _param3: P3,
    _param4: P4,
    _param5: P5,
    _param6: P6,
    _return_value: PhantomData<R>,
}
impl<P0, P1, P2, P3, P4, P5, P6, R> FunctionParams<R> for With7Params<P0, P1, P2, P3, P4, P5, P6, R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe { &*(ptr as *const fn(&F, &P0, &P1, &P2, &P3, &P4, &P5, &P6) -> R) })(
            function,
            &self._param0,
            &self._param1,
            &self._param2,
            &self._param3,
            &self._param4,
            &self._param5,
            &self._param6,
        )
    }
}
impl<P0, P1, P2, P3, P4, P5, P6, R> With7Params<P0, P1, P2, P3, P4, P5, P6, R>
where
    R: Sized,
{
    pub fn new(
        _param0: P0,
        _param1: P1,
        _param2: P2,
        _param3: P3,
        _param4: P4,
        _param5: P5,
        _param6: P6,
    ) -> Self {
        Self {
            _param0,
            _param1,
            _param2,
            _param3,
            _param4,
            _param5,
            _param6,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With8Params<P0, P1, P2, P3, P4, P5, P6, P7, R>
where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _param2: P2,
    _param3: P3,
    _param4: P4,
    _param5: P5,
    _param6: P6,
    _param7: P7,
    _return_value: PhantomData<R>,
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, R> FunctionParams<R>
for With8Params<P0, P1, P2, P3, P4, P5, P6, P7, R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe { &*(ptr as *const fn(&F, &P0, &P1, &P2, &P3, &P4, &P5, &P6, &P7) -> R) })(
            function,
            &self._param0,
            &self._param1,
            &self._param2,
            &self._param3,
            &self._param4,
            &self._param5,
            &self._param6,
            &self._param7,
        )
    }
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, R> With8Params<P0, P1, P2, P3, P4, P5, P6, P7, R>
where
    R: Sized,
{
    pub fn new(
        _param0: P0,
        _param1: P1,
        _param2: P2,
        _param3: P3,
        _param4: P4,
        _param5: P5,
        _param6: P6,
        _param7: P7,
    ) -> Self {
        Self {
            _param0,
            _param1,
            _param2,
            _param3,
            _param4,
            _param5,
            _param6,
            _param7,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With9Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, R>
where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _param2: P2,
    _param3: P3,
    _param4: P4,
    _param5: P5,
    _param6: P6,
    _param7: P7,
    _param8: P8,
    _return_value: PhantomData<R>,
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, R> FunctionParams<R>
for With9Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe { &*(ptr as *const fn(&F, &P0, &P1, &P2, &P3, &P4, &P5, &P6, &P7, &P8) -> R) })(
            function,
            &self._param0,
            &self._param1,
            &self._param2,
            &self._param3,
            &self._param4,
            &self._param5,
            &self._param6,
            &self._param7,
            &self._param8,
        )
    }
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, R> With9Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, R>
where
    R: Sized,
{
    pub fn new(
        _param0: P0,
        _param1: P1,
        _param2: P2,
        _param3: P3,
        _param4: P4,
        _param5: P5,
        _param6: P6,
        _param7: P7,
        _param8: P8,
    ) -> Self {
        Self {
            _param0,
            _param1,
            _param2,
            _param3,
            _param4,
            _param5,
            _param6,
            _param7,
            _param8,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With10Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, R>
where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _param2: P2,
    _param3: P3,
    _param4: P4,
    _param5: P5,
    _param6: P6,
    _param7: P7,
    _param8: P8,
    _param9: P9,
    _return_value: PhantomData<R>,
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, R> FunctionParams<R>
for With10Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe { &*(ptr as *const fn(&F, &P0, &P1, &P2, &P3, &P4, &P5, &P6, &P7, &P8, &P9) -> R) })(
            function,
            &self._param0,
            &self._param1,
            &self._param2,
            &self._param3,
            &self._param4,
            &self._param5,
            &self._param6,
            &self._param7,
            &self._param8,
            &self._param9,
        )
    }
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, R>
With10Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, R>
where
    R: Sized,
{
    pub fn new(
        _param0: P0,
        _param1: P1,
        _param2: P2,
        _param3: P3,
        _param4: P4,
        _param5: P5,
        _param6: P6,
        _param7: P7,
        _param8: P8,
        _param9: P9,
    ) -> Self {
        Self {
            _param0,
            _param1,
            _param2,
            _param3,
            _param4,
            _param5,
            _param6,
            _param7,
            _param8,
            _param9,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With11Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, R>
where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _param2: P2,
    _param3: P3,
    _param4: P4,
    _param5: P5,
    _param6: P6,
    _param7: P7,
    _param8: P8,
    _param9: P9,
    _param10: P10,
    _return_value: PhantomData<R>,
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, R> FunctionParams<R>
for With11Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe {
            &*(ptr as *const fn(&F, &P0, &P1, &P2, &P3, &P4, &P5, &P6, &P7, &P8, &P9, &P10) -> R)
        })(
            function,
            &self._param0,
            &self._param1,
            &self._param2,
            &self._param3,
            &self._param4,
            &self._param5,
            &self._param6,
            &self._param7,
            &self._param8,
            &self._param9,
            &self._param10,
        )
    }
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, R>
With11Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, R>
where
    R: Sized,
{
    pub fn new(
        _param0: P0,
        _param1: P1,
        _param2: P2,
        _param3: P3,
        _param4: P4,
        _param5: P5,
        _param6: P6,
        _param7: P7,
        _param8: P8,
        _param9: P9,
        _param10: P10,
    ) -> Self {
        Self {
            _param0,
            _param1,
            _param2,
            _param3,
            _param4,
            _param5,
            _param6,
            _param7,
            _param8,
            _param9,
            _param10,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With12Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, R>
where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _param2: P2,
    _param3: P3,
    _param4: P4,
    _param5: P5,
    _param6: P6,
    _param7: P7,
    _param8: P8,
    _param9: P9,
    _param10: P10,
    _param11: P11,
    _return_value: PhantomData<R>,
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, R> FunctionParams<R>
for With12Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe {
            &*(ptr as *const fn(
                &F,
                &P0,
                &P1,
                &P2,
                &P3,
                &P4,
                &P5,
                &P6,
                &P7,
                &P8,
                &P9,
                &P10,
                &P11,
            ) -> R)
        })(
            function,
            &self._param0,
            &self._param1,
            &self._param2,
            &self._param3,
            &self._param4,
            &self._param5,
            &self._param6,
            &self._param7,
            &self._param8,
            &self._param9,
            &self._param10,
            &self._param11,
        )
    }
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, R>
With12Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, R>
where
    R: Sized,
{
    pub fn new(
        _param0: P0,
        _param1: P1,
        _param2: P2,
        _param3: P3,
        _param4: P4,
        _param5: P5,
        _param6: P6,
        _param7: P7,
        _param8: P8,
        _param9: P9,
        _param10: P10,
        _param11: P11,
    ) -> Self {
        Self {
            _param0,
            _param1,
            _param2,
            _param3,
            _param4,
            _param5,
            _param6,
            _param7,
            _param8,
            _param9,
            _param10,
            _param11,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With13Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, R>
where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _param2: P2,
    _param3: P3,
    _param4: P4,
    _param5: P5,
    _param6: P6,
    _param7: P7,
    _param8: P8,
    _param9: P9,
    _param10: P10,
    _param11: P11,
    _param12: P12,
    _return_value: PhantomData<R>,
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, R> FunctionParams<R>
for With13Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe {
            &*(ptr as *const fn(
                &F,
                &P0,
                &P1,
                &P2,
                &P3,
                &P4,
                &P5,
                &P6,
                &P7,
                &P8,
                &P9,
                &P10,
                &P11,
                &P12,
            ) -> R)
        })(
            function,
            &self._param0,
            &self._param1,
            &self._param2,
            &self._param3,
            &self._param4,
            &self._param5,
            &self._param6,
            &self._param7,
            &self._param8,
            &self._param9,
            &self._param10,
            &self._param11,
            &self._param12,
        )
    }
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, R>
With13Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, R>
where
    R: Sized,
{
    pub fn new(
        _param0: P0,
        _param1: P1,
        _param2: P2,
        _param3: P3,
        _param4: P4,
        _param5: P5,
        _param6: P6,
        _param7: P7,
        _param8: P8,
        _param9: P9,
        _param10: P10,
        _param11: P11,
        _param12: P12,
    ) -> Self {
        Self {
            _param0,
            _param1,
            _param2,
            _param3,
            _param4,
            _param5,
            _param6,
            _param7,
            _param8,
            _param9,
            _param10,
            _param11,
            _param12,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With14Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, R>
where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _param2: P2,
    _param3: P3,
    _param4: P4,
    _param5: P5,
    _param6: P6,
    _param7: P7,
    _param8: P8,
    _param9: P9,
    _param10: P10,
    _param11: P11,
    _param12: P12,
    _param13: P13,
    _return_value: PhantomData<R>,
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, R> FunctionParams<R>
for With14Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe {
            &*(ptr as *const fn(
                &F,
                &P0,
                &P1,
                &P2,
                &P3,
                &P4,
                &P5,
                &P6,
                &P7,
                &P8,
                &P9,
                &P10,
                &P11,
                &P12,
                &P13,
            ) -> R)
        })(
            function,
            &self._param0,
            &self._param1,
            &self._param2,
            &self._param3,
            &self._param4,
            &self._param5,
            &self._param6,
            &self._param7,
            &self._param8,
            &self._param9,
            &self._param10,
            &self._param11,
            &self._param12,
            &self._param13,
        )
    }
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, R>
With14Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, R>
where
    R: Sized,
{
    pub fn new(
        _param0: P0,
        _param1: P1,
        _param2: P2,
        _param3: P3,
        _param4: P4,
        _param5: P5,
        _param6: P6,
        _param7: P7,
        _param8: P8,
        _param9: P9,
        _param10: P10,
        _param11: P11,
        _param12: P12,
        _param13: P13,
    ) -> Self {
        Self {
            _param0,
            _param1,
            _param2,
            _param3,
            _param4,
            _param5,
            _param6,
            _param7,
            _param8,
            _param9,
            _param10,
            _param11,
            _param12,
            _param13,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With15Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, R>
where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _param2: P2,
    _param3: P3,
    _param4: P4,
    _param5: P5,
    _param6: P6,
    _param7: P7,
    _param8: P8,
    _param9: P9,
    _param10: P10,
    _param11: P11,
    _param12: P12,
    _param13: P13,
    _param14: P14,
    _return_value: PhantomData<R>,
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, R> FunctionParams<R>
for With15Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe {
            &*(ptr as *const fn(
                &F,
                &P0,
                &P1,
                &P2,
                &P3,
                &P4,
                &P5,
                &P6,
                &P7,
                &P8,
                &P9,
                &P10,
                &P11,
                &P12,
                &P13,
                &P14,
            ) -> R)
        })(
            function,
            &self._param0,
            &self._param1,
            &self._param2,
            &self._param3,
            &self._param4,
            &self._param5,
            &self._param6,
            &self._param7,
            &self._param8,
            &self._param9,
            &self._param10,
            &self._param11,
            &self._param12,
            &self._param13,
            &self._param14,
        )
    }
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, R>
With15Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, R>
where
    R: Sized,
{
    pub fn new(
        _param0: P0,
        _param1: P1,
        _param2: P2,
        _param3: P3,
        _param4: P4,
        _param5: P5,
        _param6: P6,
        _param7: P7,
        _param8: P8,
        _param9: P9,
        _param10: P10,
        _param11: P11,
        _param12: P12,
        _param13: P13,
        _param14: P14,
    ) -> Self {
        Self {
            _param0,
            _param1,
            _param2,
            _param3,
            _param4,
            _param5,
            _param6,
            _param7,
            _param8,
            _param9,
            _param10,
            _param11,
            _param12,
            _param13,
            _param14,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With16Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, R>
where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _param2: P2,
    _param3: P3,
    _param4: P4,
    _param5: P5,
    _param6: P6,
    _param7: P7,
    _param8: P8,
    _param9: P9,
    _param10: P10,
    _param11: P11,
    _param12: P12,
    _param13: P13,
    _param14: P14,
    _param15: P15,
    _return_value: PhantomData<R>,
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, R> FunctionParams<R>
for With16Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe {
            &*(ptr as *const fn(
                &F,
                &P0,
                &P1,
                &P2,
                &P3,
                &P4,
                &P5,
                &P6,
                &P7,
                &P8,
                &P9,
                &P10,
                &P11,
                &P12,
                &P13,
                &P14,
                &P15,
            ) -> R)
        })(
            function,
            &self._param0,
            &self._param1,
            &self._param2,
            &self._param3,
            &self._param4,
            &self._param5,
            &self._param6,
            &self._param7,
            &self._param8,
            &self._param9,
            &self._param10,
            &self._param11,
            &self._param12,
            &self._param13,
            &self._param14,
            &self._param15,
        )
    }
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, R>
With16Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, R>
where
    R: Sized,
{
    pub fn new(
        _param0: P0,
        _param1: P1,
        _param2: P2,
        _param3: P3,
        _param4: P4,
        _param5: P5,
        _param6: P6,
        _param7: P7,
        _param8: P8,
        _param9: P9,
        _param10: P10,
        _param11: P11,
        _param12: P12,
        _param13: P13,
        _param14: P14,
        _param15: P15,
    ) -> Self {
        Self {
            _param0,
            _param1,
            _param2,
            _param3,
            _param4,
            _param5,
            _param6,
            _param7,
            _param8,
            _param9,
            _param10,
            _param11,
            _param12,
            _param13,
            _param14,
            _param15,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With17Params<
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
    P8,
    P9,
    P10,
    P11,
    P12,
    P13,
    P14,
    P15,
    P16,
    R,
> where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _param2: P2,
    _param3: P3,
    _param4: P4,
    _param5: P5,
    _param6: P6,
    _param7: P7,
    _param8: P8,
    _param9: P9,
    _param10: P10,
    _param11: P11,
    _param12: P12,
    _param13: P13,
    _param14: P14,
    _param15: P15,
    _param16: P16,
    _return_value: PhantomData<R>,
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, R> FunctionParams<R>
for With17Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, R>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe {
            &*(ptr as *const fn(
                &F,
                &P0,
                &P1,
                &P2,
                &P3,
                &P4,
                &P5,
                &P6,
                &P7,
                &P8,
                &P9,
                &P10,
                &P11,
                &P12,
                &P13,
                &P14,
                &P15,
                &P16,
            ) -> R)
        })(
            function,
            &self._param0,
            &self._param1,
            &self._param2,
            &self._param3,
            &self._param4,
            &self._param5,
            &self._param6,
            &self._param7,
            &self._param8,
            &self._param9,
            &self._param10,
            &self._param11,
            &self._param12,
            &self._param13,
            &self._param14,
            &self._param15,
            &self._param16,
        )
    }
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, R>
With17Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, R>
where
    R: Sized,
{
    pub fn new(
        _param0: P0,
        _param1: P1,
        _param2: P2,
        _param3: P3,
        _param4: P4,
        _param5: P5,
        _param6: P6,
        _param7: P7,
        _param8: P8,
        _param9: P9,
        _param10: P10,
        _param11: P11,
        _param12: P12,
        _param13: P13,
        _param14: P14,
        _param15: P15,
        _param16: P16,
    ) -> Self {
        Self {
            _param0,
            _param1,
            _param2,
            _param3,
            _param4,
            _param5,
            _param6,
            _param7,
            _param8,
            _param9,
            _param10,
            _param11,
            _param12,
            _param13,
            _param14,
            _param15,
            _param16,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With18Params<
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
    P8,
    P9,
    P10,
    P11,
    P12,
    P13,
    P14,
    P15,
    P16,
    P17,
    R,
> where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _param2: P2,
    _param3: P3,
    _param4: P4,
    _param5: P5,
    _param6: P6,
    _param7: P7,
    _param8: P8,
    _param9: P9,
    _param10: P10,
    _param11: P11,
    _param12: P12,
    _param13: P13,
    _param14: P14,
    _param15: P15,
    _param16: P16,
    _param17: P17,
    _return_value: PhantomData<R>,
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, R>
FunctionParams<R>
for With18Params<
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
    P8,
    P9,
    P10,
    P11,
    P12,
    P13,
    P14,
    P15,
    P16,
    P17,
    R,
>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe {
            &*(ptr as *const fn(
                &F,
                &P0,
                &P1,
                &P2,
                &P3,
                &P4,
                &P5,
                &P6,
                &P7,
                &P8,
                &P9,
                &P10,
                &P11,
                &P12,
                &P13,
                &P14,
                &P15,
                &P16,
                &P17,
            ) -> R)
        })(
            function,
            &self._param0,
            &self._param1,
            &self._param2,
            &self._param3,
            &self._param4,
            &self._param5,
            &self._param6,
            &self._param7,
            &self._param8,
            &self._param9,
            &self._param10,
            &self._param11,
            &self._param12,
            &self._param13,
            &self._param14,
            &self._param15,
            &self._param16,
            &self._param17,
        )
    }
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, R>
With18Params<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, R>
where
    R: Sized,
{
    pub fn new(
        _param0: P0,
        _param1: P1,
        _param2: P2,
        _param3: P3,
        _param4: P4,
        _param5: P5,
        _param6: P6,
        _param7: P7,
        _param8: P8,
        _param9: P9,
        _param10: P10,
        _param11: P11,
        _param12: P12,
        _param13: P13,
        _param14: P14,
        _param15: P15,
        _param16: P16,
        _param17: P17,
    ) -> Self {
        Self {
            _param0,
            _param1,
            _param2,
            _param3,
            _param4,
            _param5,
            _param6,
            _param7,
            _param8,
            _param9,
            _param10,
            _param11,
            _param12,
            _param13,
            _param14,
            _param15,
            _param16,
            _param17,
            _return_value: PhantomData::<R>,
        }
    }
}
pub struct With19Params<
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
    P8,
    P9,
    P10,
    P11,
    P12,
    P13,
    P14,
    P15,
    P16,
    P17,
    P18,
    R,
> where
    R: Sized,
{
    _param0: P0,
    _param1: P1,
    _param2: P2,
    _param3: P3,
    _param4: P4,
    _param5: P5,
    _param6: P6,
    _param7: P7,
    _param8: P8,
    _param9: P9,
    _param10: P10,
    _param11: P11,
    _param12: P12,
    _param13: P13,
    _param14: P14,
    _param15: P15,
    _param16: P16,
    _param17: P17,
    _param18: P18,
    _return_value: PhantomData<R>,
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, R>
FunctionParams<R>
for With19Params<
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
    P8,
    P9,
    P10,
    P11,
    P12,
    P13,
    P14,
    P15,
    P16,
    P17,
    P18,
    R,
>
where
    R: Sized,
{
    fn invoke<F>(&self, function: &F, ptr: *const usize) -> R {
        (unsafe {
            &*(ptr as *const fn(
                &F,
                &P0,
                &P1,
                &P2,
                &P3,
                &P4,
                &P5,
                &P6,
                &P7,
                &P8,
                &P9,
                &P10,
                &P11,
                &P12,
                &P13,
                &P14,
                &P15,
                &P16,
                &P17,
                &P18,
            ) -> R)
        })(
            function,
            &self._param0,
            &self._param1,
            &self._param2,
            &self._param3,
            &self._param4,
            &self._param5,
            &self._param6,
            &self._param7,
            &self._param8,
            &self._param9,
            &self._param10,
            &self._param11,
            &self._param12,
            &self._param13,
            &self._param14,
            &self._param15,
            &self._param16,
            &self._param17,
            &self._param18,
        )
    }
}
impl<P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, R>
With19Params<
    P0,
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
    P7,
    P8,
    P9,
    P10,
    P11,
    P12,
    P13,
    P14,
    P15,
    P16,
    P17,
    P18,
    R,
>
where
    R: Sized,
{
    pub fn new(
        _param0: P0,
        _param1: P1,
        _param2: P2,
        _param3: P3,
        _param4: P4,
        _param5: P5,
        _param6: P6,
        _param7: P7,
        _param8: P8,
        _param9: P9,
        _param10: P10,
        _param11: P11,
        _param12: P12,
        _param13: P13,
        _param14: P14,
        _param15: P15,
        _param16: P16,
        _param17: P17,
        _param18: P18,
    ) -> Self {
        Self {
            _param0,
            _param1,
            _param2,
            _param3,
            _param4,
            _param5,
            _param6,
            _param7,
            _param8,
            _param9,
            _param10,
            _param11,
            _param12,
            _param13,
            _param14,
            _param15,
            _param16,
            _param17,
            _param18,
            _return_value: PhantomData::<R>,
        }
    }
}