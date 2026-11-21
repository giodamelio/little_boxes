# Cancellation controls

Deep material for `rust-database`, "Make cancellation safe". Read it when you set the statement,
lock-wait, or transaction timeouts, or send a server-side cancel.

## The controls

SKILL.md holds the caller deadline, the statement timeout, and the dirty-connection rule. Add:

- A lock-wait timeout below the statement timeout when lock waits must fail separately.
- A transaction lifetime inside the deadline. PostgreSQL 17 adds `transaction_timeout`; it
  terminates the session, so expect a dead pooled connection after it fires.

Send a server-side cancel explicitly (tokio-postgres `Client::cancel_token()`)
when a long statement must stop, unless the driver documents that a drop sends
one. Keep the statement timeout as the real bound: the server does not report
whether a cancel worked.

Set transaction-local timeouts where possible. A session-wide timeout can leak
into the next pool user when reset fails. If the driver requires a session
setting, set it on checkout and reset or discard the session before check-in.
