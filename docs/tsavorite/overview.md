# tsavorite overview

Tsavorite is a **hash index over a hybrid log**. The index maps a hash to the
address of the newest record; records chain backward; the log is one address
space whose tail is mutable RAM, middle is immutable RAM, and oldest part is on
disk. Hot records are updated in place, everything else is copied to the tail.
Cold reads are asynchronous and the caller resumes them.

> Source: `microsoft/garnet` @ `c9607605baa4`, `libs/storage/Tsavorite/cs/src/core/`.
> Reference-only. See [`../design/`](../design/) for what we build.

---

## 1. the index points into the log; keys live in the log

```mermaid
flowchart LR
    K[key] --> H[hash]
    H --> B["bucket = hash & mask<br/>slot = matching 15-bit tag"]
    B --> A[48-bit address]
    A --> R[record in log]
    R --> C{key matches?}
    C -- yes --> V[value]
    C -- no --> P[follow PreviousAddress]
    P --> R
```

An index entry is one 8-byte word: `[tentative:1][tag:15][address:48]`. A bucket
is one 64-byte cache line: 7 entries + 1 overflow/latch word. The index never
stores a key and is sized once, up front.

## 2. one chain, two jobs

```mermaid
flowchart LR
    I["index slot<br/>tag 0x1a3"] --> D["D  key=foo  v3"]
    D --> C["C  key=bar  v1"]
    C --> B["B  key=foo  v2"]
    B --> A["A  key=foo  v1"]
    A --> X((INVALID))
```

`PreviousAddress` links every record to the prior record with the same tag. The
chain holds **old versions** (B, A) and **other keys** (C). Newest is at the head,
so `get(foo)` stops at D and never sees B or A. Delete appends a tombstone.

## 3. one address space, three regions

```mermaid
flowchart LR
    Begin([Begin]) --> D["on device<br/>fetched by async I/O"]
    D --> Head([Head])
    Head --> I["immutable RAM<br/>being flushed"]
    I --> RO([ReadOnly])
    RO --> M["mutable RAM<br/>in-place updates OK"]
    M --> Tail([Tail])
```

Addresses are 48-bit logical offsets that never change. Where an address falls
relative to `Head` and `ReadOnly` decides how an operation treats the record.

## 4. hot in place, cold by copy

```mermaid
flowchart TD
    U[upsert / rmw] --> F[find newest record for key]
    F --> W{region?}
    W -- absent --> N[append new record at Tail]
    W -- "mutable, fits" --> IP[update in place]
    W -- immutable --> CP["copy-update:<br/>append new version at Tail,<br/>previous = old address"]
    W -- on device --> K{op?}
    K -- upsert --> N
    K -- rmw --> IO[read record from device,<br/>then copy-update]
    N --> CAS[CAS index slot → new address]
    CP --> CAS
    IO --> CAS
```

Upsert never reads the old value, so it never touches the device. RMW must.
New records are written invalid, published by CAS, then marked valid; a lost CAS
retries.

## 5. memory is a ring holding the newest pages

```mermaid
stateDiagram-v2
    [*] --> Mutable: Tail enters page
    Mutable --> ReadOnly: mutable window exceeded
    ReadOnly --> Flushing: epoch drain (SafeReadOnly)
    Flushing --> Flushed: write done, FlushedUntil advances
    Flushed --> Evicted: ring needs slot, Head advances
    Evicted --> [*]: epoch drain, slot reused
```

Page `p` lives in slot `p % ring_size`, so RAM always holds a contiguous suffix of
the log. Old pages are never paged back in; a cold read fetches one record. To
make a cold record hot again, copy it to the tail (or into the read cache, a
second memory-only log spliced into the same chains).

## 6. cold reads are pending; the caller resumes them

```mermaid
sequenceDiagram
    participant C as caller thread
    participant E as engine
    participant D as device
    C->>E: Read(key)
    E->>E: index → address below Head
    E->>D: ReadAsync(one record)
    E-->>C: Status.Pending
    Note over C: keeps issuing other ops
    D-->>E: completion → session ready queue
    C->>E: CompletePending()
    E->>E: re-walk chain from current head<br/>newer version? use it<br/>tag collision? reissue
    E-->>C: Reader(record), output
```

No user code runs on the I/O thread. Every completion is re-validated against
the live index because another thread may have appended a newer version while
the read was in flight.

## 7. epochs make boundary moves safe

```mermaid
sequenceDiagram
    participant T1 as thread 1
    participant T2 as thread 2
    participant E as epoch manager
    T1->>E: enter (epoch 7)
    T2->>E: enter (epoch 7)
    Note over T1: holds pointer into page P
    E->>E: ReadOnly moves past P<br/>bump → 8, defer "flush P" until 7 is safe
    T2->>E: exit, re-enter (epoch 8)
    T1->>E: exit
    E->>E: no one in 7 → run "flush P"
```

Every operation runs inside an epoch. Anything that could invalidate a pointer
another thread holds (advancing `ReadOnly`, evicting a page, reusing a slot,
switching checkpoint phase) is a deferred action that runs after all threads
leave the old epoch. Threads run deferred actions cooperatively on entry.

## 8. concurrency: CAS to publish, short latches to mutate

| mechanism | scope | for |
|---|---|---|
| CAS on index entry | one slot | publishing a new chain head |
| bucket latch (in the 8th word) | one bucket | protecting in-place update / RMW |
| `Sealed` bit on record | one record | telling late writers "replaced, retry at new head" |
| hashed lock table | one key | multi-key transactions |

Not lock-free, deliberately: in-place RMW on a shared record needs exclusion.

## 9. checkpoint = version bump while running

```mermaid
flowchart LR
    R["REST (v)"] --> P[PREPARE] --> IP["IN_PROGRESS (v+1)<br/>sessions switch on next epoch refresh"] --> WF[WAIT_FLUSH] --> M[persist metadata] --> R2["REST (v+1)"]
```

Records written under `v+1` carry a version bit; recovery to `v` drops them.
Fold-over checkpoints flush the mutable region; snapshot checkpoints copy it to a
separate file and keep it mutable. Recovery rebuilds the index by scanning the
log (or loads an index snapshot). Durability is per checkpoint, not per write.

## 10. the user supplies the operation, the engine supplies atomicity

```mermaid
flowchart LR
    subgraph engine
        F[find] --> L[latch / seal] --> AL[allocate if copying] --> CB[callback] --> CAS[CAS publish] --> POST[post-callback]
    end
    subgraph user
        CB -.-> R[Reader]
        CB -.-> W[InitialWriter / InPlaceWriter]
        CB -.-> U[InitialUpdater / InPlaceUpdater / CopyUpdater]
        CB -.-> S[size functions]
    end
```

The engine hands the callback the record memory (or the source and a freshly
allocated destination). Post-callbacks run only after the CAS succeeded, so side
effects are never performed for lost races.

---

## what it is not

- **Not ordered.** No range scans; the index is a hash.
- **Not a page cache.** Memory holds the newest suffix; misses read one record.
- **Not per-write durable.** Durable at checkpoints; Garnet adds an AOF on top.
- **Not lock-free.** CAS for publication, latches for mutation.

## where to look

| idea | file (under `cs/src/core/`) |
|---|---|
| index entry, bucket | `Index/Tsavorite/HashBucketEntry.cs`, `HashBucket.cs` |
| chain walk | `Index/Tsavorite/Implementation/FindRecord.cs` |
| record header | `Index/Common/RecordInfo.cs`, `Allocator/RecordDataHeader.cs` |
| ring, boundaries, flush | `Allocator/AllocatorBase.cs`, `Index/Tsavorite/LogAccessor.cs` |
| read / upsert / rmw | `Index/Tsavorite/Implementation/Internal{Read,Upsert,RMW}.cs` |
| pending I/O | `Implementation/ContinuePending.cs`, `Index/Tsavorite/TsavoriteThread.cs` |
| epochs | `Epochs/LightEpoch.cs` |
| checkpoint | `Index/Checkpointing/StateMachineDriver.cs`, `Index/Recovery/Recovery.cs` |
| callbacks | `Index/Interfaces/ISessionFunctions.cs` |
