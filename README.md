<!-- cargo-rdme start -->


# pointers

This crate provide two traits for generic coding:

- [`Pointer`](https://docs.rs/pointers/latest/pointers/trait.Pointer.html) trait:
  - raw pointers: `*const T`, `*mut T`, `NonNull<T>`,
  - owned: `Box<T>`,
  - multiple-ownership:
    - `Rc<T>`, `Arc<T>`
    - [WaitGroupZeroGuard](https://docs.rs/crossfire/latest/crossfire/waitgroup/struct.WaitGroupZeroGuard.html):  see the doc in `crossfire` crate

- [`SmartPointer`](https://docs.rs/pointers/latest/pointers/trait.SmartPointer.html): type that has `new()` method

## Irc (Intrusive Reference Counter)

`Irc` is an intrusive reference counting smart pointer, similar to `Arc` but without weak reference support.
It requires the inner type to implement IrcItem trait to provide a counter field.

The underlayer of `Irc` can be any types implementing `Pointer` (default to be Box),
unlike `Arc` which wrap a hidden ArcInner on your inner types,
Irc use the pointer of your inner types by [`Pointer::into_raw`](https://docs.rs/pointers/latest/pointers/trait.Pointer.html#tymethod.into_raw)

The atomic ordering is mostly the same with std `Arc` (miri test cases verified)

**Benefits**

- No need to manual implementing the inc / dec on counter.

- No enforced weak counter if you don't need it (every atomic op has cost).

- Customized counter type (not limited to AtomicUsize)

- IrcItem::on_drop in the trait allow you to have the ownship of underlying inner memory after
  the reference count of Irc is dropped. And you only need to define the drop behavior once,
  instead of write the same logic `Arc::into_inner` in every possible places
  (If forgetting so make your code block and hard to debug).

- Using `Irc` to wrap a `Box`, no additional memory allocation and memory fragmentation, no
  additional dereference cost (than using `Arc<Box<T>>`)

- You can allocate a box from the time of its birth and wrap it will `Irc` for temporary usage,
  don't need to move bytes from / to stack. (especially when the inner object is large)

- Advanced usage, multiple layer customized counter, on the same heap object, while preserving
  the safe boundary

See module document for more.

## Feature Flags

*   **`default`**: only the traits.
*   **`irc`**: Enables the `irc` (intrusive ref count) module.

<!-- cargo-rdme end -->
