#![allow(rustdoc::redundant_explicit_links)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, allow(unused_attributes))]
//!
//! # pointers
//!
//! This crate provide two traits for generic coding:
//!
//! - [Pointer] trait:
//!   - raw pointers: `*const T`, `*mut T`, `NonNull<T>`,
//!   - owned: `Box<T>`,
//!   - multiple-ownership:
//!     - `Rc<T>`, `Arc<T>`
//!     - [WaitGroupZeroGuard](https://docs.rs/crossfire/latest/crossfire/waitgroup/struct.WaitGroupZeroGuard.html):  see the doc in `crossfire` crate
//!
//! - [SmartPointer]: type that has `new()` method
//!
//! ## Irc (Intrusive Reference Counter)
//!
//! `Irc` is an intrusive reference counting smart pointer, similar to `Arc` but without weak reference support.
//! It requires the inner type to implement [IrcItem](crate::irc::IrcItem) trait to provide a counter field.
//!
//! The underlayer of `Irc` can be any types implementing `Pointer` (default to be Box),
//! unlike `Arc` which wrap a hidden ArcInner on your inner types,
//! Irc use the pointer of your inner types by [Pointer::into_raw]
//!
//! The atomic ordering is mostly the same with std `Arc` (miri test cases verified)
//!
//! **Benefits**
//!
//! - No need to manual implementing the inc / dec on counter.
//!
//! - No enforced weak counter if you don't need it (every atomic op has cost).
//!
//! - Customized counter type (not limited to AtomicUsize)
//!
//! - [IrcItem::on_drop](crate::irc::Ircitem::on_drop) in the trait allow you to have the ownship of underlying inner memory after
//!   the reference count of Irc is dropped. And you only need to define the drop behavior once,
//!   instead of write the same logic `Arc::into_inner` in every possible places
//!   (If forgetting so make your code block and hard to debug).
//!
//! - Using `Irc` to wrap a `Box`, no additional memory allocation and memory fragmentation, no
//!   additional dereference cost (than using `Arc<Box<T>>`)
//!
//! - You can allocate a box from the time of its birth and wrap it will `Irc` for temporary usage,
//!   don't need to move bytes from / to stack. (especially when the inner object is large)
//!
//! - Advanced usage, multiple layer customized counter, on the same heap object, while preserving
//!   the safe boundary
//!
//! See [module document](crate::irc) for more.
//!
//! ## Feature Flags
//!
//! *   **`default`**: only the traits.
//! *   **`irc`**: Enables the `irc` (intrusive ref count) module.

#[cfg(feature = "irc")]
pub mod irc;

#[cfg(test)]
#[allow(unused)]
mod test;

extern crate alloc;

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::sync::Arc;
use core::ptr::NonNull;

/// Abstract pointer trait to support various pointer types in collections.
///
/// This trait allows the collections to work with:
/// - `Box<T>`: Owned, automatically dropped.
/// - `Arc<T>`: Shared ownership.
/// - `Rc<T>`: Single thread ownership.
/// - `NonNull<T>`: Raw non-null pointers (manual memory management).
/// - `*const T`: Raw pointers (recommend to use `NonNull<T>` instead)
pub trait Pointer: Sized {
    type Target;

    fn as_ref(&self) -> &Self::Target;

    #[inline(always)]
    fn as_ptr(&self) -> *const Self::Target {
        self.as_ref() as *const Self::Target
    }

    /// # Safety
    ///
    /// must be pointer acquire from [Self::into_raw()]
    unsafe fn from_raw(p: *const Self::Target) -> Self;

    fn into_raw(self) -> *const Self::Target;
}

pub trait SmartPointer: Pointer {
    fn new(t: Self::Target) -> Self;
}

#[allow(clippy::unnecessary_cast)]
impl<T> Pointer for *const T {
    type Target = T;

    #[inline]
    fn as_ref(&self) -> &Self::Target {
        unsafe { &**self }
    }

    #[inline]
    unsafe fn from_raw(p: *const Self::Target) -> Self {
        p as *const T
    }

    #[inline]
    fn into_raw(self) -> *const Self::Target {
        self as *const T
    }
}

#[allow(clippy::unnecessary_cast)]
impl<T> Pointer for *mut T {
    type Target = T;

    #[inline]
    fn as_ref(&self) -> &Self::Target {
        unsafe { &**self }
    }

    #[inline]
    unsafe fn from_raw(p: *const Self::Target) -> Self {
        p as *mut T
    }

    #[inline]
    fn into_raw(self) -> *const Self::Target {
        self as *mut T
    }
}

impl<T> Pointer for NonNull<T> {
    type Target = T;

    #[inline]
    fn as_ref(&self) -> &Self::Target {
        unsafe { self.as_ref() }
    }

    #[inline]
    unsafe fn from_raw(p: *const Self::Target) -> Self {
        unsafe { NonNull::new_unchecked(p as *mut T) }
    }

    #[inline]
    fn into_raw(self) -> *const Self::Target {
        self.as_ptr()
    }
}

impl<T> Pointer for Box<T> {
    type Target = T;

    #[inline]
    fn as_ref(&self) -> &Self::Target {
        self
    }

    #[inline]
    unsafe fn from_raw(p: *const Self::Target) -> Self {
        unsafe { Box::from_raw(p as *mut T) }
    }

    #[inline]
    fn into_raw(self) -> *const Self::Target {
        Box::into_raw(self)
    }
}

impl<T> SmartPointer for Box<T> {
    #[inline]
    fn new(inner: T) -> Self {
        Box::new(inner)
    }
}

impl<T> Pointer for Rc<T> {
    type Target = T;

    #[inline]
    fn as_ref(&self) -> &Self::Target {
        self
    }

    #[inline]
    unsafe fn from_raw(p: *const Self::Target) -> Self {
        unsafe { Rc::from_raw(p) }
    }

    #[inline]
    fn into_raw(self) -> *const Self::Target {
        Rc::into_raw(self)
    }
}

impl<T> SmartPointer for Rc<T> {
    #[inline]
    fn new(inner: T) -> Self {
        Rc::new(inner)
    }
}

impl<T> Pointer for Arc<T> {
    type Target = T;

    #[inline]
    fn as_ref(&self) -> &Self::Target {
        self
    }

    #[inline]
    unsafe fn from_raw(p: *const Self::Target) -> Self {
        unsafe { Arc::from_raw(p) }
    }

    #[inline]
    fn into_raw(self) -> *const Self::Target {
        Arc::into_raw(self)
    }
}

impl<T> SmartPointer for Arc<T> {
    #[inline]
    fn new(inner: T) -> Self {
        Arc::new(inner)
    }
}

impl<T> Pointer for &T {
    type Target = T;

    #[inline]
    fn as_ref(&self) -> &Self::Target {
        self
    }

    #[inline]
    unsafe fn from_raw(p: *const Self::Target) -> Self {
        unsafe { &*p }
    }

    #[inline]
    fn into_raw(self) -> *const Self::Target {
        self as *const T
    }
}
