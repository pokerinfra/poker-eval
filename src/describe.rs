//! The best five cards of a hand, and its readable description.
//!
//! The wording follows the conventions used by the popular pokersolver JavaScript
//! library, except where its output is wrong or awkward:
//!  * the ace of an A-2-3-4-5 straight is reported as an ace (pokersolver: rank "1");
//!  * two sets of trips give five best cards, not six;
//!  * flush descriptions carry no suit letter ("Flush, A High", not "Flush, Ah High").

use crate::card::{Card, Rank};
use crate::strength::{Category, HandStrength};

/// The cards that make up `strength`, chosen from `pool`, in display order:
/// the main group first (quads, trips, pairs), then kickers high to low;
/// straights from the top card down, the wheel as 5-4-3-2-A.
/// Short hands (fewer than five cards available) return what they have.
pub(crate) fn best_cards(strength: HandStrength, pool: &[Card]) -> Vec<Card> {
    // Tie-break ranks into a fixed array: no allocation besides the returned Vec.
    let mut ranks = [Rank::Two; 5];
    let mut n_ranks = 0;
    for r in strength.tiebreaks() {
        ranks[n_ranks] = r;
        n_ranks += 1;
    }
    let mut out = Vec::with_capacity(5);

    match strength.category() {
        Category::StraightFlush => {
            let run = straight_run(ranks[0]);
            let suit = pool
                .iter()
                .map(|c| c.suit)
                .find(|&s| run.iter().all(|&r| pool.contains(&Card::new(r, s))))
                .expect("a straight flush has one suit holding the whole run");
            out.extend(run.iter().map(|&r| Card::new(r, suit)));
        }
        Category::Flush => {
            let suit = pool
                .iter()
                .map(|c| c.suit)
                .find(|&s| pool.iter().filter(|c| c.suit == s).count() >= 5)
                .expect("a flush has a suit with five cards");
            out.extend(ranks[..n_ranks].iter().map(|&r| Card::new(r, suit)));
        }
        Category::Straight => {
            for r in straight_run(ranks[0]) {
                push_rank(&mut out, pool, r, 1);
            }
        }
        category => {
            // Group sizes in tie-break order, e.g. a full house takes 3 then 2.
            let groups: &[usize] = match category {
                Category::FourOfAKind => &[4, 1],
                Category::FullHouse => &[3, 2],
                Category::ThreeOfAKind => &[3, 1, 1],
                Category::TwoPair => &[2, 2, 1],
                Category::Pair => &[2, 1, 1, 1],
                _ => &[1, 1, 1, 1, 1],
            };
            for (&r, &n) in ranks[..n_ranks].iter().zip(groups) {
                push_rank(&mut out, pool, r, n);
            }
        }
    }
    out
}

/// The readable description, e.g. `"Two Pair, K's & 4's"`, `"Flush, A High"`, `"Royal Flush"`.
pub(crate) fn description(strength: HandStrength) -> String {
    let r: Vec<&str> = strength.tiebreaks().map(word).collect();
    match strength.category() {
        Category::HighCard => format!("{} High", r[0]),
        Category::Pair => format!("Pair, {}'s", r[0]),
        Category::TwoPair => format!("Two Pair, {}'s & {}'s", r[0], r[1]),
        Category::ThreeOfAKind => format!("Three of a Kind, {}'s", r[0]),
        Category::Straight => format!("Straight, {} High", r[0]),
        Category::Flush => format!("Flush, {} High", r[0]),
        Category::FullHouse => format!("Full House, {}'s over {}'s", r[0], r[1]),
        Category::FourOfAKind => format!("Four of a Kind, {}'s", r[0]),
        Category::StraightFlush if r[0] == "A" => "Royal Flush".to_string(),
        Category::StraightFlush => format!("Straight Flush, {} High", r[0]),
    }
}

/// How a rank is written in descriptions: `2`–`10`, `J`, `Q`, `K`, `A`.
fn word(rank: Rank) -> &'static str {
    match rank {
        Rank::Two => "2",
        Rank::Three => "3",
        Rank::Four => "4",
        Rank::Five => "5",
        Rank::Six => "6",
        Rank::Seven => "7",
        Rank::Eight => "8",
        Rank::Nine => "9",
        Rank::Ten => "10",
        Rank::Jack => "J",
        Rank::Queen => "Q",
        Rank::King => "K",
        Rank::Ace => "A",
    }
}

/// The five ranks of a straight topped by `top`, high first; the wheel ends with the ace.
fn straight_run(top: Rank) -> [Rank; 5] {
    if top == Rank::Five {
        return [Rank::Five, Rank::Four, Rank::Three, Rank::Two, Rank::Ace];
    }
    let t = top.value();
    [t, t - 1, t - 2, t - 3, t - 4].map(Rank::from_value)
}

/// Appends the first `n` cards of rank `rank` in `pool`, in pool order.
fn push_rank(out: &mut Vec<Card>, pool: &[Card], rank: Rank, n: usize) {
    out.extend(pool.iter().filter(|c| c.rank == rank).take(n).copied());
}
