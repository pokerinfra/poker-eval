//! Equity tests through the public API.
//!
//! Exact results are checked against an independent count: looping over every
//! remaining card (or opponent hand) and calling `showdown` directly.

use pi_poker_eval::{
    Card, Equity, EquityCalculator, EquityMethod, EquityRequest, EquityResult, EvalError,
    Evaluator, HandEvaluator, Rank, Suit, Variant,
};

fn cards(text: &str) -> Vec<Card> {
    Card::parse_list(text).unwrap()
}

fn request(variant: Variant, players: &[&str], board: &str, method: EquityMethod) -> EquityRequest {
    EquityRequest {
        variant,
        players: players.iter().map(|p| cards(p)).collect(),
        board: cards(board),
        dead: vec![],
        method,
    }
}

fn equity(variant: Variant, players: &[&str], board: &str, method: EquityMethod) -> EquityResult {
    Equity
        .equity(&request(variant, players, board, method))
        .unwrap()
}

fn deck_without(used: &[Card]) -> Vec<Card> {
    Rank::ALL
        .iter()
        .flat_map(|&r| Suit::ALL.map(|s| Card::new(r, s)))
        .filter(|c| !used.contains(c))
        .collect()
}

/// Independent reference: (win, tie, equity) per player, from `showdown` over every river card.
fn by_hand_on_turn(variant: Variant, players: &[&str], turn_board: &str) -> Vec<(f64, f64, f64)> {
    let holes: Vec<Vec<Card>> = players.iter().map(|p| cards(p)).collect();
    let board = cards(turn_board);
    let used: Vec<Card> = holes.iter().flatten().chain(&board).copied().collect();
    let rivers = deck_without(&used);
    let mut totals = vec![(0.0, 0.0, 0.0); holes.len()];
    for river in &rivers {
        let mut full = board.clone();
        full.push(*river);
        let refs: Vec<&[Card]> = holes.iter().map(Vec::as_slice).collect();
        let winners = Evaluator.showdown(variant, &full, &refs).unwrap().winners;
        for &w in &winners {
            if winners.len() == 1 {
                totals[w].0 += 1.0;
            } else {
                totals[w].1 += 1.0;
            }
            totals[w].2 += 1.0 / winners.len() as f64;
        }
    }
    let n = rivers.len() as f64;
    totals
        .into_iter()
        .map(|(w, t, e)| (w / n, t / n, e / n))
        .collect()
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: {actual} vs {expected} (±{tolerance})"
    );
}

fn assert_sums_to_one(result: &EquityResult) {
    let total: f64 = result.players.iter().map(|p| p.equity).sum();
    assert_close(total, 1.0, 1e-9, "equities sum to 1");
}

// ---------------------------------------------------------------------------
// Exact
// ---------------------------------------------------------------------------

#[test]
fn river_is_just_the_showdown() {
    let r = equity(
        Variant::Holdem,
        &["Ah Ad", "Kh Kd"],
        "2c 7d 9s Jh Kc",
        EquityMethod::Exact,
    );
    assert_eq!(r.runouts, 1);
    assert!(r.exact);
    assert_eq!((r.players[0].win, r.players[1].win), (0.0, 1.0));
}

#[test]
fn exact_on_the_turn_matches_an_independent_count() {
    for (variant, players, board) in [
        (Variant::Holdem, &["Ah Kh", "Qc Qd"][..], "2h 7h 9c Js"),
        (
            Variant::Holdem,
            &["Ah Kd", "As Kc", "7c 8c"][..],
            "9c Tc 2d 3s",
        ),
        (
            Variant::Omaha4,
            &["Ah Kh Qc Js", "9c 9d 8s 7s"][..],
            "2h 7h 9h Tc",
        ),
        (
            Variant::Omaha6,
            &["Ah Kh Qc Js Tc 2d", "9c 9d 8s 7s 6d 5d"][..],
            "2h 7h 9h Kc",
        ),
    ] {
        let r = equity(variant, players, board, EquityMethod::Exact);
        assert_eq!(
            r.runouts,
            52 - (players.len() * variant.hole_cards() + 4) as u64
        );
        let reference = by_hand_on_turn(variant, players, board);
        for (i, p) in r.players.iter().enumerate() {
            assert_close(p.win, reference[i].0, 1e-12, "win");
            assert_close(p.tie, reference[i].1, 1e-12, "tie");
            assert_close(p.equity, reference[i].2, 1e-12, "equity");
        }
        assert_sums_to_one(&r);
    }
}

#[test]
fn exact_counts_every_runout() {
    // Heads-up preflop: C(48, 5) boards.
    let r = equity(
        Variant::Holdem,
        &["Ah As", "Kd Kc"],
        "",
        EquityMethod::Exact,
    );
    assert_eq!(r.runouts, 1_712_304);
    // Aces win about 82% against kings.
    assert!(
        r.players[0].equity > 0.80 && r.players[0].equity < 0.83,
        "{:?}",
        r.players[0]
    );
    assert_sums_to_one(&r);
    // Flop: C(45, 2) turn-river pairs.
    assert_eq!(
        equity(
            Variant::Holdem,
            &["Ah As", "Kd Kc"],
            "2c 7d 9s",
            EquityMethod::Exact
        )
        .runouts,
        990
    );
}

#[test]
fn identical_hands_split() {
    // Same ranks, different suits: mostly ties, and exactly equal equity.
    let r = equity(
        Variant::Holdem,
        &["Ac Kd", "Ad Kc"],
        "2h 7h 9s",
        EquityMethod::Exact,
    );
    assert_close(r.players[0].equity, 0.5, 1e-12, "symmetric equity");
    assert!(r.players[0].tie > 0.9);
}

#[test]
fn unknown_opponent_exact_on_the_river() {
    // Hero vs one random hand on the river: every one of C(45, 2) = 990 opponent hands.
    let r = equity(
        Variant::Holdem,
        &["Ah Kh", ""],
        "Qh Jh 2c 3d 4s",
        EquityMethod::Exact,
    );
    assert_eq!(r.runouts, 990);
    // Independent count: showdown against each possible opponent hand.
    let used = cards("Ah Kh Qh Jh 2c 3d 4s");
    let deck = deck_without(&used);
    let (mut win, mut tie) = (0, 0);
    for i in 0..deck.len() {
        for j in i + 1..deck.len() {
            let opponent = [deck[i], deck[j]];
            let hero = cards("Ah Kh");
            let w = Evaluator
                .showdown(
                    Variant::Holdem,
                    &cards("Qh Jh 2c 3d 4s"),
                    &[&hero, &opponent],
                )
                .unwrap()
                .winners;
            match w.as_slice() {
                [0] => win += 1,
                [0, 1] => tie += 1,
                _ => {}
            }
        }
    }
    assert_close(r.players[0].win, win as f64 / 990.0, 1e-12, "win vs random");
    assert_close(r.players[0].tie, tie as f64 / 990.0, 1e-12, "tie vs random");
}

#[test]
fn dead_cards_are_removed_from_the_deck() {
    let mut req = request(
        Variant::Holdem,
        &["Ah Kh", "Qc Qd"],
        "2h 7h 9c Js",
        EquityMethod::Exact,
    );
    let live = Equity.equity(&req).unwrap();
    req.dead = cards("3h 4h 5h");
    let with_dead = Equity.equity(&req).unwrap();
    assert_eq!(with_dead.runouts, live.runouts - 3);
    // Three of hero's flush outs are gone.
    assert!(with_dead.players[0].equity < live.players[0].equity);
}

// ---------------------------------------------------------------------------
// Monte Carlo and Auto
// ---------------------------------------------------------------------------

#[test]
fn monte_carlo_converges_to_exact() {
    let players = ["Ah Kh", "Qc Qd", "7s 8s"];
    let exact = equity(Variant::Holdem, &players, "2h 7h 9c", EquityMethod::Exact);
    let mc = equity(
        Variant::Holdem,
        &players,
        "2h 7h 9c",
        EquityMethod::MonteCarlo {
            trials: 200_000,
            seed: 1,
        },
    );
    assert!(!mc.exact);
    assert_eq!(mc.runouts, 200_000);
    for i in 0..3 {
        assert_close(
            mc.players[i].equity,
            exact.players[i].equity,
            0.005,
            "MC vs exact",
        );
    }
    assert_sums_to_one(&mc);
}

#[test]
fn monte_carlo_is_repeatable_from_its_seed() {
    let run = |seed| {
        equity(
            Variant::Omaha4,
            &["Ah Kh Qc Js", ""],
            "",
            EquityMethod::MonteCarlo {
                trials: 5_000,
                seed,
            },
        )
    };
    assert_eq!(run(7), run(7));
    assert_ne!(run(7), run(8));
}

#[test]
fn monte_carlo_against_random_opponents() {
    // Aces against five random hands win roughly half the time.
    let r = equity(
        Variant::Holdem,
        &["Ah As", "", "", "", "", ""],
        "",
        EquityMethod::MonteCarlo {
            trials: 100_000,
            seed: 3,
        },
    );
    assert!(
        r.players[0].equity > 0.45 && r.players[0].equity < 0.55,
        "{:?}",
        r.players[0]
    );
    // The five random hands are interchangeable: similar equity each.
    for p in &r.players[1..] {
        assert_close(
            p.equity,
            r.players[1].equity,
            0.01,
            "random opponents alike",
        );
    }
    assert_sums_to_one(&r);
}

#[test]
fn auto_picks_exact_only_when_affordable() {
    let auto = EquityMethod::Auto {
        max_exact: 10_000,
        trials: 20_000,
        seed: 1,
    };
    let flop = equity(Variant::Holdem, &["Ah As", "Kd Kc"], "2c 7d 9s", auto);
    assert!(flop.exact);
    assert_eq!(flop.runouts, 990);
    let preflop = equity(Variant::Holdem, &["Ah As", "Kd Kc"], "", auto);
    assert!(!preflop.exact);
    assert_eq!(preflop.runouts, 20_000);
}

#[test]
fn runout_count_without_running() {
    let req = request(Variant::Holdem, &["Ah As", ""], "", EquityMethod::Exact);
    // C(50, 2) opponent hands × C(48, 5) boards.
    assert_eq!(Equity.runout_count(&req).unwrap(), 1_225 * 1_712_304);
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

#[test]
fn equity_validation() {
    let bad = |players: &[&str], board: &str, method| {
        Equity.equity(&request(Variant::Holdem, players, board, method))
    };
    let exact = EquityMethod::Exact;
    assert_eq!(bad(&[], "", exact), Err(EvalError::NoPlayers));
    assert_eq!(
        bad(&["Ah"], "", exact),
        Err(EvalError::WrongHoleCardCount {
            variant: Variant::Holdem,
            expected: 2,
            got: 1
        })
    );
    assert_eq!(
        bad(&["Ah Kh"], "2c 3c", exact),
        Err(EvalError::InvalidBoardSize(2))
    );
    assert_eq!(
        bad(&["Ah Kh", "Ah Qd"], "", exact),
        Err(EvalError::DuplicateCard("Ah".parse().unwrap()))
    );
    assert_eq!(
        bad(
            &["Ah Kh"],
            "",
            EquityMethod::MonteCarlo { trials: 0, seed: 1 }
        ),
        Err(EvalError::InvalidTrials)
    );
    // 24 random Hold'em hands + 5 board cards = 53 cards: more than the deck.
    let crowd = vec![""; 24];
    assert_eq!(
        bad(&crowd, "", EquityMethod::MonteCarlo { trials: 1, seed: 1 }),
        Err(EvalError::NotEnoughCards {
            needed: 53,
            available: 52
        })
    );
}
