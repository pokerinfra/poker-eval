//! Speed measurements for the public API, on pre-generated random hands.
//! Run: cargo run --release --example bench

use std::hint::black_box;
use std::time::Instant;

use pi_poker_eval::{
    Card, Equity, EquityCalculator, EquityMethod, EquityRequest, Evaluator, HandEvaluator, Rank,
    Suit, Variant,
};

struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) % n as u64) as usize
    }

    fn deal(&mut self, n: usize) -> Vec<Card> {
        let mut deck: Vec<Card> = Rank::ALL
            .iter()
            .flat_map(|&r| Suit::ALL.map(|s| Card::new(r, s)))
            .collect();
        for i in 0..n {
            let j = i + self.below(52 - i);
            deck.swap(i, j);
        }
        deck.truncate(n);
        deck
    }
}

/// Runs `f` over every deal `rounds` times; prints nanoseconds per call.
fn measure(label: &str, deals: &[Vec<Card>], rounds: usize, f: impl Fn(&[Card])) {
    for d in deals.iter().take(1000) {
        f(d); // warm-up
    }
    let start = Instant::now();
    for _ in 0..rounds {
        for d in deals {
            f(black_box(d));
        }
    }
    let ns = start.elapsed().as_nanos() as f64 / (rounds * deals.len()) as f64;
    println!("{label:<52} {ns:>10.1} ns/call {:>14.0} calls/s", 1e9 / ns);
}

fn main() {
    let mut rng = Lcg(42);
    let n = 10_000;

    println!(
        "pi-poker-eval {} — Rust, release build\n",
        env!("CARGO_PKG_VERSION")
    );

    for (variant, label) in [
        (
            Variant::Holdem,
            "evaluate: Hold'em, 2 hole + 5 board (7 cards)",
        ),
        (
            Variant::Omaha4,
            "evaluate: PLO, 4 hole + 5 board (60 combos)",
        ),
        (
            Variant::Omaha5,
            "evaluate: PLO5, 5 hole + 5 board (100 combos)",
        ),
        (
            Variant::Omaha6,
            "evaluate: PLO6, 6 hole + 5 board (150 combos)",
        ),
    ] {
        let h = variant.hole_cards();
        let deals: Vec<Vec<Card>> = (0..n).map(|_| rng.deal(h + 5)).collect();
        let rounds = if variant == Variant::Holdem { 200 } else { 10 };
        measure(label, &deals, rounds, |d| {
            black_box(Evaluator.evaluate(variant, &d[..h], &d[h..]).unwrap());
        });
    }

    println!();
    for (variant, players, label) in [
        (Variant::Holdem, 6, "showdown: Hold'em, 6 players"),
        (Variant::Omaha4, 6, "showdown: PLO, 6 players"),
        (Variant::Omaha6, 6, "showdown: PLO6, 6 players"),
    ] {
        let h = variant.hole_cards();
        let deals: Vec<Vec<Card>> = (0..n / 10).map(|_| rng.deal(h * players + 5)).collect();
        measure(label, &deals, 20, |d| {
            let holes: Vec<&[Card]> = (0..players).map(|p| &d[p * h..(p + 1) * h]).collect();
            black_box(
                Evaluator
                    .showdown(variant, &d[h * players..], &holes)
                    .unwrap(),
            );
        });
    }

    println!();
    let time = |label: &str, req: EquityRequest| {
        let start = Instant::now();
        let r = Equity.equity(&req).unwrap();
        let ms = start.elapsed().as_secs_f64() * 1e3;
        let eq: Vec<String> = r
            .players
            .iter()
            .map(|p| format!("{:.2}%", p.equity * 100.0))
            .collect();
        println!(
            "{label:<52} {ms:>9.2} ms   runouts {:>9}   equity {}",
            r.runouts,
            eq.join(" / ")
        );
    };
    let req = |variant, players: &[&str], board: &str, method| EquityRequest {
        variant,
        players: players
            .iter()
            .map(|p| Card::parse_list(p).unwrap())
            .collect(),
        board: Card::parse_list(board).unwrap(),
        dead: vec![],
        method,
    };
    let mc = |trials| EquityMethod::MonteCarlo { trials, seed: 1 };
    time(
        "equity exact: AhAs vs KdKc preflop (all boards)",
        req(
            Variant::Holdem,
            &["Ah As", "Kd Kc"],
            "",
            EquityMethod::Exact,
        ),
    );
    time(
        "equity MC 10k: AhKh vs 1 random, preflop",
        req(Variant::Holdem, &["Ah Kh", ""], "", mc(10_000)),
    );
    time(
        "equity MC 10k: AhKh vs 5 random, preflop",
        req(
            Variant::Holdem,
            &["Ah Kh", "", "", "", "", ""],
            "",
            mc(10_000),
        ),
    );
    time(
        "equity MC 10k: PLO AhAd Kh Qd vs 1 random",
        req(Variant::Omaha4, &["Ah Ad Kh Qd", ""], "", mc(10_000)),
    );
    time(
        "equity MC 10k: PLO6 hero vs 5 random",
        req(
            Variant::Omaha6,
            &["Ah Ad Kh Qd Jc Tc", "", "", "", "", ""],
            "",
            mc(10_000),
        ),
    );
    time(
        "equity exact: PLO flop, 2 known hands",
        req(
            Variant::Omaha4,
            &["Ah Ad Kh Qd", "9c 8c 7d 6d"],
            "Ts 5c 2h",
            EquityMethod::Exact,
        ),
    );
}
