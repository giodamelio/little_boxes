# Pool operations

The deployment-wide connection budget, pool controls, startup behavior, and
pool metrics behind the "Conditional references" table in `SKILL.md`. Engine and
crate defaults are in `engine-and-driver-notes.md`.

## Budget database connections

Calculate a deployment-wide budget before you set one process pool:

```text
usable connections = server maximum - admin reserve - migration reserve - other clients
per-instance maximum = floor(usable connections / maximum live instances)
```

Include rolling deployment overlap, autoscaling maximum, workers, scheduled
jobs, and local sidecars in `maximum live instances`. Keep an admin reserve so
an operator can connect during saturation. Keep a migration reserve when the
migration runner uses a separate connection.

Use a bounded admission limit before pool acquisition when the pool has no
bounded waiter queue. Reject overload before work holds memory or other scarce
resources.

Create one pool per database identity and process. Do not create a pool per
request, task, repository object, or transaction. Key a pool by every property
that changes routing or identity: endpoint, database, role, TLS identity, and
session policy.

Release a connection as soon as its database work ends. Do not hold a pooled
connection while you wait for HTTP, a message broker, user input, a long CPU
task, or a retry delay. Close the pool during graceful shutdown after admitted
database work finishes.

## Pool controls

Use these pool controls deliberately:

| Control | Rule |
|---|---|
| Maximum size | Fit the deployment-wide connection budget |
| Minimum idle | Keep small or zero unless cold-connect latency is measured |
| Acquire timeout | Fail before the caller deadline expires |
| Idle timeout | Reclaim unused sessions without connection churn |
| Maximum lifetime | Rotate sessions and credentials with bounded jitter |
| Connect timeout | Bound DNS, TCP, TLS, and database authentication |
| Health check | Reject a dead session before application work uses it |

Vary the maximum lifetime per instance when the pool has no jitter option.
Identical lifetimes can reconnect all instances at the same time. Keep the
driver's checkout health check: sqlx pings an idle connection before it returns
it (`test_before_acquire`, default `true`). Turn it off only when a measurement
shows that its cost matters. Do not add a second validation query on top of it.

## Startup

Bound the startup connection attempt. Fail startup when the service cannot work
without the database. Otherwise start and report not ready for
database-dependent traffic. Do not retry startup forever without a deadline or
a supervisor policy.

## Metrics

Measure checked-out, idle, and maximum connections; acquisition wait and
timeouts; overload rejections; connection creation, retirement, and failure
counts; query and transaction duration by bounded operation name; and commit,
rollback, conflict, and retry counts. Do not put SQL text, credentials, bind
values, or high-cardinality identifiers in metric labels.
