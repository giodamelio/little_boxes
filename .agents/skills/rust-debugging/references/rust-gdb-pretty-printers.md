# Rust GDB and LLDB reference

How to drive a debugger on Rust code. [SKILL.md](../SKILL.md) covers when to reach for one.

Contents: GDB setup, GDB commands, LLDB setup, LLDB commands, VS Code with CodeLLDB, symbol
demangling, panic breakpoint and formatter check.

## GDB setup

### Automatic, through rust-gdb

`rust-gdb` is a script in the toolchain, and rustup runs it through its proxy. It starts GDB
with the toolchain's `lib/rustlib/etc` on the script path. GDB then auto-loads the pretty-printers
that each Rust binary or library names in its `.debug_gdb_scripts` section, so `String`, `Vec`,
`Option`, `Result`, and `HashMap` print as Rust values.

```bash
rust-gdb --args target/debug/my-cli <args>
```

### Manual ~/.gdbinit setup

Use this when you must run plain `gdb`, for example inside an IDE or a container that does not
ship the wrapper. Source the loader that the toolchain ships. Do not call
`gdb_lookup.register_printers(gdb.current_objfile())` yourself: in `~/.gdbinit` there is no
current objfile, the call gets `None`, and it raises. The shipped loader falls back to the
program space.

```gdb
# ~/.gdbinit
python
import subprocess
sysroot = subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip()
gdb.execute(f"source {sysroot}/lib/rustlib/etc/gdb_load_rust_pretty_printers.py")
end

set print pretty on
```

`rustc --print sysroot` names the toolchain that is active in the directory where GDB starts.
Printers from another toolchain can misread the layout of std types.

## GDB commands for Rust

### Types and values

```gdb
(gdb) ptype my_var
(gdb) whatis my_var
(gdb) p my_string
$1 = "hello world"
(gdb) p my_vec
$2 = Vec(size=5) = {1, 2, 3, 4, 5}
(gdb) p my_vec.len
(gdb) p my_map
$3 = HashMap(size=2) = {...}
(gdb) info locals
```

If a value prints as raw struct fields (`buf`, `len`, `ptr`) instead of these forms, the
printers are not loaded. Restart under `rust-gdb`, or fix `~/.gdbinit`.

### Breakpoints in Rust

```gdb
# A function by full path (crate paths use underscores, not hyphens)
(gdb) break my_crate::module::function_name

# A trait method: quote the whole symbol
(gdb) break '<MyType as MyTrait>::method'

# A closure (v0 names closures {closure#N})
(gdb) break my_crate::module::function_name::{closure#0}

# Every panic. Since 1.88 at least, the full name is __rustc::rust_panic.
(gdb) break __rustc::rust_panic
(gdb) break core::panicking::panic_fmt

# A file and line
(gdb) break src/lib.rs:171

# Stop only on the input that fails. Use fields, not method calls: GDB calls only
# functions that exist in the binary, and `len()` is usually inlined away.
# A Vec has `len`. Rust mode hides the fields of a slice, so switch to C first.
(gdb) break my_crate::process if id == 100
(gdb) break my_crate::flush if buf.len > 4096
(gdb) set language c
(gdb) break my_crate::decode if bytes.length > 4096
```

GDB 15.1 resolves no `rust_panic` location (Rust 1.97 and 1.99.1), so set the breakpoint on
`__rustc::rust_panic`. A condition GDB cannot evaluate does not disable the breakpoint: GDB
prints `Error in testing condition` and stops on every hit. In Rust mode `bytes.length` on a
`&[u8]` is that error (`Attempt to extract a component of a value that is not a structure`);
after `set language c` it evaluates. Test a condition with `print` at a stop before you rely on it. Do not break on
`std::panicking::begin_panic`: on 1.99.1 it exists only as generic instances, and LLDB resolves
the plain path to no location.

A conditional breakpoint on the argument that triggers the bug is faster than stepping through
thousands of good iterations.

### Threads

```gdb
(gdb) info threads
(gdb) thread 2
# Backtrace every thread: the first command to run on a hang
(gdb) thread apply all bt
# Stop other threads from running while you step
(gdb) set scheduler-locking on
```

## LLDB setup

### Automatic, through rust-lldb

```bash
rust-lldb target/debug/my-cli -- <args>
```

The wrapper runs the toolchain's `lldb` if the toolchain ships one, and the system `lldb`
otherwise. It passes one `command script import` of `lldb_lookup.py` before the target loads.

### Manual setup (Xcode, Android Studio, plain lldb)

Use this where the IDE starts LLDB itself and the wrapper never runs. Resolve the sysroot in a
shell first, because LLDB does not expand `$(...)`:

```bash
echo "command script import \"$(rustc --print sysroot)/lib/rustlib/etc/lldb_lookup.py\""
```

Paste the printed line into the LLDB console, into `~/.lldbinit`, or into the IDE's LLDB
startup commands. On Rust 1.98 and later that line is the whole setup: the script registers the
`Rust` category when it loads. Rust 1.97 and earlier ship `lldb_commands` and need a second line,
`command source <sysroot>/lib/rustlib/etc/lldb_commands`. That file does not exist in 1.98, and
the `command source` line fails there.

Check the result before you trust a printed value. Stop where a known `String` and `Option` are
in scope and print them:

```lldb
(lldb) frame variable s o
```

`s` must show its text, `o` must show its variant and value, and no Python traceback may
appear.

`type category list` is not that check: it shows `Category: Rust (enabled)` when the category
loaded, even when its providers fail. With the Ubuntu 24.04 LLDB 18.1.3 and the 1.97 or 1.99.1
scripts, `frame variable` prints a Python traceback (`AttributeError: 'SBValue' object has no
attribute 'GetSyntheticValue'`) and shows `Option<u8>` as raw `$variants$` fields. Use an LLDB
from a current Xcode or LLVM release, or debug with `rust-gdb`.

Apple's LLDB prints `This version of LLDB has no plugin for the language "rust"` at `run`. The
warning is expected; the formatters still work. Expression evaluation of Rust syntax is limited,
so inspect with `frame variable` rather than `expr`.

## LLDB commands for Rust

```lldb
# Breakpoints by symbol, by file and line, and on every panic
(lldb) b my_crate::module::function_name
(lldb) b src/lib.rs:171
(lldb) b rust_panic
(lldb) b core::panicking::panic_fmt

# Run with arguments, when the target was created without them
(lldb) run <args>

# Values. With formatters, a Vec prints as `v = size=5` plus its elements.
(lldb) frame variable
(lldb) frame variable my_var
(lldb) p my_struct.field

# Threads and backtraces
(lldb) thread list
(lldb) thread backtrace
(lldb) thread backtrace all
```

## VS Code with CodeLLDB

Since 1.11.0, CodeLLDB has no Rust formatters of its own. It uses the data formatters that
`rustc` ships. With Rust 1.98 or later, use CodeLLDB 1.12.3 or later: older versions import the
formatters only when `lldb_commands` exists, which 1.98 removed, and show raw structs.

`.vscode/launch.json`:

```json
{
    "version": "0.2.0",
    "configurations": [
        {
            "type": "lldb",
            "request": "launch",
            "name": "Debug my-cli",
            "program": "${workspaceFolder}/target/debug/my-cli",
            "args": ["--input", "fixtures/sample.bin", "--output", "/tmp/out.bin"],
            "cwd": "${workspaceFolder}",
            "env": {
                "RUST_BACKTRACE": "1",
                "RUST_LOG": "my_crate=debug,my_other_crate=trace"
            },
            "sourceMap": {
                "/rustc/<commit-hash>": "${env:HOME}/.rustup/toolchains/<toolchain>/lib/rustlib/src/rust"
            }
        },
        {
            "type": "lldb",
            "request": "launch",
            "name": "cargo test -- my_crate",
            "cargo": {
                "args": ["test", "--no-run", "-p", "my-crate"],
                "filter": { "name": "my_crate", "kind": "lib" }
            },
            "args": ["module_name"],
            "cwd": "${workspaceFolder}"
        }
    ]
}
```

Notes:

- `sourceMap` maps the `/rustc/<commit-hash>/` paths in std debug info onto the local rust-src
  component. `rustc -Vv` prints the `commit-hash`. Install the sources with
  `rustup component add rust-src`. Without the map you cannot step into `std`.
- The `cargo` block builds the test binary with `--no-run` and launches the result, so you can
  debug a test without knowing its hashed path.
- `args` in the test configuration goes to the test harness, so it filters which tests run.

## Symbol demangling

Rust 1.97 made v0 mangling the default. Symbols start with `_R` (`__R` in Mach-O `nm` output,
which adds one underscore). `#[no_mangle]` and `#[export_name]` symbols keep their literal names.

```bash
cargo install --locked rustfilt

# v0, the default shape
echo '_RNvCsbhslDugC6KQ_2m211foo_bar_baz' | rustfilt
# m2::foo_bar_baz

# Legacy shape. Older artifacts and pre-1.97 toolchains carry it.
echo '_ZN4core3fmt9Formatter9write_fmt17hb4f5d866d07ffa27E' | rustfilt
# core::fmt::Formatter::write_fmt

# c++filt demangles v0. Mach-O names need -_ to drop the extra underscore.
echo '_RNvCsbhslDugC6KQ_2m211foo_bar_baz' | c++filt
echo '__RNvCsbhslDugC6KQ_2m211foo_bar_baz' | c++filt -_
# llvm-cxxfilt and the macOS c++filt (LLVM 18):  m2::foo_bar_baz
# GNU binutils c++filt 2.42 keeps the crate hash: m2[8367458d81cab0c2]::foo_bar_baz
```

Grep for `_R`, not `_ZN`. A `_ZN` pattern matches no Rust symbol in a current build and returns
nothing without an error. Measured on rustc 1.99.1: `-C symbol-mangling-version=v0` changes no
symbol, and `-C symbol-mangling-version=legacy` fails with "requires `-Z unstable-options`",
which stable does not accept. The panic entry demangles to `__rustc::rust_panic`; 1.88.0 already
emits that name.

`rustfilt` also filters a whole file or a stream, so you can pipe a log or `nm` output through
it. `llvm-addr2line -C` and `llvm-symbolizer` demangle v0 on their own; you do not need
`rustfilt` after them. A debugger, profiler, or crash reporter that prints raw `_R` names
predates v0 support: upgrade it, or pipe its output through `rustfilt`. GNU binutils demangles v0
from 2.36. Linux perf demangles v0 from 6.16, so the perf in Ubuntu 24.04 (6.8) and Debian 13
(6.12) prints raw `_R` names. Source: the v0 tracking issue, rust-lang/rust#60705.

## Panic breakpoint and formatter check

- The panic entry is `__rustc::rust_panic` (1.88 and later at least). LLDB also resolves
  `b rust_panic`; GDB 15.1 does not, so use the full name or `core::panicking::panic_fmt`.
- Confirm the formatters work before you trust a value: `frame variable` on a known `String`
  and `Option` must show the text and the variant, with no Python traceback. `type category
  list` is no proof: LLDB 18.1.3 shows `Rust (enabled)` while the 1.99 providers fail.
- Rust 1.98 removed `lib/rustlib/etc/lldb_commands`. Inside an IDE, where the wrapper does not
  run, on Rust 1.98 and later, load the formatters with `command script import` of
  `lldb_lookup.py` only. Older toolchains also need the `lldb_commands` line (see LLDB setup).
