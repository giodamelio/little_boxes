# Cancellation at the network boundary

Request states and cancellation rules that the "Conditional references" table
in `SKILL.md` points to.

## Define cancellation at the network boundary

Cancellation answers what the local caller does. It cannot prove what a remote
server did. After any request bytes reach the network, a state-changing result
can be ambiguous.

Return an error variant that distinguishes these states when the application
needs recovery:

| State | Meaning |
|---|---|
| Not sent | No request bytes reached the transport |
| Outcome unknown | Some bytes were sent, but no final response arrived |
| Responded | A final response arrived |

Do not turn `Outcome unknown` into a safe retry. Require idempotency or an
application reconciliation step.

On client cancellation, stop producing the request body and release the
response body. Let the protocol stack send the correct stream reset or close.
On server cancellation, stop application work promptly, but do not claim that
a partial response can be withdrawn from the peer.

Use `rust-async-internals`, when it is installed, to implement the cancellation
tree. At minimum, give every spawned task an owner that cancels its
`CancellationToken` and then joins its `JoinHandle`, and verify that dropped
futures leave no locks, permits, or transactions behind.

## Stop reading and drop uncertain connections

Stop reading after cancellation, a body-limit error, or an expired deadline.
Release or reset the protocol stream as the library requires. Do not return a
connection to the pool when its protocol state is uncertain.
