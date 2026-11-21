# Rungs one and two of the ladder

Deep material for [SKILL.md](../SKILL.md): complete, compiled examples of the two rungs that need
no `unsafe`. Read the ladder in SKILL.md first.

## Rung 1: a lifetime-parameterized enum

An enum parameterized by the lifetime holds borrowed data with no erasure at all. A new variant
then breaks every `match` with `E0004`.

```rust
use std::borrow::Cow;
use std::collections::HashMap;

enum Slot<'a> { Text(Cow<'a, str>), Count(u32) }

fn text<'a>(table: &'a HashMap<&str, Slot<'a>>, k: &str) -> Option<&'a Cow<'a, str>> {
    match table.get(k)? { Slot::Text(c) => Some(c), Slot::Count(_) => None }
}

fn main() {
    let s = String::from("hello world");
    let mut table = HashMap::new();
    table.insert("greeting", Slot::Text(Cow::from(&s)));
    table.insert("retries", Slot::Count(3));
    assert_eq!(text(&table, "greeting").map(|c| &**c), Some("hello world"));
    assert!(text(&table, "retries").is_none());
}
```

## Rung 2: an extensions map

A request-extension map stores owned values, or `Arc` clones of a shared value, so the `'static`
bound of `Any` costs nothing.

```rust
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default)]
struct Extensions { table: HashMap<TypeId, Box<dyn Any + Send + Sync>> }

impl Extensions {
    fn insert<T: Any + Send + Sync>(&mut self, value: T) {
        self.table.insert(TypeId::of::<T>(), Box::new(value));
    }
    fn get<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.table.get(&TypeId::of::<T>())?.downcast_ref()
    }
}

fn main() {
    let mut ext = Extensions::default();
    ext.insert(Arc::<str>::from("hello world"));   // share the buffer, do not borrow it
    assert_eq!(ext.get::<Arc<str>>().map(|s| &**s), Some("hello world"));
    assert_eq!(ext.get::<i64>(), None);
}
```
