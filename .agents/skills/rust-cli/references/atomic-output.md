# Atomic file output

Read this when a command writes, replaces, or refuses to overwrite a regular file.

Contents: commit transaction, standard library primitives and probe, no-clobber
output, platform replacement, metadata policy, `--force` and multi-file output.

## Commit transaction

Do not truncate the destination before computation succeeds. For a regular-file
output, use this transaction:

1. Resolve the destination and validate the overwrite and symlink policy.
2. Create a new unpredictable temporary file in the destination directory with
   exclusive creation and the restrictive mode or security descriptor in the
   same open operation. Do not create it broadly and restrict it later.
3. Apply any additional metadata before sensitive data is written.
4. Stream all bytes to the temporary file and handle every write error.
5. Flush userspace buffers. Call file sync when the contract requires durability.
6. Close handles that prevent replacement on the target platform.
7. Commit with one platform-supported atomic no-replace or replace operation
   that matches the overwrite policy.
8. Sync the parent directory on Unix when rename durability is required.
9. Remove a leftover temporary file on ordinary failure when safe.

The temporary file must use the same filesystem as the destination. A rename
across filesystems is not atomic. Do not implement overwrite as remove-then-
rename. A crash between those calls loses the old output.

## Standard library primitives

- `OpenOptions::new().write(true).create_new(true)` creates the file
  exclusively. It fails with `AlreadyExists` when the name exists. On Unix this
  includes a dangling symbolic link, so an attacker's link is not followed.
  Retry with a new name on `AlreadyExists`.
- On Unix, `std::os::unix::fs::OpenOptionsExt::mode(0o600)` sets the permission
  bits in the same `open` call. The process umask can only remove bits.
- `std::fs::rename` replaces an existing destination. Use it as the replace
  commit. It is never a no-clobber commit.
- `std::fs::hard_link(temp, dest)` fails with `AlreadyExists` when the
  destination exists. Use it as the atomic no-clobber commit on a filesystem
  that supports hard links, then remove the temporary name. On a filesystem
  without hard links, report that atomic no-clobber output is unavailable.
  Treat every `hard_link` error other than `AlreadyExists` as a failed commit.
  Never fall back to `rename`, because it overwrites a destination that another
  process created. Linux reports a filesystem without hard links as `EPERM`,
  which std maps to `PermissionDenied`, not `Unsupported`.

This probe proves both commit behaviors on the host:

```rust,run
use std::fs::{self, OpenOptions};
use std::hash::{BuildHasher, RandomState};
use std::io::{self, Write};

fn main() -> io::Result<()> {
    // RandomState starts from random keys, so these names are not predictable.
    let keys = RandomState::new();
    // create_dir fails when the name exists, so a planted directory or link is never used.
    let dir_name = format!("atomic-output-{:016x}", keys.hash_one("dir"));
    let dir = std::env::temp_dir().join(dir_name);
    fs::create_dir(&dir)?;
    let dest = dir.join("out.txt");
    fs::write(&dest, "old")?;

    let suffix = keys.hash_one("temp");
    let temp = dir.join(format!(".out.txt.{suffix:016x}.tmp"));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&temp)?;
    file.write_all(b"new")?;
    file.sync_all()?;
    drop(file);

    // No-clobber commit: the link fails and the old output survives.
    let error = fs::hard_link(&temp, &dest).expect_err("destination exists");
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read_to_string(&dest)?, "old");

    // Replace commit: rename replaces the existing destination.
    fs::rename(&temp, &dest)?;
    assert_eq!(fs::read_to_string(&dest)?, "new");
    fs::remove_dir_all(&dir)
}
```

## No-clobber output

When overwrite is not authorized, the commit operation must fail atomically if
the destination exists. A separate existence check followed by ordinary rename
has a race and can overwrite a file created between the two calls. Use the
workspace's verified no-replace primitive for the supported platform, or report
that atomic no-clobber output is unavailable. Add a concurrent creator test.

## Platform replacement

On Unix, a replace-capable rename can atomically replace a destination entry.
Decide whether replacing a symlink entry is acceptable; do not accidentally
follow it and overwrite its target.

On Windows, select one named replacement primitive for the supported Windows
and filesystem matrix. The `std::fs::rename` documentation names `MoveFileExW`
with a fallback to `SetFileInformationByHandle`, and Unix behavior on Windows 10
version 1607 and later when the filesystem supports `FileRenameInfoEx`. Verify
the existing-destination and documented failure states of the selected
primitive; do not infer them from a Unix rename or from another Windows API. If
no verified atomic replacement is available, fail before deleting the old file
and document that atomic overwrite is unsupported.

## Metadata policy

Define whether replacement preserves destination permissions, owner, ACLs,
extended attributes, streams, and timestamps. The result depends on the
platform and selected primitive. A Unix rename normally exposes the source
inode's metadata. A Windows replacement primitive can preserve or merge
destination metadata. Before commit, prove that the selected primitive
establishes the required final metadata and ACL. For sensitive output, it must
do so without a visibility window. If the ACL needs repair after publication,
fail before commit. Use a post-commit check only as defense in depth. Copy only
metadata required by the product. Do not inherit unsafe permissions from an
attacker-controlled file.

## `--force` and multi-file output

For `--force`, replace only the exact file type that the command documents.
Reject a directory. Treat a symbolic link, junction, or reparse point according
to an explicit policy. Encode the overwrite decision in the atomic commit
primitive when the directory is not trusted; a recheck alone does not close the
race.

Never claim a multi-file update is atomic because each file rename is atomic.
Use a manifest or generation directory when readers need one snapshot.
