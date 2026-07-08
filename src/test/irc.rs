use super::{CounterI32, alive_count, reset_alive_count};
use crate::Pointer;
use crate::irc::*;
use alloc::sync::Arc;
use core::sync::atomic::AtomicUsize;
use std::thread;

struct TestItem {
    value: CounterI32,
    counter: AtomicUsize,
}

impl TestItem {
    fn new(val: i32) -> Self {
        Self { value: CounterI32::new(val), counter: AtomicUsize::new(0) }
    }
}

impl Clone for TestItem {
    fn clone(&self) -> Self {
        Self { value: self.value.clone(), counter: AtomicUsize::new(0) }
    }
}

unsafe impl IrcItem for TestItem {
    type Counter = AtomicUsize;
    fn counter(&self) -> &Self::Counter {
        &self.counter
    }
}

struct ArcTestItem {
    value: CounterI32,
    counter: AtomicUsize,
}

impl ArcTestItem {
    fn new(val: i32) -> Self {
        Self { value: CounterI32::new(val), counter: AtomicUsize::new(0) }
    }
}

unsafe impl IrcItem<(), Arc<ArcTestItem>> for ArcTestItem {
    type Counter = AtomicUsize;
    fn counter(&self) -> &Self::Counter {
        &self.counter
    }
}

#[test]
fn test_basic() {
    reset_alive_count();
    {
        let item = TestItem::new(10);
        let irc1 = Irc::<_, _, _>::new(item);
        assert_eq!(irc1.value.value, 10);
        assert_eq!(irc1.strong_count(), 1);
        assert!(irc1.is_unique());
        assert_eq!(alive_count(), 1);

        let irc2 = irc1.clone();
        assert_eq!(irc1.strong_count(), 2);
        assert_eq!(irc2.strong_count(), 2);
        assert!(!irc1.is_unique());
        assert_eq!(alive_count(), 1);

        drop(irc1);
        assert_eq!(irc2.strong_count(), 1);
        assert!(irc2.is_unique());
        assert_eq!(alive_count(), 1);
    }
    assert_eq!(alive_count(), 0);
}

#[test]
fn test_arc_underlayer() {
    reset_alive_count();
    {
        let item = ArcTestItem::new(10);
        let irc1 = Irc::<ArcTestItem, (), Arc<ArcTestItem>>::new(item);
        assert_eq!(irc1.value.value, 10);
        assert_eq!(irc1.strong_count(), 1);
        assert!(irc1.is_unique());
        assert_eq!(alive_count(), 1);

        let irc2 = irc1.clone();
        assert_eq!(irc1.strong_count(), 2);
        assert_eq!(alive_count(), 1);

        drop(irc1);
        assert_eq!(irc2.strong_count(), 1);
        assert_eq!(alive_count(), 1);
    }
    assert_eq!(alive_count(), 0);
}

#[test]
fn test_get_mut() {
    reset_alive_count();
    let mut irc = Irc::<_, _, _>::new(TestItem::new(10));
    assert!(Irc::get_mut(&mut irc).is_some());

    let _irc2 = irc.clone();
    assert!(Irc::get_mut(&mut irc).is_none());
}

#[test]
fn test_make_mut() {
    reset_alive_count();
    let mut irc = Irc::new(TestItem::new(10));

    // Unique, no clone
    {
        let m = Irc::make_mut(&mut irc);
        m.value.value = 20;
    }
    assert_eq!(irc.value.value, 20);
    assert_eq!(alive_count(), 1);

    // Not unique, should clone
    let irc2 = irc.clone();
    assert_eq!(alive_count(), 1);
    {
        let m = Irc::make_mut(&mut irc);
        m.value.value = 30;
    }
    assert_eq!(irc.value.value, 30);
    assert_eq!(irc2.value.value, 20);
    assert_eq!(alive_count(), 2);

    assert!(irc.is_unique());
    assert!(irc2.is_unique());
}

#[test]
fn test_multithread_count() {
    reset_alive_count();
    {
        let irc = Irc::new(TestItem::new(0));
        let mut handles = vec![];

        for _ in 0..10 {
            let irc_clone = irc.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let temp = irc_clone.clone();
                    assert_eq!(temp.value.value, 0);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(irc.strong_count(), 1);
        assert!(irc.is_unique());
        assert_eq!(alive_count(), 1);
    }
    assert_eq!(alive_count(), 0);
}

#[test]
fn test_multithread_drop() {
    reset_alive_count();
    {
        let irc = Irc::new(TestItem::new(0));
        let mut handles = vec![];
        for _ in 0..10 {
            let irc_clone = irc.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    let temp = irc_clone.clone();
                    assert_eq!(temp.value.value, 0);
                }
            }));
        }
        drop(irc);
        for handle in handles {
            handle.join().unwrap();
        }
    }
    assert_eq!(alive_count(), 0);
}

#[test]
fn test_drop_all() {
    reset_alive_count();
    let irc = Irc::new(TestItem::new(0));
    let mut clones = vec![];
    for _ in 0..100 {
        clones.push(irc.clone());
    }
    assert_eq!(alive_count(), 1);
    drop(clones);
    assert_eq!(alive_count(), 1);
    drop(irc);
    assert_eq!(alive_count(), 0);
}

#[test]
fn test_from_into_raw() {
    {
        let irc = Irc::new(TestItem::new(0));
        let irc_1 = irc.clone();
        let irc_2 = irc.clone();
        let irc1_p = irc_1.into_raw();
        let irc2_p = irc_2.into_raw();
        assert_eq!(irc.strong_count(), 3);
        assert_eq!(alive_count(), 1);
        let _irc1 = unsafe { Irc::from_raw(irc1_p) };
        let _irc2 = unsafe { Irc::from_raw(irc2_p) };
        assert_eq!(irc.strong_count(), 3);
        assert_eq!(alive_count(), 1);
    }
    assert_eq!(alive_count(), 0);
}
