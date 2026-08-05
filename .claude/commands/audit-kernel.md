Perform a thorough Rust kernel-code audit on the specified files, or on all of `kernel/src/` if none are given.

Adapted from the `/audit-code` command in the maintainer's tron_grid project. The Vulkan-specific
section there becomes a bare-metal section here; the memory-safety section stays, because `unsafe`
Rust in a kernel reintroduces exactly the failure modes C++ never lost.

## What to Check

### `unsafe` discipline

- Any `unsafe` block outside a module CLAUDE.md designates for it — this is a hard failure, not a note
- `unsafe` blocks with no `// SAFETY:` comment, or with one that restates the code instead of naming the invariant
- `unsafe fn` with no `# Safety` section in its doc comment
- A `// SAFETY:` claim that the surrounding code does not actually guarantee — the most dangerous
  finding of all, because it looks reviewed
- `unsafe` blocks wider than they need to be: the block should contain the unsound operation, not the
  whole function around it

### Bare-metal correctness

- MMIO through anything other than a volatile read/write — the optimiser is entitled to elide or reorder plain accesses
- Missing compiler or memory barriers around MMIO and page-table writes
- TLB not invalidated after a page-table modification
- Cache maintenance missing where a device and the CPU share a buffer
- Assumptions about pointer provenance across an address-space switch
- Register reads that assume a reset value rather than programming it
- Alignment assumptions on `repr(C)` structures shared with hardware, or a missing `repr`

### Concurrency and interrupt safety

- Data reachable from both an interrupt handler and a thread without a lock or atomic
- A lock taken in an interrupt handler that is also taken with interrupts enabled — classic deadlock
- Spinlocks held across an operation that can block or yield
- Missing priority inheritance where the design requires it
- `Send`/`Sync` implemented by hand without an argument for why it holds

### Logic errors

- Off-by-one in loops, page counts and address-range arithmetic
- Integer overflow or underflow, especially in size and address computation — check for wrapping vs checked arithmetic
- Address arithmetic mixing physical and virtual addresses without a newtype to stop it
- Unreachable code or dead branches
- `as` casts that silently truncate an address or length

### Rust practice

- `unwrap()` or `expect()` on a path that can be reached at runtime — there is no unwinder underneath
- `panic!` where an error should be returned to a caller that can do something about it
- Indexing rather than `get()` where the index is not provably in range
- Missing `#[must_use]` on a function returning a status the caller must not drop
- Allocation on a path that must not allocate

### Architecture boundary

- Anything above `arch/mod.rs` that names a specific architecture
- `#[cfg(target_arch)]` used above the HAL boundary rather than inside it
- A HAL trait whose signature only makes sense for one of the two Tier-1 architectures

## Output Format

For each issue found, report:

1. **File and line number**
2. **Severity** (critical / warning / suggestion)
3. **Description** of the issue and the invariant it breaks
4. **Suggested fix**

Summarise with a count of issues by severity at the end. Report `unsafe`-outside-designated-modules
findings first regardless of their other severity, and list every new `unsafe` block you saw — that
list is required by CONSTITUTION.md §11.3 whether or not anything was wrong with it.
