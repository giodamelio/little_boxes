# Rolling-compatible migrations

Migration history, serialization, and the rolling deployment order behind
the "Conditional references" table in `SKILL.md`. "Verify and finish"
is also a `SKILL.md` section.

## Deploy migrations in compatible phases

Use one migration history. Store checksums. Never edit a migration that ran in
any shared environment, because its checksum and applied state no longer match.
Add a forward migration that corrects it.

Serialize migration execution with one deployment job or a database advisory
lock. Do not let every application instance race to migrate on startup. Give
the runner a finite lock wait and statement timeout. Record the applied
version, checksum, start time, finish time, and failure.

Use this order for a rolling deployment:

1. **Expand:** add nullable columns, tables, indexes, or compatible constraints.
2. **Backfill:** update old rows in bounded batches that can resume.
3. **Switch:** deploy code that reads the new form and stops depending on the
   old form. Dual-write only when the transition needs it.
4. **Verify:** prove backfill completion and new-code adoption with queries and
   metrics.
5. **Contract:** remove old columns, constraints, indexes, and compatibility
   code in a later deployment.

Do not add a required column with no safe value while old binaries still write
the table. Do not rename or drop an object while old binaries still use it.
Keep the expand state compatible with both the old and new application.

Build large indexes with the engine's online or concurrent mode when required.
Put non-transactional DDL, such as PostgreSQL `CREATE INDEX CONCURRENTLY`, in
its own migration (sqlx: first line `-- no-transaction`) and make its restart
behavior explicit. Inspect invalid or partial artifacts after a failed run.
MySQL DDL commits implicitly, so a multi-statement MySQL migration is not
atomic.

For a destructive migration, verify a restore path and measure the lock and
rewrite cost on production-shaped data. A rollback file is not a backup. Data
removed by a down migration can be unrecoverable.

Use the repository's migration tool. The target rules in "Verify and finish"
apply to every mutating command. Read the
[sqlx section](engine-and-driver-notes.md#sqlx) of the reference
for the `sqlx migrate` commands when the repository uses sqlx.
