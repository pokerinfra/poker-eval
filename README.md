# pi-poker-eval

Fast, exact poker hand evaluation for **Texas Hold'em** and **Omaha** (PLO, PLO5, PLO6), written in Rust, with a WebAssembly build for JavaScript and TypeScript.

Built and maintained by [PokerInfra](https://www.pokerinfra.com).

> **Status:** pre-release. Not yet published to crates.io or npm.

## Features

- **Hand ranking.** Any 1–7 card Hold'em hand, verified against every one of the 133,784,560 possible 7-card hands.
- **Omaha.** Best-hand selection using exactly two hole cards and three board cards, for 4, 5 and 6 hole cards.
- **Showdowns.** Every player's best hand and the winners, including split pots.
- **Readable results.** The best five cards and a description such as `Flush, A High`, `Two Pair, K's & 4's` or `Royal Flush`.
- **Equity.** How often each hand wins: exact over every run-out, or seeded Monte Carlo, against known hands or random opponents.
- **Small and dependency-free.** No lookup tables and no runtime dependencies. The WebAssembly build is about 130 KB.

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

On an Apple M1, in Rust:

| | Time per call |
|---|---|
| Evaluate a 7-card Hold'em hand | 144 ns |
| Evaluate a PLO hand (60 combinations) | 3.5 µs |
| Hold'em equity, 10,000 random run-outs against a random hand | 1.6 ms |

[BENCHMARKS.md](https://github.com/pokerinfra/poker-eval/blob/main/BENCHMARKS.md) has the full measurements, including comparisons with pokersolver and poker-evaluator from JavaScript.

## Building and testing

```sh
cargo test                          # unit, edge-case, equity and documentation tests
cargo test --release -- --ignored   # also checks all 133,784,560 seven-card hands (~5 s)
cargo run --release --example bench # speed measurements
```

The JavaScript build needs [wasm-pack](https://github.com/drager/wasm-pack):

```sh
./scripts/build-js.sh               # builds pkg/ and runs the JavaScript tests
```

## Licence

[MIT](https://github.com/pokerinfra/poker-eval/blob/main/LICENSE) © [PokerInfra](https://www.pokerinfra.com)
