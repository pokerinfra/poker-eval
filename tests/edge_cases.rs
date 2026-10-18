//! Tricky hands, checked through the public API only.
//!
//! Two kinds of test:
//!  * named cases — hands players and dealers argue about;
//!  * properties over many seeded random deals — rules that must hold for every hand.

use pi_poker_eval::{Card, Category, EvaluatedHand, Evaluator, HandEvaluator, Rank, Suit, Variant};

fn cards(text: &str) -> Vec<Card> {
    Card::parse_list(text).unwrap()
}

fn eval(variant: Variant, hole: &str, board: &str) -> EvaluatedHand {
    Evaluator
        .evaluate(variant, &cards(hole), &cards(board))
        .unwrap()
}

fn winners(variant: Variant, board: &str, holes: &[&str]) -> Vec<usize> {
    let holes: Vec<Vec<Card>> = holes.iter().map(|h| cards(h)).collect();
    let refs: Vec<&[Card]> = holes.iter().map(Vec::as_slice).collect();
    Evaluator
        .showdown(variant, &cards(board), &refs)
        .unwrap()
        .winners
}

// ---------------------------------------------------------------------------
// Named cases
// ---------------------------------------------------------------------------

#[test]
fn counterfeited_pocket_pair_plays_nothing() {
    // Board two pair 9s & 7s with an ace: pocket deuces add nothing, so it's a split with K-3.
    assert_eq!(
        winners(Variant::Holdem, "7c 7d 9s 9h Ac", &["2h 2d", "Kh 3d"]),
        [0, 1]
    );
    assert_eq!(
        eval(Variant::Holdem, "2h 2d", "7c 7d 9s 9h Ac").description(),
        "Two Pair, 9's & 7's"
    );
}

#[test]
fn counterfeited_two_pair_loses_to_a_better_kicker() {
    // Flopped 6-5 two pair, then the board pairs kings: K-K-6-6 with a 9 kicker.
    // A-9 makes K-K-9-9 and wins.
    assert_eq!(
        winners(Variant::Holdem, "6c 5d Kh Kd 9s", &["6h 5h", "As 9d"]),
        [1]
    );
}

#[test]
fn board_plays_for_everyone() {
    assert_eq!(
        winners(Variant::Holdem, "Ah Kh Qh Jh Th", &["2c 3d", "Ac Kc"]),
        [0, 1]
    );
    // The best five uses no hole card at all.
    let e = eval(Variant::Holdem, "2c 3d", "Ah Kh Qh Jh Th");
    assert!(!e.cards.contains(&"2c".parse().unwrap()) && !e.cards.contains(&"3d".parse().unwrap()));
    // Board quads: the higher kicker wins; kickers below the board's fifth card split.
    assert_eq!(
        winners(Variant::Holdem, "9c 9d 9h 9s 2c", &["Ah 3d", "Kh 4d"]),
        [0]
    );
    assert_eq!(
        winners(Variant::Holdem, "9c 9d 9h 9s Kc", &["Qh 3d", "Jh 4d"]),
        [0, 1]
    );
}

#[test]
fn suits_never_decide() {
    assert_eq!(
        winners(Variant::Holdem, "2c 7d 9s Jh Kc", &["Ah Qd", "As Qc"]),
        [0, 1]
    );
    assert_eq!(
        winners(Variant::Holdem, "Tc Jd Qs 2h 3c", &["Kh 9d", "Ks 9c"]),
        [0, 1]
    );
}

#[test]
fn straight_edges() {
    // The wheel loses to a 6-high straight.
    assert_eq!(
        winners(Variant::Holdem, "2c 3d 4s 5h Kc", &["Ah Qd", "6h Qc"]),
        [1]
    );
    // A steel wheel (5-high straight flush) beats four aces.
    assert_eq!(
        winners(Variant::Holdem, "2h 3h 4h Ac Ad", &["Ah 5h", "As 7d"]),
        [0]
    );
    // Broadway beats a king-high straight.
    assert_eq!(
        winners(Variant::Holdem, "Tc Jd Qs 2h 3c", &["Ah Kd", "Kh 9c"]),
        [0]
    );
    // Q-K-A-2-3 does not wrap around.
    assert_eq!(
        eval(Variant::Holdem, "Qh Kd", "Ac 2s 3h 8d 9c").category,
        Category::HighCard
    );
}

#[test]
fn full_houses_compare_trips_first() {
    // Aces full of deuces beats kings full of deuces.
    assert_eq!(
        winners(Variant::Holdem, "Ac Kd 2h 2s 7c", &["Ah Ad", "Kh Ks"]),
        [0]
    );
    // Same trips on board: the pair decides (A-A-A-K-K over A-A-A-Q-Q).
    assert_eq!(
        winners(Variant::Holdem, "Ac Ad Ah 7c 2d", &["Kh Kd", "Qh Qd"]),
        [0]
    );
    // A full house loses to four of a kind made from the same paired board.
    assert_eq!(
        winners(Variant::Holdem, "Ac Ah Kd Ks 2c", &["Ad 2d", "Kh Kc"]),
        [1]
    );
    assert_eq!(
        eval(Variant::Holdem, "Ad 2d", "Ac Ah Kd Ks 2c").description(),
        "Full House, A's over K's"
    );
    assert_eq!(
        eval(Variant::Holdem, "Kh Kc", "Ac Ah Kd Ks 2c").description(),
        "Four of a Kind, K's"
    );
}

#[test]
fn omaha_tricky_hands() {
    // Pocket aces with a third ace on board plus a paired board: aces full.
    assert_eq!(
        eval(Variant::Omaha4, "Ah Ad 7c 2s", "Ac Kd Ks 9h 3c").description(),
        "Full House, A's over K's"
    );
    // A-2 in hand makes the wheel with 3-4-5 on board.
    assert_eq!(
        eval(Variant::Omaha4, "Ah 2d Kc Ks", "3c 4d 5s 9h Jc").description(),
        "Straight, 5 High"
    );
    // Four of a suit in hand is still only two of them: needs three on board.
    assert_ne!(
        eval(Variant::Omaha4, "Ah Kh Qh Jh", "2h 7c 9d 3s 4c").category,
        Category::Flush
    );
    // Set over set: the higher set wins even though both used two hole cards.
    assert_eq!(
        winners(
            Variant::Omaha4,
            "Kc 7d 2s 9h 3c",
            &["Kh Kd 4c 5c", "7h 7c Ah Ad"]
        ),
        [0]
    );
    // PLO6: six hole cards, best two still chosen.
    assert_eq!(
        eval(Variant::Omaha6, "Ah Kh 2c 3d 7s 8s", "Qh Jh Th 4c 5d").description(),
        "Royal Flush"
    );
}

#[test]
fn single_player_showdown() {
    assert_eq!(winners(Variant::Holdem, "2c 7d 9s Jh Kc", &["Ah Ad"]), [0]);
}

// ---------------------------------------------------------------------------
// Properties over seeded random deals
// ---------------------------------------------------------------------------

/// Deterministic generator so any failure can be replayed from the seed.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) % n as u64) as usize
    }

    /// `n` distinct random cards.
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

fn subsets(items: &[Card], k: usize) -> Vec<Vec<Card>> {
    if k == 0 {
        return vec![vec![]];
    }
    (0..items.len())
        .flat_map(|i| {
            subsets(&items[i + 1..], k - 1)
                .into_iter()
                .map(move |mut rest| {
                    rest.insert(0, items[i]);
                    rest
                })
        })
        .collect()
}

/// Strength of exactly five cards, through the public API.
/// (5-card ranking is verified exhaustively in the unit tests.)
fn five_card(hand: &[Card]) -> pi_poker_eval::HandStrength {
    Evaluator
        .evaluate(Variant::Holdem, &hand[..2], &hand[2..])
        .unwrap()
        .strength
}

/// Hold'em: the 7-card result equals the best of its 21 five-card subsets.
#[test]
fn holdem_seven_cards_equal_best_five_card_subset() {
    let mut rng = Lcg(20_261_018);
    for _ in 0..20_000 {
        let deal = rng.deal(7);
        let seven = Evaluator
            .evaluate(Variant::Holdem, &deal[..2], &deal[2..])
            .unwrap()
            .strength;
        let best = subsets(&deal, 5)
            .iter()
            .map(|s| five_card(s))
            .max()
            .unwrap();
        assert_eq!(seven, best, "{deal:?}");
    }
}

/// For every variant: the best five are real cards from the input, they alone rank the
/// same as the whole hand, Omaha's use exactly two hole cards, and card order doesn't matter.
#[test]
fn best_cards_are_consistent() {
    let mut rng = Lcg(7);
    for variant in [
        Variant::Holdem,
        Variant::Omaha4,
        Variant::Omaha5,
        Variant::Omaha6,
    ] {
        let h = variant.hole_cards();
        for _ in 0..5_000 {
            let deal = rng.deal(h + 5);
            let (hole, board) = deal.split_at(h);
            let e = Evaluator.evaluate(variant, hole, board).unwrap();

            assert_eq!(e.cards.len(), 5);
            assert!(
                e.cards.iter().all(|c| deal.contains(c)),
                "{variant} {deal:?}"
            );
            assert_eq!(five_card(&e.cards), e.strength, "{variant} {deal:?}");
            if variant != Variant::Holdem {
                assert_eq!(
                    e.cards.iter().filter(|c| hole.contains(c)).count(),
                    2,
                    "{variant} {deal:?}"
                );
            }

            // Reversing the order of the cards changes nothing.
            let (mut hole_rev, mut board_rev) = (hole.to_vec(), board.to_vec());
            hole_rev.reverse();
            board_rev.reverse();
            assert_eq!(
                Evaluator
                    .evaluate(variant, &hole_rev, &board_rev)
                    .unwrap()
                    .strength,
                e.strength
            );
        }
    }
}
