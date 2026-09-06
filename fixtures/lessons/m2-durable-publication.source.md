# Source: Durable Publication on a Crashing Machine

Reviewed source material for the lesson `m2-durable-publication`. Owner-authored for this
repository. The lesson's `source.content_hash` is the BLAKE3 of this file, and each segment's
`source_refs` name the blocks below.

Blocks are stable. Add a block rather than renumbering one, for the reason segment IDs are
stable: a renumbered block silently reassigns what an approved segment cited.

## block-001 — The problem

A program writes a file and the machine loses power in the middle. When it comes back, the file
is neither the old one nor the new one. It is a torn thing that has never been reviewed and that
every later step will trust, because a file on disk carries no record of whether the write that
produced it finished.

## block-002 — Why the write returning proves nothing

A successful `write` does not mean the bytes are on the storage device. The kernel copies them
into the page cache and returns; the device sees them later, at a time the kernel chooses. A
crash between those two moments loses the bytes that the program was already told it had written.

## block-003 — Ordering is not promised either

Nor is the order preserved. A filesystem is free to make later writes durable before earlier
ones. Two writes a program issued in an order it cared about can reach the device in the other
order, so a crash can leave a state the program never passed through.

## block-004 — Durable

A byte is durable when it will survive a power loss without further cooperation from the program
that wrote it. Durability is a property of bytes on a device, not of a function that returned.

## block-005 — Atomic

A change is atomic when every reader sees either all of it or none of it, never a partial state.
Atomicity is about what a concurrent or later reader can observe. Durability is about what
survives a crash. They are separate properties, and a correct publication needs both.

## block-006 — Two different failures

Torn content and torn visibility are two different failures. `fsync` addresses the first: it
returns once the file's bytes and metadata are on the device. It does nothing about the second,
because a reader can still open a file that is complete on disk but wrong for the name it holds.

## block-007 — Rename as the commit point

Within a single filesystem, `rename` replaces one directory entry with another in a single
metadata operation. A reader either resolves the name to the old inode or to the new one. There
is no instant at which the name resolves to something half-written, which is what makes rename
the commit point rather than the write.

## block-008 — The order

Publication is therefore four steps in one order: serialize into a sibling temporary file in the
destination's own directory; synchronize that file so its contents are durable; rename it over
the destination; synchronize the containing directory so the rename itself is durable. Then, and
only then, record that the change happened.

## block-009 — Why the file sync precedes the rename

The file is synchronized before the rename because the rename is what makes the file reachable
under the authoritative name. Renaming first would publish a name that resolves to content the
device may not hold yet, so a crash would leave the destination pointing at a file that is
missing its tail.

## block-010 — Why the directory sync follows

The rename returning is not the end. A rename changes a directory, and a directory is itself
data the kernel may hold in memory. Until the containing directory is synchronized, a crash can
leave the old name in place with the new file present but unreferenced. The change was atomic
and it was not yet durable.

## block-011 — The abandoned staging file

A crash before the rename leaves a staging file with a name nothing authoritative refers to. That
is a recoverable outcome rather than corruption: the destination still holds the previous
complete version, and the staging file can be inspected or discarded without consulting anything
else. This is the reason staging happens in the destination's own directory — `rename(2)` cannot
cross a filesystem boundary at all. It fails with `EXDEV` rather than copying, and the
copy-and-delete fallback a caller might reach for in its place is not atomic.

## block-012 — Publish-once against replace

Two publications with different rules. A cache entry is content-addressed, so its name already
claims what it holds: it must be published only if no one else got there first, which is
`RENAME_NOREPLACE`. A job document is authoritative state that legitimately moves forward, so it
is replaced. Choosing the replacing form for a cache entry would let a second writer overwrite
bytes a reader is already trusting under that name.

## block-013 — The event log comes last

A diagnostic event is appended after the authoritative state is durable, never before. An event
recording a state the system never reached is worse than no event, because recovery reads the log
to decide what happened.

## block-014 — Every crash window

Take the windows in turn. Before the file sync: the destination is unchanged. Between the file
sync and the rename: the destination is unchanged and a staging file remains. Between the rename
and the directory sync: either outcome, both of them whole. After the directory sync: the new
version, durable. No window yields a mixture.

## block-015 — What the guarantee rests on

The guarantee is filesystem-specific. Rename is atomic within one filesystem, so a destination
and its staging file must live on the same one. Recovery guarantees here are claimed for the
qualified Linux filesystem and not for a mount that emulates it.

## block-016 — The compressed rule

Stage beside the destination, synchronize the file, rename, synchronize the directory, then log.

## block-017 — What the discipline buys

The result is that a machine that dies at an arbitrary instant comes back holding one of exactly
two reviewed states. Recovery does not need a repair pass, because there is nothing to repair.

## block-018 — Where it is written down

The ordering is fixed by `docs/adr/ADR-0001-production-rust-study-guide-tts.md` §12.3 and
implemented by `crates/study-tts-runtime/src/durable.rs`, which names that section in return.
