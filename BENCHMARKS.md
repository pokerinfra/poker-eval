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
| Hold'em, one hand | 14.4 µs | 0.85 µs | **0.66 µs** | **22× faster** | **1.3× faster** |
| PLO, one hand | 778 µs | 72.6 µs | **4.27 µs** | **182× faster** | **17× faster** |
| PLO6, one hand | 1,958 µs | 174 µs | **9.27 µs** | **211× faster** | **19× faster** |
| Hold'em showdown, 6 players | 87.6 µs | 5.72 µs | **3.56 µs** | **25× faster** | **1.6× faster** |
| PLO showdown, 6 players | 4,708 µs | 426 µs | **24.4 µs** | **193× faster** | **17× faster** |

**Memory:** the pi-poker-eval WebAssembly file is about 130 KB. poker-evaluator loads a 130 MB lookup table.

**How the JavaScript calls stay cheap:** card strings are turned into card numbers in JavaScript and written straight into a fixed buffer inside WebAssembly memory; the result comes back the same way and is turned into an object in JavaScript. Each call passes only a few numbers across, with no allocation or copying in between. Descriptions are cached by hand strength, so each is fetched from WebAssembly only once.

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
