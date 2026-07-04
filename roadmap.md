Phase 1: The Multi-Pass Compiler (AST -> VIR -> MIR)

This is the immediate next frontier. It replaces the single-pass AST compiler with a professional, multi-tier pipeline.

    VIR (Vypr Intermediate Representation): Create the desugaring layer. Convert syntactic sugar (like for loops, if/elif chains, and list comprehensions) into raw while loops and base expressions. Implement compile-time name resolution to map string variable names to numerical stack IDs.

    MIR (Mid-Level IR): Lower the VIR into Three-Address Code (TAC). Flatten expressions into temporary variables (t0, t1) and model control flow explicitly using Basic Blocks and Terminators (Jumps).

    The Optimizer: Write the Rust analysis passes over the MIR graph to implement Constant Folding, Dead Code Elimination, and Copy Propagation.

    The Bytecode Emitter: Gut the complexity out of compiler.rs. Rewrite it to blindly and rapidly translate the optimized MIR blocks into the final hex OpCodes.

Phase 2: Engine Speed & Architecture

With memory management locked in via Rc, these are the purely architectural upgrades to maximize bytecode execution speed.

    String Interning: Scan all source strings at compile time, assign them unique usize IDs, and swap the VM's property/module HashMaps to use integer keys instead of strings.

    Value Representation (NaN Boxing): Drop the 24-byte Value enum. Encode integers, booleans, and 48-bit Rc heap pointers into the quiet space of an 8-byte f64 float to drastically reduce stack size and eliminate CPU cache misses.

Phase 3: The Pythonic Features

Once the VIR/MIR pipeline is humming, adding these complex features becomes an exercise in simple AST desugaring rather than hacking the bytecode loop.

    Keyword Arguments (kwargs): Support named and out-of-order parameters in function calls.

    Closures and Upvalues: Add lambda or nested function syntax, using "Open" and "Closed" upvalues so closures can capture and mutate parent stack slots.

    Classes and Objects: Introduce class syntax, Value::Class (the blueprint), Value::Instance (the Rc-wrapped property table), and ExprKind::Set for property assignment.
