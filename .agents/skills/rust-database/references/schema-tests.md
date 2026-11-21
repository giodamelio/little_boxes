# Schema integration tests

Test database setup, the minimum CI matrix, and cancellation test points behind
the "Conditional references" table and "Make cancellation safe" in `SKILL.md`.

## Test the real schema

Run database integration tests against the same engine and major version as
production. An in-memory substitute does not prove SQL syntax, isolation,
locking, collation, extensions, or migration behavior.

Use an isolated database or schema per test worker. Do not rely only on
transaction rollback for test isolation. It cannot model code that opens
another connection, commits internally, or tests migrations.

Do not mark a database test as flaky because it exposed an unbounded retry,
missing lock, shared schema, or unsafe cleanup.

## Isolated test databases

Use an isolated database or schema per test worker. Give each one a generated,
bounded identifier. Apply migrations from an empty database. Run application
queries through the production repository or data-access code. Drop the test
database or schema after the pool closes.

## Minimum CI matrix

- migrate an empty database to the current version;
- start from the previous released schema and migrate to the current version;
- run repository integration tests on the migrated schema;
- run one concurrent test for each protected business invariant;
- inject one serialization conflict or deadlock and prove bounded retry;
- cancel a transaction and prove rollback and pool recovery;
- run a schema or query metadata freshness check when the driver supports it
  (sqlx: `cargo sqlx prepare --workspace --check` against a migrated
  `DATABASE_URL`, which exits 1 when `.sqlx`
  is stale).

## Cancellation test points

Test cancellation at these points:

- while waiting for a pool connection;
- while `BEGIN` runs;
- while waiting for a row or advisory lock;
- during a long statement;
- after writes but before commit;
- while commit is in flight.

After each test, prove that the pool regains capacity and that a new transaction
can execute. Also prove the expected rows and outbox records.
