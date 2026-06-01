# Examples

Provable programs that ship with Maat. Every file in this directory proves and verifies end-to-end under `development_options`.

Each program defines a top-level `fn main(<params>) -> <ret>`. Parameters marked `pub` bind to a boundary-constrained public-memory cell (the verifier learns and pins the value); bare parameters bind to a prover-supplied witness cell with no public commitment (the verifier learns nothing about it). Omitted inputs default to zero.

## Prerequisites

Install the Maat CLI per the [root README](../README.md#installation), or substitute `cargo run --release --` for `maat` in every command below when working from a source build.

## How to run an example

The lifecycle for any example is the same three commands:

```bash
maat run examples/<name>.maat               # execute concretely (no proof)
maat prove examples/<name>.maat             # generate a STARK proof
maat verify examples/<name>.proof.bin       # check the proof
```

`maat run` does not bind public/private inputs; every parameter is treated as zero. To supply inputs, use `maat prove` with `--input` (public, comma-separated; binds the `pub` parameters in declaration order) and `--private-input` (private; binds the bare parameters in declaration order). JSON files work too via `--inputs-file` / `--private-inputs-file`. The proof output path defaults to the source path with its extension swapped to `.proof.bin` (so `examples/<name>.maat` proves to `examples/<name>.proof.bin`); override with `-o`.

```bash
maat prove examples/<name>.maat \
    --input 1,2,3 \
    --private-input 4,5
maat verify examples/<name>.proof.bin
```

`-p` / `--production` switches from `development_options` (~12 bits security) to `production_options` (~97 bits security); proof size and prover time grow accordingly. `-t <trace.csv>` additionally dumps the execution trace for inspection.

---

## Fibonacci ports

These five no-hash ports mirror Winterfell's Fibonacci example family. The first three are additive (`(b, a + b)`), the last two are multiplicative (`(b, a * b)`). All five take no inputs---the sequence length and seed are baked in---so the proved output is fully determined by the source.

### `fib.maat` --- 64-term additive Fibonacci over `Felt`

```rust
fn main() -> Felt
```

64-term additive Fibonacci over the Goldilocks field. Field wraparound keeps the sequence well-defined past machine-integer overflow.

```bash
maat run examples/fib.maat                # output: 10610209857723
maat prove examples/fib.maat
maat verify examples/fib.proof.bin
```

### `fib8.maat` --- 128-term additive Fibonacci over `Felt`

```rust
fn main() -> Felt
```

Same additive recurrence as `fib`, run for 128 steps. This is the analogue of Winterfell's `fib8`, which packs eight terms per trace row to keep longer sequences cheap.

```bash
maat run examples/fib8.maat               # output: 18213276994518315295
maat prove examples/fib8.maat
maat verify examples/fib8.proof.bin
```

### `fib_small.maat` --- 24-term additive Fibonacci over `i64`

```rust
fn main() -> i64
```

The same additive recurrence in the native `i64` integer type rather than the Goldilocks field, exercising the range-checked integer-arithmetic path instead of the field path. Length stays within `i64` so no value overflows.

```bash
maat run examples/fib_small.maat          # output: 46368
maat prove examples/fib_small.maat
maat verify examples/fib_small.proof.bin
```

### `mulfib.maat` --- 32-term multiplicative Fibonacci over `Felt`

```rust
fn main() -> Felt
```

Multiplicative Fibonacci `(b, a * b)` over the Goldilocks field, seeds `(1, 2)`, run for 32 steps. Products grow far faster than sums, so the sequence is only meaningful in a field with modular wraparound.

```bash
maat run examples/mulfib.maat             # output: 137438953440
maat prove examples/mulfib.maat
maat verify examples/mulfib.proof.bin
```

### `mulfib8.maat` --- 64-term multiplicative Fibonacci over `Felt`

```rust
fn main() -> Felt
```

Same multiplicative recurrence as `mulfib`, run for 64 steps. This is the analogue of Winterfell's `mulfib8`.

```bash
maat run examples/mulfib8.maat            # output: 18446744069280366593
maat prove examples/mulfib8.maat
maat verify examples/mulfib8.proof.bin
```

## Verifiable delay

### `vdf.maat` --- 256-iteration VDF from a public seed

```rust
fn main(seed: pub Felt) -> Felt
```

Iterates the VDF permutation `x <- x*x*x + 42` for 256 sequential rounds from a caller-supplied public `seed`. Each step depends on the previous one, so the work cannot be parallelised away---the defining property of a verifiable delay function. The `pub` seed binds to a boundary-constrained public-memory cell, so the proof attests `output = vdf(seed, 256)` for the exact seed the verifier sees.

```bash
maat run examples/vdf.maat                                 # output: 16460772602756348587 (seed = 0)
maat prove examples/vdf.maat                               # output: 16460772602756348587 (seed = 0)
maat prove examples/vdf.maat --input 3                     # output: 11509554200763765976 (seed = 3)
maat verify examples/vdf.proof.bin
```

## Hash workloads

These four ports cover the hash-consuming half of the Winterfell example suite, each driven by the inline Rescue-Prime AIR (parameters `rp64_256`: Goldilocks, state width 12, capacity 4, rate 8, digest size 4, 7 rounds). Every Rescue dispatch materialises one period-8 block of round witness rows in the trace, plus `N + 4` synthetic memory writes that bind the input cells and digest cells through the unified memory permutation.

### `rescue.maat` --- 32-round Rescue hash chain from a public seed

```rust
fn main(seed: pub Felt) -> Felt
```

Iterates `state <- rescue_4(state)` for 32 rounds starting from the rate block `[seed, 0, 0, 0]`, returning the first cell of the final state. The `pub` seed binds to a boundary-constrained public-memory cell, mirroring Winterfell's `rescue` example.

```bash
maat run examples/rescue.maat                              # output: 11725969031558041514 (seed = 0)
maat prove examples/rescue.maat                            # output: 11725969031558041514 (seed = 0)
maat prove examples/rescue.maat --input 3                  # output: 10180035301926974958 (seed = 3)
maat verify examples/rescue.proof.bin
```

### `rescue_raps.maat` --- twin Rescue chains absorbed through a final wide-block hash

```rust
fn main(seed_a: pub Felt, seed_b: pub Felt) -> Felt
```

Runs two independent 16-round Rescue chains from a pair of public seeds, then absorbs the two endpoints (`[end_a | end_b]` as one 8-felt rate block) through one final `rescue_8`, returning the first cell of the aggregate. The single-trace analogue of Winterfell's `rescue_raps` "multi-permutation" example.

```bash
maat run examples/rescue_raps.maat                         # output: 10391412091215224520 (seeds = 0, 0)
maat prove examples/rescue_raps.maat                       # output: 10391412091215224520
maat prove examples/rescue_raps.maat --input 1,2           # output: 14882797101753228084 (seeds = 1, 2)
maat verify examples/rescue_raps.proof.bin
```

### `lamport.maat` --- 4-secret Lamport-style commitment under one public challenge

```rust
fn main(sig0: Felt, sig1: Felt, sig2: Felt, sig3: Felt, challenge: pub Felt) -> Felt
```

Hashes four private signature halves (`sig0..sig3`) under one public `challenge` via `rescue_2`, concatenates the four 2-felt digests into an 8-felt rate block, and returns the first cell of the final `rescue_8` aggregate---the Rescue-driven verification recurrence at the heart of Winterfell's `lamport` example, simplified from one dispatch per signature bit (256) to four (one per private secret). A verifier holding the matching public-key aggregate accepts the proof iff the prover knew a signature consistent with that aggregate under the challenge.

```bash
maat run examples/lamport.maat                                            # output: 13179955082315886473 (all zero)
maat prove examples/lamport.maat                                          # output: 13179955082315886473
maat prove examples/lamport.maat --input 7 --private-input 1,2,3,4        # output: 14907819753415673850
maat verify examples/lamport.proof.bin
```

### `merkle.maat` --- depth-4 Rescue-compressed authentication path

```rust
fn main(leaf_word: Felt, sib0_word: Felt, sib1_word: Felt, sib2_word: Felt, sib3_word: Felt, bit0: pub Felt, bit1: pub Felt, bit2: pub Felt, bit3: pub Felt) -> Felt
```

Walks a depth-four Merkle authentication path from the private `leaf_word` against four private sibling words `sib_i_word` under four public path selectors `bit_i`. The parent of each level uses a branch-free `is_right * sibling + (1 - is_right) * node` mix before feeding the resulting 8-felt block through `rescue_8`. The proven output is the first cell of the computed root; a verifier that knows the published root accepts the proof iff the prover knew a leaf and four siblings that compress to that root under the public bit sequence.

```bash
maat run examples/merkle.maat                                                          # output: 15977934316784223861 (all zero)
maat prove examples/merkle.maat                                                        # output: 15977934316784223861
maat prove examples/merkle.maat --input 0,1,0,1 --private-input 42,1,2,3,4             # output: 1078833057380401044
maat verify examples/merkle.proof.bin
```
