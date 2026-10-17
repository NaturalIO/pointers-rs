use crate::lock::PointerSpinLock;
use core::ops::Deref;
use std::sync::Arc;
use std::thread;

#[test]
fn test_pointer_spinlock_box() {
    let mut cell = PointerSpinLock::<Box<usize>>::new(None);
    assert!(cell.lock().is_none());
    assert!(cell.swap(Box::new(0)).is_none());
    {
        let mut guard = cell.lock().unwrap();
        assert_eq!(guard.deref().as_ref(), &0);
        **guard = 1;
    }
    {
        let guard = cell.lock().unwrap();
        assert_eq!(guard.deref().as_ref(), &1);
    }
    {
        let b = cell.take().unwrap();
        assert_eq!(*b, 1);
    }
    {
        assert!(cell.take().is_none());
    }
}

#[test]
fn test_pointer_spinlock_arc() {
    let mut cell = PointerSpinLock::<Arc<usize>>::new(None);
    assert!(cell.lock().is_none());
    assert!(cell.swap(Arc::new(0)).is_none());
    {
        let guard = cell.lock().unwrap();
        assert_eq!(guard.deref().as_ref(), &0);
    }
    {
        assert_eq!(cell.swap(Arc::new(1)).unwrap().as_ref(), &0);
    }
    let arc1 = cell.cloned();
    {
        let guard = cell.lock().unwrap();
        assert_eq!(guard.deref().as_ref(), &1);
    }
    {
        let b = cell.take().unwrap();
        assert_eq!(*b, 1);
    }
    {
        assert!(cell.take().is_none());
    }
}

#[test]
fn test_pointer_spinlock_threads() {
    let mut cell = Arc::new(PointerSpinLock::<Arc<usize>>::new(None));

    let _cell = cell.clone();
    let th = thread::spawn(move || {
        // reader
        for i in 0..50 {
            let _obj = _cell.cloned();
            thread::yield_now();
            //println!("{_obj:?}");
        }
    });
    for i in 0..50 {
        // writer
        let _ = unsafe { cell.cloned_bypass() };
        let _ = cell.swap(Arc::new(i));
        thread::yield_now();
    }
    th.join().expect("join");
}
