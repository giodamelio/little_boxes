# Lint failure triage

Use this file when the gate is red and you must decide what to change. The default answer is
"fix the code". Change the config only when the table says the config is the defect.

## Triage order

1. Reproduce with the exact CI command, including `--locked`, `--all-targets`, the feature
   flags, `--target`, and `-- -D warnings`. A different command is a different gate.
2. Sort the findings by lint name, not by file. Put each one in a bucket (real defect,
   refactor, noise) as in the SKILL.md loop "Add or tighten a lint".
3. Never weaken an unrelated lint to pay for the one that fired.

## Symptom table

| Symptom | Cause | Fix |
|---------|-------|-----|
| Clippy passes locally, fails in CI | The local command omits `--all-targets`, `-D warnings`, `--locked`, a feature set, or a `--target` | Run the CI command verbatim. Tests, benches, and examples are code too. |
| `cannot update the lock file ... because --locked was passed` (or `cannot create`) | `Cargo.lock` is stale or not committed | Do not drop `--locked` from the lint gate to make it pass. The `cargo-workflows` skill, when it is installed, owns the lockfile repair and policy. Rerun the gate with `--locked` after the repair. |
| Clippy fires lints you never enabled | The toolchain is newer, and clippy added lints to a group you enabled. Clippy 1.97 added `manual_assert_eq` to `pedantic`; 1.98 added `with_capacity_zero` and `unused_async_trait_impl`; 1.99 added `assert_is_empty` | Pin the toolchain. Adopt the new lints on purpose through the tightening loop. |
| One crate produces no lint output | Its `Cargo.toml` has no `[lints] workspace = true`, or the directory is not a workspace member | Add the inheritance stanza. A directory that is not in `members` gets no workspace lint. |
| A lint is `warn` in one crate although the workspace sets `deny` | A crate attribute such as `#![warn(lint)]` lowers the inherited level | Remove the attribute, or restate the lint at the level of the workspace table. |
| A crate has its own lint levels in its manifest | Somebody added per-crate overrides | Move the levels to the workspace root. Leave only `[lints] workspace = true`. |
| ``lint group `<group>` has the same priority (0) as a lint`` (`clippy::lint_groups_priority`, deny by default) | A group level in `[lints.clippy]` or `[workspace.lints.clippy]` has the implicit priority 0, and Cargo ignores the order of the table | Write the group as `{ level = "warn", priority = -1 }`, as in [workspace-lints.md](workspace-lints.md). Do not allow the lint. |
| `clippy::multiple_crate_versions` fails (the `cargo` group) | Two major versions of one dependency coexist in the graph | Run `cargo tree --locked --duplicates`. Unify the versions in workspace dependencies. Only if that fails, add the crate to `allowed-duplicate-crates` with a comment that names both consumers. |
| Clippy suggests an API newer than your MSRV | `rust-version` is missing or stale in the member manifest, or an `msrv` key in `clippy.toml` overrides it | Set `rust-version` in `[workspace.package]` and inherit it. Remove a stale `msrv`. |
| `clippy::doc_markdown` flags a product or proper noun | The noun is not in the ident allow-list | Add it to `doc-valid-idents`. Keep `".."` in the list so clippy's default list survives. |
| `unwrap_used` or `expect_used` fires in test code | The test exemption is missing, or the site is a shared helper that the exemption does not cover | Set `allow-unwrap-in-tests` and `allow-expect-in-tests` in `clippy.toml`. The keys cover only `#[test]` functions and `#[cfg(test)]` items. A shared helper such as `tests/common/mod.rs` is not covered: put `#![expect(clippy::unwrap_used, reason = "test helpers assert with unwrap")]` at the top of that module (verified on 1.98.1; the expectation is fulfilled in every test crate that includes the module). Do not add `cfg_attr(test, allow(..))`: `allow_attributes` fires on it. Never relax the workspace level. |
| "this lint expectation is unfulfilled" | The `expect` is stale, or a `clippy.toml` key such as `allow-unwrap-in-tests` already exempts the site | Delete the `expect`. This warning is why `expect` beats `allow`. If the lint fires only on some targets or feature sets, use `cfg_attr(<cfg>, expect(..))`. |
| A lint fires inside macro-expanded code | A macro on the in-crate expansion path produced code you do not control | Add a crate-level `#![expect(lint, reason = "<macro> expands to ...")]` in that crate only. Do not disable the lint for the workspace. |
| `missing_docs` floods a new crate | The crate is new and undocumented | Document the public items. If the crate must land first, add per-item suppressions with a TODO and track them. Do not set `missing_docs = "allow"`. |
| `arithmetic_side_effects` floods numeric code | Unchecked arithmetic everywhere | Convert to `checked_*`, `saturating_*`, or `wrapping_*` to make the intent explicit. Where wrapping is correct, `wrapping_add` documents it and satisfies the lint. |
| `too_many_arguments` fires | A function grew past the threshold | Group the parameters into a struct. Do not raise `too-many-arguments-threshold`. The lint skips `extern` functions and `#[jni_mangle]` exports (verified on 1.98.1). A `native_method!` implementation function is a Rust-ABI function whose parameters the Java signature fixes, and the lint fires on it. Put `#[expect(clippy::too_many_arguments, reason = "the Java native signature fixes the parameters")]` on that function. |
| `type_complexity` fires | A nested generic type is unreadable, for example `HashMap<K, Arc<Mutex<HashMap<K, V>>>>` | Introduce a type alias or a named struct. Do not raise `type-complexity-threshold`. |
| `unsafe_op_in_unsafe_fn` errors in an FFI crate | The body of an `unsafe fn` relies on the implicit unsafe scope | Wrap each unsafe operation in its own `unsafe { .. }` block with its own `// SAFETY:` comment. |
| `undocumented_unsafe_blocks` fires after you split a block | Each new block needs its own comment | Write one `// SAFETY:` per block, stating the invariant that block relies on. |
| `improper_ctypes_definitions` fires | A parameter or return type in an exported `extern` function is not FFI-safe | Use a `#[repr(C)]` type, a raw pointer, or an FFI-safe integer. Never allow the lint crate-wide. On `jni` 0.22, take the VM in `JNI_OnLoad` as `*mut jni::sys::JavaVM` (the `rust-jni` skill). |
| A lint fires only in one feature build | Feature-gated code is not covered by the default build | Add that feature set to the gate: `--all-features` when the features are additive, or else each supported set (`cargo hack --each-feature` when installed). |
| A lint fires only in a cross build | Code behind a target `cfg` is not linted on the host | Add `cargo clippy --target <triple>` for each shipped target to the gate. |
| `cargo doc` fails, clippy is green | Rustdoc lints (`broken_intra_doc_links` and the rest) run only under `cargo doc` | Fix the doc link. Keep `RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --no-deps --document-private-items` in the gate. |
| `warnings are denied by build.warnings configuration`, clippy with `-D warnings` is green | `CARGO_BUILD_WARNINGS=deny` also fails on `linker_messages` and rustdoc warnings | Read the warning. For linker output, fix the flag or library that the linker complains about. |
| `cargo deny --config deny.toml --locked check` fails at argument parsing | cargo-deny is older than 0.20.0, which moved `--config` to the top level | Run `cargo deny --version`. Upgrade with `cargo install --locked cargo-deny`. |
| `cargo deny --config deny.toml --locked check` fails | An advisory, a license, a banned or duplicate crate, or a source | Use the `rust-security` skill. It owns the `deny.toml` policy and the ignore rules. |
| `cargo fmt --check` fails on a file that looks formatted | `rustfmt.toml` uses an unstable option, or it sets neither `edition` nor `style_edition`, so a direct `rustfmt` call (editor, hook) formats with style edition 2015 and sorts imports differently from `cargo fmt` | Use stable options only. Set `edition` and `style_edition` in `rustfmt.toml` to the steady-state edition (the old one during a migration). |
| The documented lint policy and the manifest disagree | The document was updated ahead of the code, or the code changed without the document | Make the document describe what is enforced today. Put the target in the escalation ladder. |

## Common configuration mistakes

| Mistake | Fix |
|---------|-----|
| Suppressing a lint workspace-wide to avoid fixing code | Fix the violations, or add a per-site `#[expect]` with a reason and a TODO |
| Adding a dependency without `cargo deny --config deny.toml --locked check` | A new dependency can break the license or advisory policy. Run the check in the same commit. |
| A bare `#[allow(...)]` with no reason | `allow_attributes_without_reason` rejects it. Use `#[expect(..., reason = "...")]`. |
| Raising a `clippy.toml` threshold to avoid a refactor | Thresholds go down over time, never up |

After any config change, run the workspace gate and the suppression audit in the SKILL.md
Verification section.
