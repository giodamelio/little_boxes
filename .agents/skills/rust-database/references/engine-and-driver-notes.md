# Engine and driver notes

Engine and crate facts behind the rules in `SKILL.md`. Read only the sections for
the engine and crates that `Cargo.lock` names. The facts were checked in 2026-09
against PostgreSQL 18 docs, MySQL 8.4 docs, SQLite docs, sqlx 0.9.0,
tokio-postgres 0.7.18, and deadpool 0.13.1. Re-check a row after a major upgrade.

Contents:

- [PostgreSQL](#postgresql)
- [MySQL (InnoDB)](#mysql-innodb)
- [SQLite](#sqlite)
- [sqlx](#sqlx)
- [tokio-postgres](#tokio-postgres)
- [deadpool](#deadpool)

## PostgreSQL

Server-side limits:

| Setting | What it limits | When it fires | SQLSTATE |
|---|---|---|---|
| `statement_timeout` | One statement | Cancels the statement; an open transaction becomes aborted | `57014` `query_canceled` |
| `lock_timeout` | Each lock wait | Cancels the statement | `55P03` `lock_not_available` |
| `idle_in_transaction_session_timeout` | Client idle inside an open transaction | Terminates the session | `25P03` |
| `transaction_timeout` (17 and later) | Whole transaction, explicit or implicit | Terminates the session | `25P04` |

- Set `lock_timeout` below `statement_timeout`. The docs call an equal or larger
  value "rather pointless", because the statement timeout fires first.
- A client cancel request (`pg_cancel_backend`, a driver cancel token) also
  ends the statement with `57014`.
- Set a constant transaction-local limit with a static
  `SET LOCAL statement_timeout = '2s'` as the first statement of the
  transaction. `SET` takes no bind parameter, so use
  `SELECT set_config('statement_timeout', $1, true)` when the value varies. The
  third argument `true` limits the value to the current transaction.
- Set `idle_in_transaction_session_timeout` per role as a backstop for a
  transaction that a client leaked. Do not set `statement_timeout`,
  `lock_timeout`, or `transaction_timeout` in `postgresql.conf`; they then
  apply to every session, including migrations.

```sql
ALTER ROLE app_rw SET idle_in_transaction_session_timeout = '60s';
```

- A session-terminating limit closes the connection. The pool finds it at the
  next health check or first use. Expect that error and let the pool replace
  the connection.
- Retry `40001` `serialization_failure` and `40P01` `deadlock_detected` by
  default. `SKILL.md` names the opt-in cases. Do not match class `40` by prefix: it also holds `40002`
  `transaction_integrity_constraint_violation` and `40003`
  `statement_completion_unknown`.
- `CREATE INDEX CONCURRENTLY` cannot run inside a transaction block. A failed
  concurrent build leaves an `INVALID` index that queries ignore but writes
  still maintain. Find it, drop it, and rerun the build:

```sql
SELECT indexrelid::regclass FROM pg_index WHERE NOT indisvalid;
DROP INDEX CONCURRENTLY index_name;
```

## MySQL (InnoDB)

| Error | SQLSTATE | What InnoDB rolled back | Action |
|---|---|---|---|
| `1213` `ER_LOCK_DEADLOCK` | `40001` | The whole transaction | Retry the complete transaction |
| `1205` `ER_LOCK_WAIT_TIMEOUT` | `HY000` | Only the statement that waited, by default (`innodb_rollback_on_timeout=OFF`); the whole transaction when it is `ON` | Roll back the transaction. Report the error by default; retry only inside the remaining caller deadline and attempt limit |

- After `1205` with the default setting, earlier writes of the transaction are
  still pending. Do not commit them by accident on the error path.
- DDL statements commit implicitly. A migration file with several DDL
  statements is not atomic. Put one DDL statement in each migration, or make
  every step safe to rerun.

## SQLite

- `SQLITE_BUSY` means another connection holds a conflicting lock. Retry the
  complete transaction.
- `SQLITE_LOCKED` means a conflict inside the same connection. It points to a
  code defect, such as an unfinished statement, not to contention.
- With shared cache, `SQLITE_LOCKED_SHAREDCACHE` is contention between
  connections. sqlx uses shared cache for a `sqlite::memory:` or `mode=memory`
  pool. sqlx 0.9 waits on this code only with the `sqlite-unlock-notify`
  feature. The `sqlite` feature enables it; `sqlite-bundled` or
  `sqlite-unbundled` alone does not.
- A deferred transaction (plain `BEGIN`) that reads and then writes can get
  `SQLITE_BUSY` at once. SQLite skips the busy handler, and so `busy_timeout`,
  when waiting could deadlock. In WAL mode the extended code is
  `SQLITE_BUSY_SNAPSHOT`. Begin a transaction that will write with
  `BEGIN IMMEDIATE`: rusqlite
  `conn.transaction_with_behavior(TransactionBehavior::Immediate)`, sqlx
  `conn.begin_with("BEGIN IMMEDIATE")`.

## sqlx

sqlx 0.9.0 needs Rust 1.94.0 or later.

| Item | Fact | Action |
|---|---|---|
| Drop during `begin()` | Before 0.9.0, a transaction dropped while `BEGIN` ran could stay open and return to the pool (sqlx PR #3980). | Use 0.9.0 or later when `begin()` can be cancelled. If the MSRV blocks the upgrade, do not put `begin()` inside a cancellable future. |
| `Pool::close` | Since 0.9.0 it closes every connection before it returns (sqlx PR #3952). Earlier versions could return first. | On older versions, do not assume the server saw every session close when `close().await` returns. |
| Dynamic SQL | `query*()` takes `impl SqlSafeStr`. `QueryBuilder::new` takes `impl Into<String>` with no such check. | Follow "Keep SQL text static" in `SKILL.md`. `QueryBuilder::new` and `push` do not escape; `push_bind` binds. |
| Cargo features | Combined runtime and TLS features such as `runtime-tokio-rustls` are gone. | Use `runtime-tokio` plus one TLS feature: `tls-rustls-ring-webpki`, `tls-rustls-ring-native-roots`, `tls-rustls-aws-lc-rs`, or `tls-native-tls`. `tls-rustls` means ring with webpki roots. |
| `sqlx.toml` | Per-crate config can rename `DATABASE_URL` and the migrations table. The library needs the `sqlx-toml` feature; `sqlx-cli` enables it by default. | Read `sqlx.toml` before you assume the variable name. |
| CLI install | The repository no longer tracks `Cargo.lock`, but the published sqlx-cli 0.9.0 crate still ships one. The changelog note that `--locked` "will no longer work" does not apply to this crate. | Pin the CLI with `cargo install --locked sqlx-cli --version 0.9.0`. |

Pool defaults from `PoolOptions::new()`:

| Option | Default | Note |
|---|---|---|
| `max_connections` | 10 | Set it from the connection budget |
| `min_connections` | 0 | |
| `acquire_timeout` | 30 s | Covers the permit wait, the health check, and a new connection; set it below the caller deadline |
| `idle_timeout` | 10 min | |
| `max_lifetime` | 30 min, no jitter | Vary it per instance when many connections open at the same time |
| `test_before_acquire` | `true` | Pings an idle connection before `acquire()` returns it |

On release, the pool pings the connection to flush queued work such as a
rollback, and closes the connection when the ping fails. A dropped
`Transaction` queues its rollback for the next use of the connection.

Migrations:

- Each migration file runs in a transaction unless its first line is
  `-- no-transaction`. Use that line for `CREATE INDEX CONCURRENTLY`, with one
  statement in the file.
- `Migrator` takes a database lock by default. Keep it. `set_locking(false)` is
  only for an engine without advisory locks.
- The CLI reads `DATABASE_URL` from the environment or `.env`, so the
  `--database-url` flag is not needed.

```bash
sqlx migrate info
sqlx migrate run
cargo sqlx prepare --workspace --check
```

`sqlx migrate run` changes the database; the target rules in `SKILL.md` apply.
`prepare --check` needs `cargo install --locked sqlx-cli`. As of sqlx-cli 0.9.0 it
connects to `DATABASE_URL` first, then runs `cargo check` with `SQLX_OFFLINE=false`, so
the query macros run against that live database: apply every migration there first
(`sqlx migrate run` on a disposable database). It exits with 1 when the `.sqlx` query
metadata needs an update; a connection error is a setup failure, not stale
metadata.

## tokio-postgres

- Take `client.cancel_token()` before a long query. Call `cancel_query(tls)`
  on the token from the deadline path; it opens a new connection to the
  server. The docs state that the server gives no success result and that
  cancellation "is inherently racy". The cancelled statement ends with `57014`.
- `Error::code()` returns `Option<&SqlState>`. Compare it with
  `SqlState::T_R_SERIALIZATION_FAILURE` and `SqlState::T_R_DEADLOCK_DETECTED`.

## deadpool

`PoolConfig::default()` in deadpool 0.13.1:

- `max_size` is twice `std::thread::available_parallelism()`: the logical CPU
  count, or the container CPU quota. 0.12.3 to 0.13.0 used
  `num_cpus::get() * 2`; earlier versions used four times the physical core
  count. A larger host opens more connections with no configuration change.
  Set it from the connection budget.
- `timeouts` has no values, so `Pool::get()` can wait without a limit. Set
  `wait`, `create`, and `recycle`. The builder then needs a runtime
  (`.runtime(Runtime::Tokio1)`, feature `rt_tokio_1`); otherwise `build()`
  fails with `BuildError::NoRuntimeSpecified`.
