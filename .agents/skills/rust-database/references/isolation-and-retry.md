# Isolation and retry classification

Isolation choice, controls for common invariants, and a runnable retry
classifier, behind "Retry the complete transaction" and the "Conditional
references" table in `SKILL.md`.

Contents: choose isolation from the invariant; controls for common invariants; retry codes
by engine; PostgreSQL retry classifier.

## Choose isolation from the invariant

Write the invariant and the conflicting schedule before you choose an
isolation level. Use the lowest level that the database documents as sufficient
for that schedule. Prefer a unique, foreign-key, exclusion, or check
constraint, or an explicit lock, when it makes the invariant simpler and
cheaper.

Do not use `SELECT` followed by `UPDATE` as concurrency control at read
committed isolation unless a constraint, row lock, or compare-and-swap predicate
closes the race, because two transactions can pass the same check at the same
time.

Acquire multiple locks in a stable order. Keep lock scopes short. A deadlock can
still occur through another code path, so classify and handle the database's
deadlock-victim error.

## Controls for common invariants

| Invariant or operation | Typical control |
|---|---|
| One value must stay unique | Unique constraint, then classify its violation |
| Update only the version read | Version column in the `WHERE` clause |
| Update a known row set | Row locks in a stable order |
| Protect a missing row or range | Serializable isolation or engine-specific range lock |
| Maintain a cross-row predicate | Serializable isolation or a matching constraint |
| Claim queued jobs once | Atomic update or locking read with explicit skip policy |

## Retry codes by engine

Retryable examples: PostgreSQL SQLSTATE `40001` and `40P01`, MySQL error `1213`
(SQLSTATE `40001`), and SQLite `SQLITE_BUSY`.

Report a lock-wait timeout (PostgreSQL `55P03`, MySQL `1205`) by default. Retry
it only after a full rollback, inside the remaining caller deadline and attempt
limit. MySQL `1205` rolls back only the statement by default
(`innodb_rollback_on_timeout=OFF`), so roll back the transaction before you act
on it. PostgreSQL documents that `23505` or `23P01` can be a serialization
failure when the transaction derived the key from its own reads. Let only that
call site opt into a retry.

Get the code with `err.as_database_error().and_then(|e| e.code())` in sqlx, or `err.code()` in
tokio-postgres.

## PostgreSQL retry classifier

The classifier and the retry decision are pure functions, so a unit test covers
them without a database. The classifier matches exact SQLSTATE codes. Only a
bucket that is never retried may match a class prefix.

```rust,run
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DbFailure {
    Serialization,
    DeadlockVictim,
    Constraint,
    Timeout,
    CommitOutcomeUnknown,
    Cancelled,
    Other,
}

fn classify_postgres(sqlstate: &str) -> DbFailure {
    match sqlstate {
        "40001" => DbFailure::Serialization,
        "40P01" => DbFailure::DeadlockVictim,
        "57014" | "55P03" | "25P03" | "25P04" => DbFailure::Timeout,
        // A prefix is safe only for a bucket that is never retried.
        code if code.starts_with("23") => DbFailure::Constraint,
        _ => DbFailure::Other,
    }
}

fn can_retry(failure: DbFailure, attempt: u8, maximum_attempts: u8) -> bool {
    attempt < maximum_attempts
        && matches!(failure, DbFailure::Serialization | DbFailure::DeadlockVictim)
}

fn main() {
    assert_eq!(classify_postgres("40001"), DbFailure::Serialization);
    assert_eq!(classify_postgres("40003"), DbFailure::Other);
    assert_eq!(classify_postgres("23505"), DbFailure::Constraint);
    assert_eq!(classify_postgres("57014"), DbFailure::Timeout);
    assert!(can_retry(DbFailure::Serialization, 1, 3));
    assert!(can_retry(DbFailure::DeadlockVictim, 2, 3));
    assert!(!can_retry(DbFailure::Serialization, 3, 3));
    assert!(!can_retry(DbFailure::Constraint, 1, 3));
    assert!(!can_retry(DbFailure::Timeout, 1, 3));
    assert!(!can_retry(DbFailure::CommitOutcomeUnknown, 1, 3));
    assert!(!can_retry(DbFailure::Cancelled, 1, 3));
}
```

`CommitOutcomeUnknown` and `Cancelled` have no SQLSTATE. The call site sets
them, because only it knows whether `COMMIT` was in flight or the caller gave
up.
