//! Spinlock packed inside AtomicPointer
//!
//! # Features
//!
//! - It's exact the size of a pointer, no additional field and allocation. Better than wrapping a
//!   small pointer with `Mutex`.
//! - When no reader reading the pointer, writer can swap out old pointer with a single CAS ops.
//! - Unlike `ArcSwap`, no leaking global memory and no maintenance debt for writers.
//! - If there are readers and writer contention, writer have more active spinning.
//!
//! **Limitation:**
//!
//! Access to the pointer should be short, it does not have parking ability, only spin and yield
//! is implemented.
//!
//! # Scenario
//!
//! - One writer thread and few reader threads. Writer can bypass the lock to read and clone.
//! - Many objects, the contention on a single object is low.
//!
//! # Example
//!
//! ```rust
//! use pointers::lock::PointerSpinLock;
//! use std::sync::Arc;
//! use std::thread;
//! let mut cell = Arc::new(PointerSpinLock::<Arc<usize>>::new(None));
//!
//! let _cell = cell.clone();
//! let th = thread::spawn(move || {
//!     // reader
//!     for i in 0..10 {
//!         let _obj  = _cell.cloned();
//!         thread::yield_now();
//!     }
//! });
//! for i in 0..10 {
//!     // writer
//!     let _ = unsafe{cell.cloned_bypass()};
//!     let _ = cell.swap(Arc::new(i));
//!     thread::yield_now();
//! }
//! th.join().expect("join");
//! ```
use crate::{CloneablePointer, Pointer};
use core::marker::PhantomData;
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{
    AtomicUsize,
    Ordering::{AcqRel, Acquire, Relaxed, Release},
};
use crossfire_utils::backoff::Backoff;

pub struct PointerSpinLock<P: Pointer> {
    p: AtomicUsize,
    _phan: PhantomData<fn(&P)>,
}

const LOCK_FLAG: usize = 1;

impl<P: Pointer> Default for PointerSpinLock<P> {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

impl<P: Pointer> PointerSpinLock<P> {
    #[inline]
    pub fn empty() -> Self {
        Self { p: AtomicUsize::new(0), _phan: Default::default() }
    }

    #[inline]
    pub fn new(node: Option<P>) -> Self {
        let p = if let Some(_node) = node { _node.into_raw() as usize } else { 0 };
        Self { p: AtomicUsize::new(p), _phan: Default::default() }
    }

    #[inline]
    pub fn new_with(node: P) -> Self {
        Self { p: AtomicUsize::new(node.into_raw() as usize), _phan: Default::default() }
    }

    #[inline]
    fn _try_lock(&self, p: usize) -> Result<*mut P::Target, usize> {
        if p != 0 {
            if p & LOCK_FLAG == 0 {
                match self.p.compare_exchange(p, p | LOCK_FLAG, AcqRel, Acquire) {
                    Ok(_) => Ok(p as *mut P::Target),
                    Err(_p) => Err(_p ^ LOCK_FLAG),
                }
            } else {
                Err(p ^ LOCK_FLAG)
            }
        } else {
            Err(0)
        }
    }

    /// Acquire lock and return the guard.
    ///
    /// If the pointer is null, return None. otherwise spin the lock and return guard
    /// for the reference
    #[inline]
    pub fn lock<'a>(&'a self) -> Option<PointerSpinGuard<'a, P>> {
        let p = match self._try_lock(self.p.load(Relaxed)) {
            Ok(p) => p,
            Err(mut p) => {
                let mut back = Backoff::new();
                loop {
                    if p != 0 {
                        if !back.spin() {
                        } else {
                            std::thread::yield_now();
                        }
                        match self._try_lock(p) {
                            Ok(_p) => {
                                break _p;
                            }
                            Err(_p) => {
                                p = _p;
                            }
                        }
                    } else {
                        return None;
                    }
                }
            }
        };
        return Some(PointerSpinGuard {
            p: ManuallyDrop::new(unsafe { P::from_raw(p) }),
            inner: self,
        });
    }

    #[inline]
    fn _swap(&self, p: usize) -> Option<P> {
        let mut back = Backoff::new();
        let mut cur = self.p.load(Relaxed);
        loop {
            if cur & LOCK_FLAG == 0 {
                match self.p.compare_exchange(cur, p, AcqRel, Acquire) {
                    Ok(_) => {
                        if cur != 0 {
                            return Some(unsafe { P::from_raw(cur as *mut P::Target) });
                        } else {
                            return None;
                        }
                    }
                    Err(_cur) => {
                        cur = _cur;
                    }
                }
            } else {
                cur ^= LOCK_FLAG;
                if !back.spin() {
                } else {
                    std::thread::yield_now();
                    // try more frequently for the writer, reset to spin the next time
                    back.reset();
                }
            }
        }
    }

    /// Wait until no reader and perform swap of the pointer.
    ///
    /// If the pointer inside is null, return None, otherwise return the old pointer.
    ///
    /// NOTE: It does not need to lock the bit while spinning,
    /// in the fastpath only equals to Load + CAS.
    #[inline]
    pub fn swap(&self, new_node: P) -> Option<P> {
        let p = new_node.into_raw();
        self._swap(p as usize)
    }

    #[inline]
    pub fn take(&self) -> Option<P> {
        self._swap(0)
    }

    #[inline(always)]
    fn _unlock(&self, p: *mut P::Target) {
        self.p.store(p as usize, Release);
    }

    /// get the reference without lock
    ///
    /// # Safety
    ///
    /// For the only writer thread, you can bypass the lock to get reference
    #[inline]
    pub unsafe fn as_ref_bypass<'a>(&'a self) -> Option<&'a P::Target> {
        let p = self.p.load(Relaxed) & (!LOCK_FLAG);
        if p != 0 { Some(unsafe { &*(p as *const P::Target) }) } else { None }
    }
}

impl<P: CloneablePointer + Pointer> PointerSpinLock<P> {
    /// If the pointer inside is not None, acquire lock, clone, unlock and return P.
    #[inline]
    pub fn cloned(&self) -> Option<P> {
        let p: *mut P::Target = match self._try_lock(self.p.load(Relaxed)) {
            Ok(p) => p,
            Err(mut p) => {
                let mut back = Backoff::new();
                loop {
                    if p != 0 {
                        if !back.spin() {
                        } else {
                            std::thread::yield_now();
                        }
                        match self._try_lock(p) {
                            Ok(_p) => {
                                break _p;
                            }
                            Err(_p) => {
                                p = _p;
                            }
                        }
                    } else {
                        return None;
                    }
                }
            }
        };
        let node = unsafe { P::from_raw_and_clone(p as *const P::Target) };
        self._unlock(p);
        Some(node)
    }

    /// unsafe version of cloned
    ///
    /// # Safety
    ///
    /// For the only on writer thread model, you can bypass the lock to clone the reference
    #[inline]
    pub unsafe fn cloned_bypass(&self) -> Option<P> {
        let p = self.p.load(Relaxed) & (!LOCK_FLAG);
        if p != 0 { Some(unsafe { P::from_raw_and_clone(p as *const P::Target) }) } else { None }
    }
}

impl<P: Pointer> Drop for PointerSpinLock<P> {
    #[inline]
    fn drop(&mut self) {
        let p = *self.p.get_mut();
        debug_assert_eq!(p & LOCK_FLAG, 0);
        if p != 0 {
            let _ = unsafe { P::from_raw(p as *mut P::Target) };
        }
    }
}

pub struct PointerSpinGuard<'a, P: Pointer> {
    p: ManuallyDrop<P>,
    inner: &'a PointerSpinLock<P>,
}

impl<'a, P: Pointer> Deref for PointerSpinGuard<'a, P> {
    type Target = P;

    #[inline]
    fn deref(&self) -> &P {
        &self.p
    }
}

impl<'a, P: Pointer> DerefMut for PointerSpinGuard<'a, P> {
    #[inline]
    fn deref_mut(&mut self) -> &mut P {
        &mut self.p
    }
}

impl<'a, P: Pointer> AsRef<P::Target> for PointerSpinGuard<'a, P> {
    #[inline]
    fn as_ref(&self) -> &P::Target {
        self.p.as_ref()
    }
}

impl<'a, P: Pointer> Drop for PointerSpinGuard<'a, P> {
    #[inline]
    fn drop(&mut self) {
        self.inner._unlock(self.p.as_ptr_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::Ordering;

    #[test]
    fn test_pointer_spinlock() {
        let cell = PointerSpinLock::<Box<usize>>::new(None);
        assert!(cell.lock().is_none());
        assert_eq!(cell.p.load(Ordering::Acquire), 0);
        assert!(cell.swap(Box::new(0)).is_none());
        let p = cell._try_lock(cell.p.load(Ordering::Acquire)).unwrap();
        assert!(!p.is_null());
        // on ok, it return clean pointer
        assert_eq!((p as usize) & LOCK_FLAG, 0);
        assert_eq!(cell.p.load(Ordering::Acquire) & LOCK_FLAG, 1);
        let _p = cell._try_lock(cell.p.load(Ordering::Acquire)).unwrap_err();
        // on err, it return clean pointer
        assert!(!p.is_null());
        assert_eq!(_p & LOCK_FLAG, 0);
        cell._unlock(p);
        assert!(cell.lock().is_some());
        assert!(cell.lock().is_some());
    }
}
