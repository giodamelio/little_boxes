# Global state: rules and examples

`SKILL.md` holds the table of declaration forms. This file holds the rules that decide between
them, the `static mut` migration, and a run block that uses each form. Re-measured on Rust 1.99.1,
edition 2024.

Five rules decide the choice. Each one is a compile error or a silent bug.

1. **A `static` needs `Sync`.** `static BAD: RefCell<u32>` is rejected with
   `error[E0277]: RefCell<u32> cannot be shared between threads safely` and the
   note `shared static variables must have a type that implements Sync`. Put the
   value in a `Mutex` or an `RwLock`, or use an atomic.
2. **A `const` has no single address.** The compiler inlines the value at every
   use site, so interior mutability in a `const` mutates a fresh temporary and
   discards it. `const HITS: AtomicUsize` incremented three times reads back
   `0`. The same declaration as a `static` reads back `3`. rustc 1.93 and later
   warns with `const_item_interior_mutations`, and clippy warns with
   `clippy::declare_interior_mutable_const`. Neither is an error, so this ships.
3. **`OnceLock` and `LazyLock` differ on panic.** When the initializer panics,
   `OnceLock::get_or_init` leaves the cell empty and the next call retries.
   `LazyLock` poisons for the life of the process: every later deref panics with
   `LazyLock instance has previously been poisoned`. Use `LazyLock` for an
   initializer that cannot fail. Use `OnceLock` when the value arrives from
   outside `main`, or when the first attempt can fail.
4. **A `static` is never dropped.** Process exit runs no `Drop` on a `static`,
   so a flush-on-drop writer in a global loses its last buffer. Flush it
   explicitly before `main` returns.
5. **`thread_local!` destructors have holes.** A destructor runs when its thread
   exits, but std documents that on Unix with pthread-based TLS it does not run
   for a value on the main thread. Never put required cleanup in thread-local
   state.

Do not rewrite a `static mut` to an atomic without reading its type. An atomic
replaces a `static mut` that holds an integer or a `bool`. A `static mut` that
holds a `Vec` or a `String` needs `OnceLock` or `LazyLock` plus a `Mutex`. On
edition 2024 the `static_mut_refs` lint is deny-by-default, so every reference
to the static stops the build. Do not silence it with
`#[allow(static_mut_refs)]`; the `cargo-workflows` skill, when it is installed, has the
two exact messages and the edition-2024 migration steps.

`LazyLock` is stable since 1.80.0 and `OnceLock` since 1.70.0. A workspace on a
newer toolchain needs no `lazy_static` or `static_init` dependency. Keep
`once_cell` while `rust-version` stays below 1.80.0, or while a global
initializer returns a `Result`. `OnceLock::get_or_try_init` is still unstable on
Rust 1.99.1. It fails with `error[E0658]` under the feature gate `once_cell_try`,
issue 109737.

```rust,run
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, OnceLock};

// One address, computed on first deref from a fixed initializer.
static TABLE: LazyLock<Mutex<HashMap<u32, u64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

// One address, written once at start-up by the owner of the value.
static CONFIG: OnceLock<String> = OnceLock::new();

// One value per thread. The `const` block skips the lazy-init check per access.
thread_local! {
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

fn main() {
    CONFIG.set(String::from("prod")).expect("set exactly once");
    TABLE.lock().expect("TABLE poisoned").insert(1, 42);
    DEPTH.set(DEPTH.get() + 1);
    let len = TABLE.lock().expect("TABLE poisoned").len();
    println!("{:?} {} {}", CONFIG.get(), len, DEPTH.get());
}
```
