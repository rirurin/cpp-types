#![allow(dead_code, unused_imports)]
use allocator_api2::alloc::{ Allocator, Global };
use std::{
    alloc::Layout,
    marker::PhantomData,
    fmt::{ Debug, Display },
    mem::{ 
        align_of,
        size_of,
        ManuallyDrop
    },
    ptr::{ self, NonNull },
    sync::atomic::{
        AtomicU32,
        Ordering
    }
};


// std::_Ref_count_base
#[repr(C)]
pub struct RefCountObject<T, A = Global>
where A: Allocator + Clone
{
    // Self::destroy
    // Self::delete_this
    // Self::`scalar_deleting_destructor`
    // Self::get_deleter
    _cpp_vtable: *const u8,
    uses: AtomicU32,
    weaks: AtomicU32,
    _data: PhantomData<T>,
    _allocator: A
}

impl<T, A> RefCountObject<T, A>
where A: Allocator + Clone
{
    fn get_deleter(&self) -> usize { 0 }
}

impl<T, A> RefCountObject<T, A>
where A: Allocator + Clone
{
    fn get_layout() -> Layout {
        let size = size_of::<Self>() + size_of::<T>();
        let align = align_of::<Self>().max(align_of::<T>());
        unsafe { Layout::from_size_align_unchecked(size, align) }
    }

    pub unsafe fn get_data_ptr(&self) -> *const T {
        (&raw const *self).add(1) as *const T
    }

    pub unsafe fn get_data_ptr_mut(&mut self) -> *mut T {
        (&raw mut *self).add(1) as *mut T
    }

    fn new_inner(data: T, alloc: A, _cpp_vtable: *const u8) -> *mut Self {
        let out = unsafe { &mut *(alloc.allocate(Self::get_layout()).unwrap().as_ptr() as *mut Self) };
        out._cpp_vtable = _cpp_vtable;
        out.uses = 1.into();
        out.weaks = 1.into();
        out._data = PhantomData;
        out._allocator = alloc;
        unsafe { std::ptr::write(out.get_data_ptr_mut(), data); }
        &raw mut *out
    }
}

impl<T, A> RefCountObject<T, A>
where A: Allocator + Clone 
{
    // pub fn _debug_strong_count(&self) -> usize { self.uses.load(Ordering::Acquire) as usize }
    // pub fn _debug_weak_count(&self) -> usize { self.weaks.load(Ordering::Acquire) as usize }
    pub fn _debug_strong_count(&self) -> usize { self.uses.load(Ordering::SeqCst) as usize }
    pub fn _debug_weak_count(&self) -> usize { self.weaks.load(Ordering::SeqCst) as usize }
}

impl<T, A> Drop for RefCountObject<T, A>
where A: Allocator + Clone
{
    fn drop(&mut self) {
        unsafe { std::ptr::drop_in_place(self.get_data_ptr_mut()) }
    }
}

// std::shared_ptr
#[repr(C)]
pub struct SharedPtr<T, A>
where A: Allocator + Clone
{
    _ptr: *mut T,
    _rep: *mut RefCountObject<T, A>,
    _alloc: A
}

impl<T> SharedPtr<T, Global> {
    /// Construct an object of type T and wrap it in a SharedPtr to act as a reference counting
    /// smart pointer.
    pub fn make_shared(data: T) -> Self { Self::make_shared_in(data, Global) }
}

impl<T, A> SharedPtr<T, A>
where A: Allocator + Clone
{
    pub fn make_shared_in(data: T, _alloc: A) -> Self {
        let _rep = RefCountObject::new_inner(data, 
            _alloc.clone(), std::ptr::null());
        unsafe { Self { _ptr: (&mut *_rep).get_data_ptr_mut(), _rep, _alloc } }
    }

    pub(crate) unsafe fn get_rep(&self) -> &RefCountObject<T, A> { unsafe { &*self._rep } }
    pub(crate) unsafe fn get_rep_mut(&mut self) -> &mut RefCountObject<T, A> { unsafe { &mut *self._rep } }

    pub fn get(&self) -> &T { unsafe { &*self._ptr } }
    pub fn get_mut(&mut self) -> &mut T { unsafe { &mut *self._ptr } }
    pub fn get_ptr(&self) -> *mut T { self._ptr }

    // pub fn strong_count(&self) -> usize { unsafe { self.get_rep().uses.load(Ordering::Acquire) as usize } }
    // pub fn weak_count(&self) -> usize { unsafe { self.get_rep().weaks.load(Ordering::Acquire) as usize } }
    pub fn strong_count(&self) -> usize { unsafe { self.get_rep().uses.load(Ordering::SeqCst) as usize } }
    pub fn weak_count(&self) -> usize { unsafe { self.get_rep().weaks.load(Ordering::SeqCst) as usize } }

    pub fn unique(&self) -> bool { self.strong_count() == 1 }

    pub fn into_raw(this: Self) -> *mut RefCountObject<T, A> {
        let rep = this._rep;
        // don't call drop on this, so we can pass it to native code
        let _this = ManuallyDrop::new(this);
        rep
    }
    // Convert a SharedPtr pointer into an owned value. This will allow you to drop it once it
    // leaves the current scope.
    pub fn from_raw(mut ptr: NonNull<RefCountObject<T, A>>) -> Option<Self> {
        let r = unsafe { ptr.as_mut() };
        // match r.uses.load(Ordering::Acquire) {
        match r.uses.load(Ordering::SeqCst) {
            0 => None,
            _ => Some(Self { 
                _ptr: unsafe { r.get_data_ptr_mut() }, 
                _rep: &raw mut *r, 
                _alloc: r._allocator.clone()
            })
        }
    }

    pub fn downgrade(&mut self) -> WeakPtr<T, A> {
        let rep = unsafe { self.get_rep_mut() };
        // rep.weaks.fetch_add(1, Ordering::Release);
        rep.weaks.fetch_add(1, Ordering::SeqCst);
        WeakPtr {
            _ptr: self._ptr,
            _rep: self._rep,
            _alloc: self._alloc.clone()
        }
    }
}

impl<T, A> SharedPtr<T, A>
where A: Allocator + Clone
{
    pub fn _force_set_ptr(&mut self, _ptr: *const T) {
        unsafe { std::ptr::write(&raw mut self._ptr, _ptr as *mut T) }
    }
    pub fn _force_set_rep(&mut self, _rep: *const RefCountObject<T, A>) {
        unsafe { std::ptr::write(&raw mut self._rep, _rep as *mut RefCountObject<T, A>) }
    }
    pub fn _force_get_ptr(&self) -> *const T { self._ptr }
    pub fn _force_get_rep(&self) -> *const RefCountObject<T, A> { self._rep }

    pub fn _force_set_vtable(&mut self, _vtable: *const u8) {
        unsafe { self.get_rep_mut()._cpp_vtable = _vtable }
    }
}

impl<T, A> SharedPtr<T, A>
where A: Allocator + Clone
{
    pub fn _debug_get_ptr(&self) -> *const u8 { self._ptr as *const u8 }
    pub fn _debug_get_rep(&self) -> *const u8 { self._rep as *const u8 }
}

impl<T, A> Clone for SharedPtr<T, A>
where A: Allocator + Clone
{
    fn clone(&self) -> Self {
        let rep = unsafe { &mut *self._rep };
        // rep.uses.fetch_add(1, Ordering::Release);
        rep.uses.fetch_add(1, Ordering::SeqCst);
        Self {
            _ptr: self._ptr,
            _rep: self._rep,
            _alloc: self._alloc.clone()
        }
    }
}

impl<T, A> Drop for SharedPtr<T, A>
where A: Allocator + Clone
{
    fn drop(&mut self) {
        unsafe {
            let rep = self.get_rep_mut();
            // let old = rep.uses.fetch_sub(1, Ordering::Release);
            let old = rep.uses.fetch_sub(1, Ordering::SeqCst);
            if old == 1 {
                // keep the allocation alive, but it's semantically dropped
                // let weaks = rep.weaks.fetch_sub(1, Ordering::Release);
                let weaks = rep.weaks.fetch_sub(1, Ordering::SeqCst);
                if weaks == 1 { // now it's safe to actually drop it
                    std::ptr::drop_in_place(self._rep);
                    self._alloc.deallocate(NonNull::new_unchecked(
                        self._rep as *mut u8), RefCountObject::<T, A>::get_layout());
                }
            }
        }
    }
}

impl<T, A> Debug for SharedPtr<T, A>
where T: Debug,
      A: Allocator + Clone
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SharedPtr {{ data: {:?}, strong: {}, weak: {} }}", 
            self.get(), self.strong_count(), self.weak_count())
    }
}

impl<T, A> Display for SharedPtr<T, A>
where T: Display,
      A: Allocator + Clone
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get())
    }
}

// std::weak_ptr
#[repr(C)]
pub struct WeakPtr<T, A>
where A: Allocator + Clone
{
    _ptr: *mut T,
    _rep: *mut RefCountObject<T, A>,
    _alloc: A
}

impl<T, A> WeakPtr<T, A>
where A: Allocator + Clone
{
    pub(crate) unsafe fn get_rep(&self) -> &RefCountObject<T, A> { unsafe { &*self._rep } }
    pub(crate) unsafe fn get_rep_mut(&mut self) -> &mut RefCountObject<T, A> { unsafe { &mut *self._rep } }

    pub fn get(&self) -> Option<&T> {
        match self.strong_count() {
            0 => None,
            _ => Some(unsafe { &*self._ptr })
        }
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        match self.strong_count() {
            0 => None,
            _ => Some(unsafe { &mut *self._ptr })
        }
    }

    pub fn get_ptr(&self) -> Option<*mut T> {
        match self.strong_count() {
            0 => None,
            _ => Some(self._ptr)
        }
    }

    // Convert a WeakPtr pointer into an owned value. This will allow you to drop it once it
    // leaves the current scope.
    pub fn from_raw(mut ptr: NonNull<RefCountObject<T, A>>) -> Option<Self> {
        let r = unsafe { ptr.as_mut() };
        match r.uses.load(Ordering::Acquire) {
            0 => None,
            _ => Some(Self { 
                _ptr: unsafe { r.get_data_ptr_mut() }, 
                _rep: &raw mut *r, 
                _alloc: r._allocator.clone()
            })
        }
    }

    // pub fn strong_count(&self) -> usize { unsafe { self.get_rep().uses.load(Ordering::Acquire) as usize } }
    // pub fn weak_count(&self) -> usize { unsafe { self.get_rep().weaks.load(Ordering::Acquire) as usize } }
    pub fn strong_count(&self) -> usize { unsafe { self.get_rep().uses.load(Ordering::SeqCst) as usize } }
    pub fn weak_count(&self) -> usize { unsafe { self.get_rep().weaks.load(Ordering::SeqCst) as usize } }

    pub fn as_ptr(&self) -> *const Self { &raw const *self }
    pub fn as_ptr_mut(&mut self) -> *mut Self { &raw mut *self }

    pub fn into_raw(this: Self) -> *mut RefCountObject<T, A> {
        let rep = this._rep;
        // don't call drop on this, so we can pass it to native code
        let _this = ManuallyDrop::new(this);
        rep
    }
}

impl<T, A> WeakPtr<T, A>
where A: Allocator + Clone
{
    pub fn _force_set_ptr(&mut self, _ptr: *const T) {
        unsafe { std::ptr::write(&raw mut self._ptr, _ptr as *mut T) }
    }
    pub fn _force_set_rep(&mut self, _rep: *const RefCountObject<T, A>) {
        unsafe { std::ptr::write(&raw mut self._rep, _rep as *mut RefCountObject<T, A>) }
    }
    pub fn _force_get_ptr(&self) -> *const T { self._ptr }
    pub fn _force_get_rep(&self) -> *const RefCountObject<T, A> { self._rep }
}

impl<T, A> WeakPtr<T, A>
where A: Allocator + Clone
{
    pub fn _debug_get_ptr(&self) -> *const u8 { self._ptr as *const u8 }
    pub fn _debug_get_rep(&self) -> *const u8 { self._rep as *const u8 }
}

impl<T, A> Drop for WeakPtr<T, A>
where A: Allocator + Clone
{
    fn drop(&mut self) {
        unsafe {
            let rep = self.get_rep_mut();
            // let weaks = rep.uses.fetch_sub(1, Ordering::Release);
            let weaks = rep.uses.fetch_sub(1, Ordering::SeqCst);
            // no other SharedPtr/WeakPtr is referencing this, safe to drop
            if self.strong_count() == 0 && weaks == 1 {
                std::ptr::drop_in_place(self._rep);
                self._alloc.deallocate(NonNull::new_unchecked(
                    self._rep as *mut u8), RefCountObject::<T, A>::get_layout());
            }
        }
    }
}

impl<T, A> Debug for WeakPtr<T, A>
where T: Debug,
      A: Allocator + Clone
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WeakPtr {{ data: {:?}, strong: {}, weak: {} }}", 
            self.get(), self.strong_count(), self.weak_count())
    }
}

impl<T, A> Display for WeakPtr<T, A>
where T: Display,
      A: Allocator + Clone
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.get() {
            Some(v) => write!(f, "{}", v),
            None => write!(f, "None")
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::SharedPtr;
    use allocator_api2::alloc::{ Allocator, Global };
    use crate::msvc::string::String as CppString;
    use std::{
        error::Error,
        ptr::NonNull
    };
    type TestReturn = Result<(), Box<dyn Error>>;

    #[test]
    fn create_reference_count_string() -> TestReturn {
        let test_str = CppString::from_str("Player");
        let shared_str = SharedPtr::make_shared(test_str);
        unsafe {
            assert!(shared_str._debug_get_ptr() == shared_str._debug_get_rep().add(0x10), 
            "_ptr in SharedPtr is not contiguous with _rep");
        }
        assert!(shared_str.strong_count() == 1, "Strong count for initialized SharedPtr should be 1");
        assert!(shared_str.weak_count() == 1, "Weak count for initialized SharedPtr should be 1");
        let str_out: &str = shared_str.get().into();
        assert!(str_out == "Player", "Retrieved value from SharedPtr should equal \"Player\" instead of {}", str_out);
        Ok(())
    }

    #[test]
    fn create_cloned_reference() -> TestReturn {
        let shared_str: SharedPtr<i32, Global> = SharedPtr::make_shared(100);
        assert!(shared_str.strong_count() == 1, "Strong count for initialized SharedPtr should be 1");
        {
            let new_ptr = shared_str.clone();
            assert!(shared_str.strong_count() == 2, "Strong count after cloned should be 2");
            assert!(*new_ptr.get() == 100, "Value for cloned pointer should be 100 instead of {}", *new_ptr.get());
        }
        assert!(shared_str.strong_count() == 1, "Strong count after cloned dropped should be 1");
        Ok(())
    }

    #[test]
    fn weak_pointers() -> TestReturn {
        let weak_ptr;
        {
            let mut shared_ptr: SharedPtr<i32, Global> = SharedPtr::make_shared(200);
            weak_ptr = shared_ptr.downgrade();
            assert!(shared_ptr.strong_count() == 1, "Strong count after creating weak pointer should be 1");
            assert!(shared_ptr.weak_count() == 2, "Weak count after creating weak pointer should be 2");
            assert!(weak_ptr._debug_get_rep() == shared_ptr._debug_get_rep(), "Rep should point to the same address");
            assert!(weak_ptr.get() == Some(200).as_ref(), "Value from weak_ptr should be Some(200)");
        }
        assert!(weak_ptr.strong_count() == 0, "Strong after dropping shared_ptr should be 0");
        assert!(weak_ptr.get() == None, "Value after shared ptr dropped should be None");
        Ok(())
    }

    fn check_strong_count(get: usize, expect: usize) {
        assert!(get == expect, "Strong count should be {} instead of {}", expect, get);
    }
    fn check_weak_count(get: usize, expect: usize) {
        assert!(get == expect, "Weak count should be {} instead of {}", expect, get);
    }

    #[test]
    fn convert_raw() -> TestReturn {
        let mut shared_ptr: SharedPtr<i32, Global> = SharedPtr::make_shared(300);
        let weak = shared_ptr.downgrade();
        let shared_raw = SharedPtr::into_raw(shared_ptr);
        check_strong_count(weak.strong_count(), 1);
        check_weak_count(weak.weak_count(), 2);
        assert!(weak.get() == Some(300).as_ref(), "Value after converting SharedPtr to raw pointer should be Some(300)");
        let shared_ptr = SharedPtr::from_raw(unsafe { NonNull::new_unchecked(shared_raw) });
        assert!(shared_ptr.is_some(), "Couldn't convert raw pointer into SharedPtr");
        let shared_ptr = shared_ptr.unwrap();
        println!("{:?}", shared_ptr);
        check_strong_count(shared_ptr.strong_count(), 1);
        check_weak_count(shared_ptr.weak_count(), 2);
        assert!(*shared_ptr.get() == 300, "Value after converting raw pointer to SharedPtr should be Some(300)");
        Ok(())
    }
}
