# Guard probes

Each probe compiles or fails on stable as its fence says. Copy the one that matches the claim,
then replace the values with the exact pattern under review.

## A guard on an or-pattern runs once per matching alternative

```rust,run
fn main() {
    let mut calls = 0;
    match 1 {
        1 | 1 if {
            calls += 1;
            false
        } => unreachable!(),
        1 => {}
        _ => unreachable!(),
    }
    assert_eq!(calls, 2);
}
```

## Guard mutation errors

Assigning to the scrutinee in a guard gives E0510:

```rust,compile_fail,E0510
fn main() {
    let mut slot = Some(1);
    match slot {
        Some(_) if { slot = None; false } => {}
        _ => {}
    }
}
```

Assigning through a `ref mut` binding in a guard gives E0594:

```rust,compile_fail,E0594
fn main() {
    let mut slot = Some(1);
    match slot {
        Some(ref mut n) if { *n += 1; false } => {}
        _ => {}
    }
}
```

## An `if let` guard holds its scrutinee temporaries through the arm body

```rust,run
use std::sync::Mutex;

fn main() {
    let state = Mutex::new(Some(1));

    match 0 {
        _ if let Some(_) = *state.lock().unwrap() => assert!(state.try_lock().is_err()),
        _ => unreachable!(),
    }

    match 0 {
        _ if state.lock().unwrap().is_some() => assert!(state.try_lock().is_ok()),
        _ => unreachable!(),
    }
}
```
