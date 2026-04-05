use std::panic::{catch_unwind, AssertUnwindSafe};

pub(crate) fn ffi_int<F>(panic_value: i32, f: F) -> i32
where
    F: FnOnce() -> i32,
{
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(panic_value)
}

pub(crate) fn ffi_ptr<T, F>(f: F) -> *mut T
where
    F: FnOnce() -> *mut T,
{
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(std::ptr::null_mut())
}

pub(crate) fn ffi_const_ptr<T, F>(f: F) -> *const T
where
    F: FnOnce() -> *const T,
{
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(std::ptr::null())
}

pub(crate) fn ffi_void<F>(f: F)
where
    F: FnOnce(),
{
    let _ = catch_unwind(AssertUnwindSafe(f));
}
