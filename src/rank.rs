//! Ranks a set of 1–7 cards: the best poker hand that can be made from all of them.
//!
//! No lookup tables. Each hand is reduced to a 13-bit rank mask per suit plus a
//! count per rank; flushes, straights and pairs then fall out of a few bit
//! operations. Hands of fewer than five cards (preflop, partial boards) can only
//! make the pair-based categories, with as many kickers as there are cards.

use crate::card::Card;
#[cfg(test)]
use crate::error::EvalError;
use crate::strength::{Category, HandStrength};

/// The best hand that can be made from `cards` (1–7 cards, no duplicates checked here).
/// Checked wrapper over [`rank_unchecked`], used by the tests.
#[cfg(test)]
pub(crate) fn rank_cards(cards: &[Card]) -> Result<HandStrength, EvalError> {
    if cards.is_empty() || cards.len() > 7 {
        return Err(EvalError::InvalidHandSize(cards.len()));
    }
    Ok(rank_unchecked(cards))
}

/// The hot path: ranks 1–7 cards without size checks and without allocating.
///
/// Masks use bit `r` for rank value `r + 2` (bit 0 = Two … bit 12 = Ace).
#[inline]
pub(crate) fn rank_unchecked(cards: &[Card]) -> HandStrength {
    let mut suit_masks = [0u16; 4];
    let mut counts = [0u8; 13];
    for card in cards {
        let r = card.rank as usize - 2;
        suit_masks[card.suit as usize] |= 1 << r;
        counts[r] += 1;
    }

    // Straight flush and flush: at most one suit can hold five of 5–7 cards.
    for &mask in &suit_masks {
        if mask.count_ones() >= 5 {
            if let Some(top) = straight_top(mask) {
                return HandStrength::from_ranks(Category::StraightFlush, [top, 0, 0, 0, 0]);
            }
            return HandStrength::from_ranks(Category::Flush, top_ranks(mask, 5));
        }
    }

    // One mask per group size; `present` holds every rank in the hand.
    let (mut quads, mut trips, mut pairs, mut present) = (0u16, 0u16, 0u16, 0u16);
    for (r, &n) in counts.iter().enumerate() {
        let bit = 1u16 << r;
        match n {
            0 => continue,
            1 => {}
            2 => pairs |= bit,
            3 => trips |= bit,
            _ => quads |= bit,
        }
        present |= bit;
    }

    if quads != 0 {
        // Kicker: the highest other card, whatever group it is in.
        let quad = high_bit(quads);
        return HandStrength::from_ranks(
            Category::FourOfAKind,
            [value(quad), highest(present & !(1 << quad)), 0, 0, 0],
        );
    }

    if trips != 0 {
        // A second set of trips counts as the pair (A-A-A-K-K-K plays A-A-A-K-K).
        let trip = high_bit(trips);
        let pair_candidates = (trips & !(1 << trip)) | pairs;
        if pair_candidates != 0 {
            return HandStrength::from_ranks(
                Category::FullHouse,
                [value(trip), value(high_bit(pair_candidates)), 0, 0, 0],
            );
        }
    }

    if let Some(top) = straight_top(present) {
        return HandStrength::from_ranks(Category::Straight, [top, 0, 0, 0, 0]);
    }

    if trips != 0 {
        let trip = high_bit(trips);
        let k = top_ranks(present & !(1 << trip), 2);
        return HandStrength::from_ranks(Category::ThreeOfAKind, [value(trip), k[0], k[1], 0, 0]);
    }

    if pairs.count_ones() >= 2 {
        // With three pairs, the third pair's rank can be the kicker.
        let high = high_bit(pairs);
        let low = high_bit(pairs & !(1 << high));
        let kicker = highest(present & !(1 << high) & !(1 << low));
        return HandStrength::from_ranks(
            Category::TwoPair,
            [value(high), value(low), kicker, 0, 0],
        );
    }

    if pairs != 0 {
        let pair = high_bit(pairs);
        let k = top_ranks(present & !(1 << pair), 3);
        return HandStrength::from_ranks(Category::Pair, [value(pair), k[0], k[1], k[2], 0]);
    }

    HandStrength::from_ranks(Category::HighCard, top_ranks(present, 5))
}

/// The top rank value of the highest five-in-a-row in `mask`, if any.
/// The ace also plays low, so A-2-3-4-5 (the wheel) returns 5.
#[inline]
fn straight_top(mask: u16) -> Option<u8> {
    // Shift up one and copy the ace into bit 0: bit i now stands for rank value i + 1.
    let extended = (mask << 1) | ((mask >> 12) & 1);
    // Bit j survives only if bits j..=j+4 are all set: a straight from value j+1 to j+5.
    let runs = extended & (extended >> 1) & (extended >> 2) & (extended >> 3) & (extended >> 4);
    (runs != 0).then(|| high_bit(runs) as u8 + 5)
}

/// Index of the highest set bit (`mask` must be non-zero).
#[inline]
fn high_bit(mask: u16) -> u32 {
    15 - mask.leading_zeros()
}

/// Rank value of mask bit `bit`.
#[inline]
fn value(bit: u32) -> u8 {
    bit as u8 + 2
}

/// Rank value of the highest bit in `mask`, or 0 if empty (a short hand with no kicker).
#[inline]
fn highest(mask: u16) -> u8 {
    if mask == 0 { 0 } else { value(high_bit(mask)) }
}

/// The `n` (≤ 5) highest rank values in `mask`, high first, padded with 0.
#[inline]
fn top_ranks(mut mask: u16, n: usize) -> [u8; 5] {
    let mut out = [0u8; 5];
    for slot in out.iter_mut().take(n) {
        if mask == 0 {
            break;
        }
        let bit = high_bit(mask);
        *slot = value(bit);
        mask &= !(1 << bit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Rank, Suit};

    fn rank(text: &str) -> HandStrength {
        rank_cards(&Card::parse_list(text).unwrap()).unwrap()
    }

    fn ranks(strength: HandStrength) -> Vec<u8> {
        strength.tiebreaks().map(Rank::value).collect()
    }

    #[test]
    fn detects_every_category() {
        let cases = [
            (
                "Ah Kd 9c 7s 2h 3d 4c",
                Category::HighCard,
                vec![14, 13, 9, 7, 4],
            ),
            ("Ah Ad 9c 7s 2h 3d 5c", Category::Pair, vec![14, 9, 7, 5]),
            ("Ah Ad 9c 9s 2h 3d 5c", Category::TwoPair, vec![14, 9, 5]),
            (
                "Ah Ad Ac 7s 2h 3d 5c",
                Category::ThreeOfAKind,
                vec![14, 7, 5],
            ),
            ("Ah Kd Qc Js Th 3d 4c", Category::Straight, vec![14]),
            (
                "Ah Kh 9h 7h 2h 3d 4c",
                Category::Flush,
                vec![14, 13, 9, 7, 2],
            ),
            ("Ah Ad Ac 7s 7h 3d 4c", Category::FullHouse, vec![14, 7]),
            ("Ah Ad Ac As 7h 3d 4c", Category::FourOfAKind, vec![14, 7]),
            ("9h Th Jh Qh Kh 3d 4c", Category::StraightFlush, vec![13]),
        ];
        for (hand, category, expected) in cases {
            let s = rank(hand);
            assert_eq!(s.category(), category, "{hand}");
            assert_eq!(ranks(s), expected, "{hand}");
        }
    }

    #[test]
    fn ace_plays_low_in_the_wheel() {
        assert_eq!(ranks(rank("Ah 2d 3c 4s 5h Kd 9c")), [5]);
        assert_eq!(rank("Ah 2d 3c 4s 5h").category(), Category::Straight);
        assert_eq!(
            rank("Ah 2h 3h 4h 5h Kd 9c").category(),
            Category::StraightFlush
        );
        assert_eq!(ranks(rank("Ah 2h 3h 4h 5h")), [5]);
        // A 6-high straight beats the wheel; the wheel beats nothing else in straights.
        assert!(rank("2d 3c 4s 5h 6h") > rank("Ah 2d 3c 4s 5h"));
        // No wrap-around: Q-K-A-2-3 is not a straight.
        assert_eq!(rank("Qh Kd Ac 2s 3h").category(), Category::HighCard);
    }

    #[test]
    fn picks_the_best_five_from_seven() {
        // Six hearts: the five highest play.
        assert_eq!(ranks(rank("Ah Kh 9h 7h 4h 2h 3c")), [14, 13, 9, 7, 4]);
        // Straight and flush both present, not connected: flush wins.
        assert_eq!(rank("5h 6d 7h 8c 9h 2h Jh").category(), Category::Flush);
        // Straight flush inside a longer straight.
        assert_eq!(ranks(rank("4d 5h 6h 7h 8h 9h Td")), [9]);
        // Two sets of trips make a full house with the higher set on top.
        assert_eq!(ranks(rank("Ah Ad Ac Kd Kc Ks 2c")), [14, 13]);
        assert_eq!(ranks(rank("2h 2d 2c Kd Kc Ks 3c")), [13, 2]);
        // Trips plus two pairs: the higher pair completes the house.
        assert_eq!(ranks(rank("9h 9d 9c 5d 5c Qs Qc")), [9, 12]);
        // Quads with trips on the side: the kicker is the trips' rank.
        assert_eq!(ranks(rank("9h 9d 9c 9s Ah Ad Ac")), [9, 14]);
        // Three pairs: the best two play, and the third pair's rank can be the kicker.
        assert_eq!(ranks(rank("Ah Ad Kc Kd Qs Qh 2c")), [14, 13, 12]);
        assert_eq!(ranks(rank("Ah Ad Kc Kd 3s 3h Qc")), [14, 13, 12]);
        // A full house beats a flush that is also present.
        assert_eq!(rank("Ah Ad Ac Kh Kd 2h 7h").category(), Category::FullHouse);
    }

    #[test]
    fn ranks_short_hands() {
        assert_eq!(ranks(rank("Ah Kd")), [14, 13]);
        assert_eq!(rank("Ah Ad").category(), Category::Pair);
        assert_eq!(ranks(rank("Ah Kd Kc")), [13, 14]);
        assert_eq!(ranks(rank("Ah Ad Kc Kd")), [14, 13]);
        assert_eq!(ranks(rank("Ah Ad Ac Kd")), [14, 13]);
        assert_eq!(rank("Ah Ad Ac As").category(), Category::FourOfAKind);
        // Four suited cards are not a flush, four in a row are not a straight.
        assert_eq!(rank("Ah Kh Qh Jh").category(), Category::HighCard);
        assert_eq!(ranks(rank("7c")), [7]);
    }

    #[test]
    fn orders_hands_within_a_category() {
        // Kicker decides a pair.
        assert!(rank("Ah Ad Kc 7s 2h") > rank("Ah Ad Qc 7s 2h"));
        // Second pair decides two pair, then the kicker.
        assert!(rank("Ah Ad Kc Ks 2h") > rank("Ah Ad Qc Qs Kh"));
        assert!(rank("Ah Ad Kc Ks 3h") > rank("Ah Ad Kc Ks 2h"));
        // The lowest card decides a flush.
        assert!(rank("Ah Kh 9h 7h 3h") > rank("Ah Kh 9h 7h 2h"));
        // Pair of the full house decides it once the trips tie.
        assert!(rank("Ah Ad Ac Kd Kc") > rank("Ah Ad Ac Qd Qc"));
        // Different suits, same ranks: a tie.
        assert_eq!(rank("Ah Kd 9c 7s 2h"), rank("As Kc 9d 7h 2c"));
        // Every category beats the one below it.
        assert!(rank("2h 3h 4h 5h 6h") > rank("Ah Ad Ac As Kd"));
        assert!(rank("2h 2d 2c 2s 3h") > rank("Ah Ad Ac Kd Kc"));
        assert!(rank("2h 2d 2c 3s 3h") > rank("Ah Kh Qh Jh 9h"));
        assert!(rank("2h 4h 6h 8h Th") > rank("Ah Kd Qc Js Th"));
        assert!(rank("Ah 2d 3c 4s 5h") > rank("Ah Ad Ac Kd Qc"));
        assert!(rank("2h 2d 2c 3s 4h") > rank("Ah Ad Kc Ks Qh"));
        assert!(rank("2h 2d 3c 3s 4h") > rank("Ah Ad Kc Qs Jh"));
        assert!(rank("2h 2d 3c 4s 5h") > rank("Ah Kd Qc Js 9h"));
    }

    #[test]
    fn rejects_bad_hand_sizes() {
        assert_eq!(rank_cards(&[]), Err(EvalError::InvalidHandSize(0)));
        let eight = Card::parse_list("Ah Kh Qh Jh Th 9h 8h 7h").unwrap();
        assert_eq!(rank_cards(&eight), Err(EvalError::InvalidHandSize(8)));
    }

    /// Every 5-card hand: the textbook count per category and 7,462 distinct strengths.
    #[test]
    // Nested index loops are the clearest way to enumerate combinations.
    #[allow(clippy::needless_range_loop)]
    fn every_five_card_hand() {
        let deck: Vec<Card> = Rank::ALL
            .iter()
            .flat_map(|&r| Suit::ALL.map(|s| Card::new(r, s)))
            .collect();
        let mut counts = [0u64; 10];
        let mut distinct = std::collections::HashSet::new();
        let mut hand = [deck[0]; 5];
        for a in 0..52 {
            hand[0] = deck[a];
            for b in a + 1..52 {
                hand[1] = deck[b];
                for c in b + 1..52 {
                    hand[2] = deck[c];
                    for d in c + 1..52 {
                        hand[3] = deck[d];
                        for e in d + 1..52 {
                            hand[4] = deck[e];
                            let s = rank_cards(&hand).unwrap();
                            counts[s.category() as usize] += 1;
                            distinct.insert(s);
                        }
                    }
                }
            }
        }
        assert_eq!(
            counts[1..],
            [
                1_302_540, 1_098_240, 123_552, 54_912, 10_200, 5_108, 3_744, 624, 40
            ]
        );
        assert_eq!(counts.iter().sum::<u64>(), 2_598_960);
        assert_eq!(distinct.len(), 7_462);
    }

    /// Every 7-card hand (133,784,560): the textbook count per category and
    /// 4,824 distinct strengths.
    /// Slow in debug builds; run with `cargo test --release -- --ignored`.
    #[test]
    #[ignore]
    #[allow(clippy::needless_range_loop)]
    fn every_seven_card_hand() {
        let deck: Vec<Card> = Rank::ALL
            .iter()
            .flat_map(|&r| Suit::ALL.map(|s| Card::new(r, s)))
            .collect();
        let mut counts = [0u64; 10];
        let mut distinct = std::collections::HashSet::new();
        let mut hand = [deck[0]; 7];
        for a in 0..52 {
            hand[0] = deck[a];
            for b in a + 1..52 {
                hand[1] = deck[b];
                for c in b + 1..52 {
                    hand[2] = deck[c];
                    for d in c + 1..52 {
                        hand[3] = deck[d];
                        for e in d + 1..52 {
                            hand[4] = deck[e];
                            for f in e + 1..52 {
                                hand[5] = deck[f];
                                for g in f + 1..52 {
                                    hand[6] = deck[g];
                                    let s = rank_cards(&hand).unwrap();
                                    counts[s.category() as usize] += 1;
                                    distinct.insert(s);
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(
            counts[1..],
            [
                23_294_460, 58_627_800, 31_433_400, 6_461_620, 6_180_020, 4_047_644, 3_473_184,
                224_848, 41_584
            ]
        );
        assert_eq!(counts.iter().sum::<u64>(), 133_784_560);
        assert_eq!(distinct.len(), 4_824);
    }
}
