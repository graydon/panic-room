# Panic Room

This is a little experiment in non-unwind panic handling.

The idea is to bolt something like an arena on to sj/lj such that:

  - You can compile with panic=abort but..
  - Within a "room" it never actually gets to abort, it recovers
  - You can allocate, take, borrow and mut-borrow stuff in the room
  - Everything allocated gets destroyed on success _or_ panic-recover
  - If that stuff has destructors they'll all run too, like normal

It's a sort of miniature semi-transaction, similar to catch-unwind, but without
any actual unwinding, just sj/lj. A strange mutant relative of SEH.

Considerations:

  - I'm not actually sure there's a reason to connect memory allocation to
    control this way. Maybe the two pieces should remain separate. I think it
    is possibly convenient to entangle the arena stack, jmp_buf stack, panic
    handlers and destructors all together, but I might be imagining it.

Limitations:

  - Absent access to a single internal API, it can't reset the panic number, so
    it will forever think it's still panicking. This is just a number in the
    stdlib. It should be adjustable by people who want to take their life into
    their own hands. Please send me a patch if you know how to access it.

  - The "arena" isn't an arena at all, it's a vec-refcell-option-box-dyn-any
    thing. Just enough to sketch the right API. If you wanted it to perform like
    a real arena you'd need much more unsafe code I suspect. As it is I only use
    unsafe on sj/lj.

  - Everyone thinks sj/lj is fundamentally unsafe and horrible. Which, ok, sure,
    but unwinding is almost as bad _and_ it generates tons more code for landing
    pads and makes you have to wonder which code paths are going to unwind and
    run destructors on what half-constructed state. With this crate, dtors only
    run on happy paths in your code and on sad paths _on objects you allocated
    in the room_ and _at the end of the room's life_; the idea is to let you
    think less-hard about dtors from unwinds in the middle of your happy paths
    (and also not to codegen landing pads). Your code is therefore "a little
    more atomic", though of course it can still stop mid-execution due to a
    longjmp.

  - In exchange for not thinking about dtors running until the end, your objects
    will typically live longer (until the end!) unless you call Room::take on a
    handle and drop the result. Which you can do to force a dtor to run early.
