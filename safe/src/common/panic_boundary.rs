use std::panic::{catch_unwind, AssertUnwindSafe};

pub(crate) fn ffi_value<T, F>(panic_value: T, f: F) -> T
where
    F: FnOnce() -> T,
{
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(panic_value)
}

pub(crate) fn ffi_default<T, F>(f: F) -> T
where
    T: Default,
    F: FnOnce() -> T,
{
    ffi_value(T::default(), f)
}

pub(crate) fn ffi_int<F>(panic_value: i32, f: F) -> i32
where
    F: FnOnce() -> i32,
{
    ffi_value(panic_value, f)
}

pub(crate) fn ffi_ptr<T, F>(f: F) -> *mut T
where
    F: FnOnce() -> *mut T,
{
    ffi_value(std::ptr::null_mut(), f)
}

pub(crate) fn ffi_const_ptr<T, F>(f: F) -> *const T
where
    F: FnOnce() -> *const T,
{
    ffi_value(std::ptr::null(), f)
}

pub(crate) fn ffi_void<F>(f: F)
where
    F: FnOnce(),
{
    let _ = catch_unwind(AssertUnwindSafe(f));
}
