---
description: Use when building or reviewing production Rust database code (sqlx, diesel, tokio-postgres, rusqlite, deadpool, bb8) for database pool exhaustion, transaction rollback, transaction cancellation safety, a serialization failure or deadlock retry, SQLITE_BUSY, SQL injection or dynamic SQL with QueryBuilder, database migration ordering in a rolling deploy, a schema integration test, or a sqlx 0.9 upgrade that hits SqlSafeStr or AssertSqlSafe.
license: BSD-3-Clause
metadata:
    github-path: skills/rust-database
    github-pinned: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-ref: 0d0b9462c8f51532253af6e20458d934b24c8bf7
    github-repo: https://github.com/po4yka/rust-skills
    github-tree-sha: 6d5526611f5673677ec3b4fbb9bc3e22afe18384
name: rust-database
---
# Rust Database

Keep the database, driver, and migration tool that the workspace already uses,
with their native pool and transaction types.

When installed, `rust-networking` owns HTTP retries and `rust-observability`
owns telemetry. `rust-async-internals` owns task ownership;
to keep a commit sequence atomic, run it in `tokio::spawn` and join the handle.

Read the section of
[references/engine-and-driver-notes.md](references/engine-and-driver-notes.md)
for the locked engine and crates when you set a pool option, a timeout, a retry
code, or a migration command.

## Inspect the existing contract

Find the database boundary before you change it:

```bash
rg -g 'Cargo.toml' -g 'Cargo.lock' \
  'sqlx|diesel|tokio-postgres|rusqlite|deadpool|bb8|r2d2|sea-orm' .
rg 'begin\(|begin_with\(|transaction\(|commit\(|rollback\(|acquire\(' --glob '*.rs' .
rg 'CREATE TABLE|ALTER TABLE|CREATE INDEX|migrate' --glob '*.sql' --glob '*.rs' .
rg -U -i 'AssertSqlSafe|sql_query\(|format!\(\s*r?#*"\s*(select|insert|update|delete|with)\b' --glob '*.rs' .
```

Record these facts before you change pool, transaction, retry, or migration
policy.

| Fact | Required answer |
|---|---|
| Database | Engine and production major version |
| Driver | Crate, version, runtime, and TLS backend |
| Pool owner | Process object that creates and closes the pool |
| Connection budget | Server limit, reserved capacity, instance count, pool maximum |
| Acquisition policy | Queue bound, acquisition timeout, overload result |
| Query policy | Operation deadline, server-side statement limit |
| Transaction policy | Isolation, retryable errors, maximum attempts |
| Migration owner | One job or process that serializes migrations |
| Deployment policy | Expand, migrate data, switch code, contract |
| Test engine | The production engine major version |

Do not infer a safe pool size or wait from a library default: a deadpool pool
has no wait timeout, so `Pool::get()` can wait forever; set `timeouts.wait`
below the caller deadline. Do not infer transaction semantics from an ORM
method name.

## Triage failures

| Symptom | Likely cause | First proof | Fix |
|---|---|---|---|
| Pool acquisition timeout rises | Connections held too long, or budget too small | Compare checked-out count, wait, and query duration | Shorten hold scope, bound admission, recalculate the budget |
| Database rejects new sessions during deploy | Rolling instances exceed the budget | Count pools across old and new instances | Lower the per-instance maximum or the overlap |
| Pool stays smaller after cancellation | A cancelled query or rollback still owns a connection | Cancel at each phase; watch capacity | Await cleanup or discard the connection |
| Writes succeed but later vanish, with no error | An open transaction returned to the pool (sqlx before 0.9.0, drop during `BEGIN`) | `cargo tree` shows sqlx-core below 0.9.0; `pg_stat_activity` shows `idle in transaction` | Upgrade to sqlx 0.9.0, or stop cancelling `begin()` |
| Rows changed although caller saw an error | Unknown commit outcome | Query by stable operation ID | Reconcile; do not repeat blindly |
| Deadlocks repeat on every attempt | Lock order differs, or retry is immediate | Capture operation names and lock order | Stable lock order, jittered bounded retry |
| Serialization failures surge | Contention or transaction scope grew | Compare conflict rate, duration, touched rows | Shorten scope, reduce hot rows |
| SQLite returns `SQLITE_BUSY` at once despite `busy_timeout` | A deferred transaction tried to upgrade from read to write | Check how the transaction begins | Begin write transactions with `BEGIN IMMEDIATE` |
| Migration blocks application traffic | DDL takes a strong lock or rewrites data | Measure on production-shaped data | Split the change; use online or batched operations |
| `CREATE INDEX CONCURRENTLY cannot run inside a transaction block` | The migration tool wraps the file in a transaction | Read the migration tool's transaction rule | Move the statement to its own non-transactional migration |
| New binary fails before migration completes | Incompatible deploy order | Run old and new binaries on the expand schema | Restore expand-switch-contract order |
| Migration passes locally but fails in CI | Test engine or extension differs | Compare engine version and extensions | Match the production engine |
| Tests fail only in parallel | Workers share a schema or exceed the budget | Run with one worker | Isolate schemas, bound parallelism |

## Verify and finish

Run the test target while you iterate, and the rest once before merge.

```bash
cargo test --locked --test <db-test-target> -- --test-threads=4
cargo sqlx prepare --workspace --check   # sqlx only
cargo tree --locked -i sqlx-core --depth 0   # sqlx only
cargo deny --config deny.toml --locked check advisories
```

- The worker count is a connection-budget input. A green run does not prove
  lock duration or rewrite cost on production volumes.
- `prepare --check` needs `sqlx-cli` and a `DATABASE_URL` with every migration
  applied. It exits 1 when `.sqlx` is stale.
- `cargo tree` checks the sqlx 0.9.0 floor of "Make cancellation safe".
- `cargo deny` needs `unsound = "all"`; without it, run `cargo audit --deny
  warnings`.

Run a mutating migration command (`sqlx migrate run`, a down migration, or a
DDL script) only against a disposable test database or an environment the user
has authorized, because a migration can destroy data. Do not print
`DATABASE_URL` or pass it as a command-line argument.

Before merge, confirm each item:

- [ ] The budget includes rolling overlap, and every wait is finite.
- [ ] Each section below holds: transaction scope, cancellation, retry, static SQL.
- [ ] Isolation follows a written invariant.
- [ ] Migrations are serialized and rolling-compatible. Destructive or non-transactional DDL has a recovery plan.
- [ ] CI on the production engine covers empty and previous-release schemas, conflicts, cancellation, pool recovery.

Report the versions, pool calculation, transaction and retry policy, migration
phases, commands run, and what stays unverified.

## Keep SQL text static

Bind every value. Do not build SQL text from untrusted input with `format!`.

sqlx 0.9 `query*()` functions take `impl SqlSafeStr`, which a runtime `String`
does not implement, so `query(&sql)` stops compiling after the upgrade. Do not
wrap the string in `AssertSqlSafe` only to make it compile: the wrapper asserts
safety without proof. Build dynamic SQL with `QueryBuilder`, which does not
escape: start from a static string, `push_bind` every value, and `push` only a
fragment from a fixed allowlist, such as a column name. Give every remaining
`AssertSqlSafe(` and diesel `sql_query(` a comment that names its text source.

## Keep one transaction in one async scope

The function that starts a transaction owns it until commit or rollback; pass
`&mut Transaction` to helpers. Do not put a live transaction in shared state,
move it into a detached task, or return it across an API boundary that does not
only compose database operations in the same task.

```text
begin
  read and lock the rows required by the invariant
  validate the invariant
  write all related rows
  write an outbox record when an external effect must follow commit
commit
publish or perform the external effect from the committed outbox
```

Commit explicitly on the success path, return the commit error, and do not
report success before commit succeeds. Do not assume that `Drop` performs a
synchronous rollback: many async drivers queue it, and sqlx runs it on the next
use of the connection. Test the drop behavior of the selected driver.

Keep every domain invariant in one transaction or in a database constraint. Do
not perform an irreversible external effect inside a transaction: the database
can roll back after the effect succeeds. Use a transactional outbox with an
idempotent consumer.

Treat a lost connection during `COMMIT` as an unknown outcome: the server might
have committed. Do not retry blindly. Reconcile by a stable operation ID, an
idempotency row, or a domain read that can prove the outcome.

## Make cancellation safe

A dropped query future does not prove that the database stopped the statement.
Confirm what the locked driver does.

Require sqlx 0.9.0 or later when a timeout, `select!`, or an aborted request
can cancel `begin()`: before it, a transaction dropped during `BEGIN` could
return to the pool open, and later writes on it were silently lost.

Carry one caller deadline into pool acquisition and database work, and set a
server-side statement timeout below it. Do not let a connection serve another
caller while a cancelled statement or a queued rollback can still produce
protocol messages: discard it when the driver does not drain, roll back, or
close it. Read [references/cancellation-controls.md](references/cancellation-controls.md)
for lock-wait and transaction timeouts, the server-side cancel, and session
settings that leak into the next pool user.

Cancellation before commit must leave no partial state; during commit it is an
unknown outcome, so keep the operation ID after the future drops. Test every
phase, from the pool wait to a commit in flight
([references/schema-tests.md](references/schema-tests.md) lists the points).

## Retry the complete transaction

Retry only an error that proves the current transaction did not commit and that
the database defines as transient: serialization failure and deadlock victim.
Match exact structured codes, not message text and not a class prefix
(PostgreSQL `40003` means statement completion unknown). Report a lock-wait
timeout by default. Read [references/isolation-and-retry.md](references/isolation-and-retry.md) for the codes of each
engine and a runnable classifier.

Retry the complete transaction closure with a new transaction, never only the
failed statement. Keep the same operation ID and input, and re-read database
state on every attempt.

Never retry automatically, because the outcome is permanent, the caller gave
up, or a repeat can apply the effect twice: a domain constraint violation;
invalid SQL, data, permission, or schema; cancellation, a statement timeout
(PostgreSQL `57014`), or an exhausted deadline; an unknown commit outcome; a
transaction that performed a non-idempotent external effect.

Count the first execution as attempt one. Use a small attempt maximum, the
operation deadline, and jittered exponential backoff. Release the transaction
and its connection before the delay, and bound retries across the process. Keep
the classifier pure. A lost connection has no SQLSTATE: classify it at the call
site, where you know whether `COMMIT` was in flight.

## Conditional references

| Read | When |
|---|---|
| `references/migrations.md` | You write, order, or run a schema migration |
| `references/pool-operations.md` | You size a pool or set its timeouts, lifetime, health checks, startup, or metrics |
| `references/isolation-and-retry.md` | You choose an isolation level, a constraint, or a lock order for an invariant |
| `references/schema-tests.md` | You write a database integration test, set up test databases, or build the CI matrix |
