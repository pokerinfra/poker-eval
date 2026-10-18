# Benchmarks

Measured on an Apple M1 with Rust 1.99.1 and Node 20.18.0. Hands are pre-generated from a fixed seed.

- **Rust:** `cargo run --release --example bench`
- **JavaScript:** `./scripts/build-js.sh`, then `cd bench && npm install && node compare.cjs`

## Rust

Times include input validation, the duplicate-card check and building the best five cards.

| Call | Time per call | Calls per second |
|---|---|---|
| `evaluate`, Hold'em (7 cards) | 144 ns | 6.96 M |
| `evaluate`, PLO (60 combinations) | 3.46 µs | 289 k |
| `evaluate`, PLO5 (100 combinations) | 5.50 µs | 182 k |
| `evaluate`, PLO6 (150 combinations) | 7.81 µs | 128 k |
| `showdown`, Hold'em, 6 players | 1.28 µs | 781 k |
| `showdown`, PLO, 6 players | 19.8 µs | 50 k |
| `showdown`, PLO6, 6 players | 42.6 µs | 23 k |

Ranking a 7-card hand on its own, with no result object, takes about 35 ns. That is measured by the test over all 133,784,560 hands, which runs in 4.6 s including its own bookkeeping.

## From JavaScript, compared with pokersolver and poker-evaluator

The same hands, called from Node. For Omaha, pokersolver ranks every 2+3 combination and picks the best with `Hand.winners`. poker-evaluator returns only a comparable number, with no description and no best five cards.

| Call | pokersolver | poker-evaluator | pi-poker-eval | vs pokersolver | vs poker-evaluator |
|---|---|---|---|---|---|
| Hold'em, one hand | 14.2 µs | 0.86 µs | 4.14 µs | **3.4× faster** | 0.2× (slower) |
| PLO, one hand | 771 µs | 70.8 µs | 8.15 µs | **95× faster** | **8.7× faster** |
| PLO6, one hand | 1,949 µs | 173 µs | 13.7 µs | **142× faster** | **12.6× faster** |
| Hold'em showdown, 6 players | 85.4 µs | 5.62 µs | 19.4 µs | **4.4× faster** | 0.3× (slower) |
| PLO showdown, 6 players | 4,616 µs | 415 µs | 42.4 µs | **109× faster** | **9.8× faster** |

**Memory:** the pi-poker-eval WebAssembly file is about 130 KB. poker-evaluator loads a 130 MB lookup table.

**Why Hold'em is slower than poker-evaluator from JavaScript:** about 4 µs of each call goes on passing data between JavaScript and WebAssembly (card strings in, a result object with a description and five card strings out). The evaluation itself takes 144 ns. Making this crossing cheaper, without changing the API, is planned before the first release.

## Equity

Monte Carlo runs 10,000 random run-outs.

| Request | Rust | From JavaScript |
|---|---|---|
| Hold'em, A♥K♥ vs 1 random hand, preflop | 1.58 ms | 2.92 ms |
| Hold'em, A♥K♥ vs 5 random hands, preflop | 4.68 ms | 7.24 ms |
| PLO, A♥A♦K♥Q♦ vs 1 random hand, preflop | 62.5 ms | 67.7 ms |
| PLO6, one hand vs 5 random hands, preflop | 419 ms | — |
| Hold'em, A♥A♠ vs K♦K♣, exact (all 1,712,304 boards) | 143 ms | 216 ms |
| PLO flop, 2 known hands, exact (820 run-outs) | 2.89 ms | — |

The same Hold'em request (A♥K♥ vs 1 random hand, 10,000 trials) from JavaScript with other approaches:

| | Time | pi-poker-eval is |
|---|---|---|
| pokersolver | 277 ms | 95× faster |
| poker-evaluator | 8.2 ms | 2.8× faster |
| Hand-written loop over poker-evaluator's raw lookup table | 4.0 ms | 1.4× faster |
| **pi-poker-eval** | **2.9 ms** | — |

**Correctness check:** exact A♥A♠ vs K♦K♣ gives equity 0.8125549, win 0.8106458 and tie 0.0038182. Enumerating all 1,712,304 boards independently with poker-evaluator gives the same figures to 7 decimal places.

**Room to improve:** Omaha equity checks 60–150 combinations per player per run-out, so PLO6 against five opponents takes 419 ms for 10,000 trials.
