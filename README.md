<p align="center">
  <a href="https://www.pokerinfra.com"><img src="https://www.pokerinfra.com/wp-content/uploads/2026/06/pokerinfra_logo.png" alt="PokerInfra" width="280" /></a>
</p>

# pi-poker-eval

Fast, exact poker hand evaluation for **Texas Hold'em** and **Omaha** (PLO, PLO5, PLO6), written in Rust, with a WebAssembly build for JavaScript and TypeScript.

Built and maintained by [PokerInfra](https://www.pokerinfra.com). Created by [Vikas Sood](https://github.com/vikassood11).

## Features

- **Hand ranking.** Any 1–7 card Hold'em hand, verified against every one of the 133,784,560 possible 7-card hands.
- **Omaha.** Best-hand selection using exactly two hole cards and three board cards, for 4, 5 and 6 hole cards.
- **Showdowns.** Every player's best hand and the winners, including split pots.
- **Readable results.** The best five cards and a description such as `Flush, A High`, `Two Pair, K's & 4's` or `Royal Flush`.
- **Equity.** How often each hand wins: exact over every run-out, or seeded Monte Carlo, against known hands or random opponents.
- **Small and dependency-free.** No lookup tables and no runtime dependencies. The WebAssembly build is about 130 KB.

## Installation

```sh
cargo add pi-poker-eval      # Rust
npm install pi-poker-eval    # JavaScript / TypeScript
```

## Rust

```rust
use pi_poker_eval::{Card, Equity, EquityCalculator, EquityMethod, EquityRequest, Evaluator, HandEvaluator, Variant};

let cards = |s: &str| Card::parse_list(s).unwrap();

// One hand: Omaha must use exactly two hole cards and three board cards.
let hand = Evaluator.evaluate(Variant::Omaha4, &cards("Tc 9c Qc Ac"), &cards("3c 5h 6c 9s 7c")).unwrap();
assert_eq!(hand.description(), "Flush, A High");
assert_eq!(hand.cards, cards("Ac Qc 7c 6c 3c"));

// A showdown: indices of the winners (more than one means a split pot).
let (aces, kings) = (cards("Ah Ad"), cards("Kh Kd"));
let result = Evaluator.showdown(Variant::Holdem, &cards("2c 7d 9s Jh Kc"), &[&aces, &kings]).unwrap();
assert_eq!(result.winners, [1]);

// Equity on the flop, over every turn and river.
let equity = Equity
    .equity(&EquityRequest {
        variant: Variant::Holdem,
        players: vec![cards("Ah As"), cards("Kd Kc")],
        board: cards("2c 7d 9s"),
        dead: vec![],
        method: EquityMethod::Exact,
    })
    .unwrap();
assert_eq!(equity.runouts, 990);
assert!(equity.players[0].equity > 0.9);
```

Hand strengths are comparable values: higher wins and equal is a tie. `EvaluatedHand` also gives the hand's `category` and `name()`.

Depend on the `HandEvaluator` and `EquityCalculator` traits rather than the concrete types, so implementations can be swapped or mocked in tests.

## JavaScript and TypeScript

The WebAssembly build has the same three calls, with full TypeScript types:

```ts
import { evaluate, showdown, equity } from "pi-poker-eval";

evaluate("PLO", ["Tc", "9c", "Qc", "Ac"], ["3c", "5h", "6c", "9s", "7c"]);
// { strength: …, category: "Flush", name: "Flush",
//   description: "Flush, A High", cards: ["Ac", "Qc", "7c", "6c", "3c"] }

showdown("NLH", ["2c", "7d", "9s", "Jh", "Kc"], [["Ah", "Ad"], ["Kh", "Kd"]]);
// { hands: [...], winners: [1] }

equity({
    variant: "NLH",
    players: [["Ah", "Kh"], []],            // [] is a random opponent
    board: [],
    method: { type: "monteCarlo", trials: 10000, seed: 1 },
});
// { players: [{ win, tie, equity }, …], runouts: 10000, exact: false }
```

Errors, such as an invalid card, a duplicate card or the wrong number of hole cards, are thrown as `Error` with a readable message.

## Cards and variants

| | Format |
|---|---|
| Card | A rank `2`–`9`, `T` or `10`, `J`, `Q`, `K`, `A`, then a suit `c`, `d`, `h`, `s`. Case-insensitive: `Ah`, `td`, `10D`. |
| Variant | `Holdem`, `Omaha4`, `Omaha5`, `Omaha6` in Rust; `"NLH"`, `"PLO"`, `"PLO5"`, `"PLO6"` in JavaScript. |
| Board | 0, 3, 4 or 5 cards. With a partial board, the best hand uses the cards available. |

## Performance

Measured on an Apple M1 with Rust 1.99.1 and Node 20.18.0, on hands pre-generated from a fixed seed. To reproduce: `cargo run --release --example bench` for Rust; for JavaScript, `./scripts/build-js.sh`, then `cd bench && npm install && node compare.cjs`.

### Rust

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

Ranking a 7-card hand on its own, with no result object, takes about 35 ns.

### JavaScript, compared with pokersolver and poker-evaluator

The same hands, called from Node. For Omaha, pokersolver ranks every 2+3 combination and picks the best with `Hand.winners`. poker-evaluator returns only a comparable number, with no description and no best five cards.

| Call | pokersolver | poker-evaluator | pi-poker-eval | vs pokersolver | vs poker-evaluator |
|---|---|---|---|---|---|
| Hold'em, one hand | 14.4 µs | 0.85 µs | **0.66 µs** | **22× faster** | **1.3× faster** |
| PLO, one hand | 778 µs | 72.6 µs | **4.27 µs** | **182× faster** | **17× faster** |
| PLO6, one hand | 1,958 µs | 174 µs | **9.27 µs** | **211× faster** | **19× faster** |
| Hold'em showdown, 6 players | 87.6 µs | 5.72 µs | **3.56 µs** | **25× faster** | **1.6× faster** |
| PLO showdown, 6 players | 4,708 µs | 426 µs | **24.4 µs** | **193× faster** | **17× faster** |

**Memory:** the WebAssembly file is about 130 KB. poker-evaluator loads a 130 MB lookup table.

### Equity

Monte Carlo runs 10,000 random run-outs.

| Request | Rust | From JavaScript |
|---|---|---|
| Hold'em, A♥K♥ vs 1 random hand, preflop | 1.58 ms | 2.92 ms |
| Hold'em, A♥K♥ vs 5 random hands, preflop | 4.68 ms | 7.24 ms |
| PLO, A♥A♦K♥Q♦ vs 1 random hand, preflop | 62.5 ms | 67.7 ms |
| Hold'em, A♥A♠ vs K♦K♣, exact (all 1,712,304 boards) | 143 ms | 216 ms |

For the first request from JavaScript, pokersolver takes 277 ms and poker-evaluator 8.2 ms.

**Correctness check:** exact A♥A♠ vs K♦K♣ gives equity 0.8125549, win 0.8106458 and tie 0.0038182. Enumerating all 1,712,304 boards independently with poker-evaluator gives the same figures to 7 decimal places.

## Building and testing

```sh
cargo test                          # unit, edge-case, equity and documentation tests
cargo test --release -- --ignored   # also checks all 133,784,560 seven-card hands (~5 s)
cargo run --release --example bench # speed measurements (see Performance)
```

The JavaScript build needs [wasm-pack](https://github.com/drager/wasm-pack):

```sh
./scripts/build-js.sh               # builds pkg/ and runs the JavaScript tests
```

## About PokerInfra

[PokerInfra](https://www.pokerinfra.com) builds poker software for operators: white-label platforms, enterprise source-code licensing and managed hosting. pi-poker-eval is the hand evaluator built for our poker engine, released as open source.

Questions or commercial enquiries: [www.pokerinfra.com](https://www.pokerinfra.com)

## Licence

[MIT](https://github.com/pokerinfra/poker-eval/blob/main/LICENSE) © [PokerInfra](https://www.pokerinfra.com)
