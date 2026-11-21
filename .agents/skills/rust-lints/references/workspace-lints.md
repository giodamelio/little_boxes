# `[workspace.lints]` templates

- [Strict template](#strict-template): the target for a new workspace.
- [Notes on the strict template](#notes-on-the-strict-template)
- [Pragmatic baseline](#pragmatic-baseline-for-an-existing-workspace): the start for an
  existing workspace with a backlog.
- [Escalation ladder](#escalation-ladder): the next step up from the baseline.

Both tables belong in the workspace root manifest. Every member inherits them with
`[lints] workspace = true`.

## Strict template

Start here for a new workspace. For an existing workspace with a large backlog, start from the
[pragmatic baseline](#pragmatic-baseline-for-an-existing-workspace) and climb.

```toml
[workspace.lints.rust]
unsafe_op_in_unsafe_fn       = "deny"   # force an explicit `unsafe { .. }` inside an unsafe fn
missing_docs                 = "warn"
unused_lifetimes             = "warn"
unreachable_pub              = "warn"
elided_lifetimes_in_paths    = "warn"
let_underscore_drop          = "deny"   # `let _ = make_guard();` drops the guard at once
dropping_references          = "deny"   # drop(&guard) does nothing; only drop(guard) releases
unconditional_recursion      = "deny"   # warn by default; catches a body that calls only itself
non_ascii_idents             = "deny"
trivial_numeric_casts        = "warn"
unused_must_use              = "deny"
# unsafe_code              = "forbid"   # set per crate in lib.rs, never here (SKILL.md, Crate-level attributes)

[workspace.lints.clippy]
# Group activations. priority = -1 lowers the group below the individual
# lints below it, so a single lint can override its own group.
all                          = { level = "deny",  priority = -1 }
pedantic                     = { level = "warn",  priority = -1 }
nursery                      = { level = "warn",  priority = -1 }
cargo                        = { level = "warn",  priority = -1 }
# Under -D warnings (clippy 1.99.1), two lints of `cargo` fail builds that have no defect:
# multiple_crate_versions fires once two versions of one crate enter Cargo.lock, which most
# `cargo update` runs cause, so it is off here and cargo-deny judges duplicates.
# cargo_common_metadata fires on each crate without `publish = false` that lacks description,
# license, repository, readme, keywords, or categories.
multiple_crate_versions      = "allow"
# nursery's redundant_pub_crate asks for `pub` where rustc's unreachable_pub asks for
# `pub(crate)`, so with both on no visibility is green. Keep unreachable_pub.
redundant_pub_crate          = "allow"

# Highest-value promotions. An explicit entry keeps the lint on if its group is relaxed later.
# std::mem::forget is denied by disallowed-methods in clippy.toml. Do not add mem_forget
# as well: each call site then reports two errors.
undocumented_unsafe_blocks   = "deny"   # every unsafe block needs a // SAFETY: comment
multiple_unsafe_ops_per_block = "deny" # one SAFETY: comment per operation, not per block
missing_safety_doc           = "deny"   # a # Safety section on every pub unsafe fn
missing_panics_doc           = "warn"
missing_errors_doc           = "warn"
unwrap_used                  = "warn"   # promote to deny per crate when the backlog is clear
# expect_used is not here: `.expect("<invariant>")` is allowed in non-test code. Add it per crate.
panic                        = "warn"
todo                         = "warn"
unimplemented                = "warn"
dbg_macro                    = "deny"
print_stdout                 = "warn"
exit                         = "deny"
large_stack_arrays           = "warn"
large_stack_frames           = "warn"
large_futures                = "warn"
large_enum_variant           = "warn"   # fires on the size DIFFERENCE between the two largest variants
result_large_err             = "warn"   # warn by default; fires when the Err variant reaches 128 bytes
exhaustive_enums             = "warn"   # require #[non_exhaustive] on a pub enum
exhaustive_structs           = "warn"
disallowed_methods           = "deny"
disallowed_types             = "deny"
str_to_string                = "warn"
implicit_clone               = "warn"
assigning_clones             = "warn"   # pedantic: `a = b.clone()` becomes `a.clone_from(&b)`
redundant_clone              = "warn"   # nursery
or_fun_call                  = "warn"   # nursery: `ok_or(build())` becomes `ok_or_else(build)`
unnecessary_lazy_evaluations = "warn"   # style: the reverse error, a closure that should stay eager
needless_collect             = "warn"   # nursery: a collect() the same body only iterates or counts
needless_pass_by_value       = "warn"
ptr_arg                      = "warn"   # style, NOT perf: `&Vec<T>` becomes `&[T]`
ref_option                   = "warn"
trivially_copy_pass_by_ref   = "warn"
redundant_closure_for_method_calls = "warn"
collapsible_if               = "warn"
uninlined_format_args        = "warn"

# Restriction-class lints with a high signal-to-noise ratio.
allow_attributes             = "warn"
allow_attributes_without_reason = "warn"   # reject a bare #[allow(..)] with no justification
indexing_slicing             = "warn"   # prefer .get() over buf[i] on data read from I/O
integer_division             = "warn"
arithmetic_side_effects      = "warn"   # catches unchecked integer arithmetic
modulo_arithmetic            = "warn"
unwrap_in_result             = "warn"

[workspace.lints.rustdoc]                   # only `cargo doc` enforces these; clippy never runs them
broken_intra_doc_links       = "deny"
private_intra_doc_links      = "warn"
missing_crate_level_docs     = "warn"
bare_urls                    = "warn"
```

## Notes on the strict template

- `unreachable_pub` and the nursery lint `clippy::redundant_pub_crate` contradict each other on
  an item in a private module: `pub fn` fires the first ("consider restricting its visibility:
  `pub(crate)`"), and `pub(crate) fn` fires the second ("consider using: `pub`"), clippy 0.1.99.
  The template allows `redundant_pub_crate`, so the `pub(crate)` level that the
  `rust-code-style` skill prescribes stays green.
- Under `-D warnings`, `warn` and `deny` both fail the gate. `deny` also stops a local
  `cargo build`, so it suits lints that are never acceptable, even for a moment.
- `clippy::string_to_string` was deprecated in clippy 1.91; `implicit_clone` covers it. Clippy
  0.1.98 reports `lint clippy::string_to_string has been removed`. Do not re-add it.
- `clippy::inefficient_to_string` is not in either table. It fires only when the MSRV is below
  1.82, where `to_string()` on `&&str` skips the specialized impl, so it can never fire in an
  edition 2024 crate (rust-version 1.85 or later). `implicit_clone` catches `to_string()` on a
  `&&String` (clippy 0.1.99).
- `clippy::unwrap_in_result` checks `unwrap` and `expect` on a `Result` only; an `Option` unwrap
  in a function that returns `Result` passes it (clippy 0.1.99). `unwrap_used` still catches it.
- `all = "deny"` already denies the `await_holding_*` lints, which are in `clippy::suspicious`.
  Pin them by name only when the workspace runs an async runtime. Add the pointer and FFI
  lints only when it contains `unsafe` pointer work. Both blocks are in
  [lint-catalog.md](lint-catalog.md).
- `unconditional_recursion` is warn-by-default, so the strict template must
  promote it. An inherent method that carries a trait method's name resolves
  first, so `fn into_iter(self) -> Owned { self.into_iter() }` inside
  `impl IntoIterator` calls the inherent method and passes review. Delete that
  inherent method in a later cleanup and the same body calls itself. rustc
  still only warns and the binary links. A debug build then dies with `fatal
  runtime error: stack overflow, aborting` and exit code 134 (1.98.1). A release build
  is worse: LLVM turns the tail call into an infinite loop, so the process
  hangs with no diagnostic at all.
- Most of the performance lints above are not in the `perf` group;
  [lint-catalog.md](lint-catalog.md#clippy-lint-groups) gives the group of each.

## Pragmatic baseline for an existing workspace

An existing workspace with thousands of files cannot land the strict set in one
commit. Use group levels first, then cherry-pick the pedantic lints that pay
for themselves:

```toml
[workspace.lints.clippy]
correctness = { level = "deny", priority = -1 }   # almost certainly a bug
suspicious  = { level = "deny", priority = -1 }   # very likely wrong
style       = { level = "warn", priority = -1 }
complexity  = { level = "warn", priority = -1 }
perf        = { level = "warn", priority = -1 }

# Cherry-picked pedantic lints - cheap to fix, high hit rate.
cloned_instead_of_copied            = "warn"
explicit_iter_loop                  = "warn"
explicit_into_iter_loop             = "warn"
implicit_clone                      = "warn"
map_unwrap_or                       = "warn"
redundant_closure_for_method_calls  = "warn"
semicolon_if_nothing_returned       = "warn"
uninlined_format_args               = "warn"
unnested_or_patterns                = "warn"
manual_let_else                     = "warn"
trivially_copy_pass_by_ref          = "warn"
unused_self                         = "warn"
default_trait_access                = "warn"
match_wildcard_for_single_variants  = "warn"
should_panic_without_expect         = "warn"   # pin the expected panic message
ignore_without_reason               = "warn"   # #[ignore = "why"]

[workspace.lints.rust]
unsafe_op_in_unsafe_fn = "deny"
```

This baseline is a starting point, not a destination. Record which strict lints
are still off and why. Then follow [Add or tighten a lint](../SKILL.md#add-or-tighten-a-lint).

## Escalation ladder

Take each step through the SKILL.md loop "Add or tighten a lint".

| Step | Precondition | Move |
|------|--------------|------|
| Add `unwrap_used` at `warn` | Existing sites carry a per-site `expect`. `allow-unwrap-in-tests` exempts tests. | Every new call site then needs a reason. The `rust-discipline` skill owns the policy. |
| `unwrap_used` `warn` to `deny` in one crate | That crate has zero remaining sites | Add `#![deny(clippy::unwrap_used)]` to that crate. Keep the workspace at `warn` until every crate is clean. |
| Add `expect_used` to one crate | No panic may leave that crate, for example an FFI adapter | Add it per crate, not to the workspace table: a documented `.expect("<invariant>")` is allowed elsewhere. |
| Lower `type-complexity-threshold` or `too-many-arguments-threshold` | The workspace is clean at the current value | Step down. Do not raise it for a bridge layer; [triage.md](triage.md) has the `extern` and `native_method!` cases. |
| Unsafe-documentation lints to `deny` | Every `unsafe` block has a `// SAFETY:` comment | Promote `undocumented_unsafe_blocks` and `multiple_unsafe_ops_per_block`. |
