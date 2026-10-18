//! The public evaluation interface and its implementation.

use crate::card::Card;
use crate::describe::{best_cards, description};
use crate::error::EvalError;
use crate::strength::{Category, HandStrength};
use crate::variant::Variant;

/// A player's best hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedHand {
    /// Comparable strength: higher wins, equal is a tie.
    pub strength: HandStrength,
    /// The hand's type.
    pub category: Category,
    /// The best five cards (fewer on a short hand), in display order.
    pub cards: Vec<Card>,
}

impl EvaluatedHand {
    /// The hand type's name, e.g. `"Flush"`.
    pub fn name(&self) -> &'static str {
        self.category.name()
    }

    /// The readable description, e.g. `"Flush, A High"`.
    pub fn description(&self) -> String {
        description(self.strength)
    }
}

/// Every player's best hand at showdown, and who wins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Showdown {
    /// Each player's best hand, in the same order as the hole cards were given.
    pub hands: Vec<EvaluatedHand>,
    /// Indices of the winning players, lowest first. More than one means a split pot.
    pub winners: Vec<usize>,
}

/// The evaluation contract. Depend on this trait and inject it, rather than constructing
/// an evaluator inline, so the implementation can be swapped or mocked.
pub trait HandEvaluator {
    /// The best hand `hole` can make with `board` under `variant`'s rules.
    fn evaluate(
        &self,
        variant: Variant,
        hole: &[Card],
        board: &[Card],
    ) -> Result<EvaluatedHand, EvalError>;

    /// Every player's best hand with the shared `board`, plus the winners
    /// (more than one means a split pot).
    ///
    /// `holes[i]` is player `i`'s hole cards. No card may appear twice across
    /// all players and the board.
    fn showdown(
        &self,
        variant: Variant,
        board: &[Card],
        holes: &[&[Card]],
    ) -> Result<Showdown, EvalError>;
}

/// The library's evaluator.
#[derive(Debug, Clone, Copy, Default)]
pub struct Evaluator;

impl HandEvaluator for Evaluator {
    fn evaluate(
        &self,
        variant: Variant,
        hole: &[Card],
        board: &[Card],
    ) -> Result<EvaluatedHand, EvalError> {
        variant.validate(hole, board)?;
        check_no_duplicates(hole.iter().chain(board))?;
        let selected = variant.selection().best(hole, board);
        Ok(EvaluatedHand {
            strength: selected.strength,
            category: selected.strength.category(),
            cards: best_cards(selected.strength, selected.pool()),
        })
    }

    fn showdown(
        &self,
        variant: Variant,
        board: &[Card],
        holes: &[&[Card]],
    ) -> Result<Showdown, EvalError> {
        if holes.is_empty() {
            return Err(EvalError::NoPlayers);
        }
        check_no_duplicates(holes.iter().flat_map(|h| h.iter()).chain(board))?;
        let hands = holes
            .iter()
            .map(|hole| self.evaluate(variant, hole, board))
            .collect::<Result<Vec<_>, _>>()?;

        let best = hands
            .iter()
            .map(|h| h.strength)
            .max()
            .expect("at least one player");
        let winners = hands
            .iter()
            .enumerate()
            .filter(|(_, h)| h.strength == best)
            .map(|(i, _)| i)
            .collect();
        Ok(Showdown { hands, winners })
    }
}

/// Fails on the first card that appears twice.
pub(crate) fn check_no_duplicates<'a>(
    cards: impl Iterator<Item = &'a Card>,
) -> Result<(), EvalError> {
    let mut seen = 0u64;
    for card in cards {
        let bit = 1u64 << card.index();
        if seen & bit != 0 {
            return Err(EvalError::DuplicateCard(*card));
        }
        seen |= bit;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rank::rank_cards;

    fn eval(variant: Variant, hole: &str, board: &str) -> EvaluatedHand {
        Evaluator
            .evaluate(
                variant,
                &Card::parse_list(hole).unwrap(),
                &Card::parse_list(board).unwrap(),
            )
            .unwrap()
    }

    fn strength(variant: Variant, hole: &str, board: &str) -> HandStrength {
        eval(variant, hole, board).strength
    }

    #[test]
    fn holdem_uses_any_cards() {
        let s = strength(Variant::Holdem, "Ah Kh", "Qh Jh Th 2c 3d");
        assert_eq!(s.category(), Category::StraightFlush);
        assert_eq!(
            s,
            rank_cards(&Card::parse_list("Ah Kh Qh Jh Th 2c 3d").unwrap()).unwrap()
        );
        // The board can play on its own.
        assert_eq!(
            strength(Variant::Holdem, "2c 3d", "Ah Kh Qh Jh Th").category(),
            Category::StraightFlush
        );
    }

    // ---- Omaha: exactly two hole cards plus three board cards ----

    #[test]
    fn omaha_needs_two_hole_cards_for_a_flush() {
        // One heart in hand, four on board: no flush in Omaha (but a flush in Hold'em terms).
        let e = eval(Variant::Omaha4, "Ah Kd Qc Js", "2h 5h 8h 9h 3c");
        assert_ne!(e.category, Category::Flush);
        // A flush on the board alone does not play either.
        assert_ne!(
            eval(Variant::Omaha4, "Ac Kd Qc Js", "2h 5h 8h 9h Th").category,
            Category::Flush
        );
        // Two hearts in hand plus three on board: a flush.
        assert_eq!(
            eval(Variant::Omaha4, "Ah Kh Qc Js", "2h 5h 8h 9c 3c").category,
            Category::Flush
        );
    }

    #[test]
    fn omaha_needs_two_hole_cards_for_a_straight() {
        // 9 alone with 5-6-7-8 on board is not a straight: two hole cards must play.
        assert_ne!(
            eval(Variant::Omaha4, "9h 2c 2d Kd", "5c 6d 7s 8h Qc").category,
            Category::Straight
        );
        // T-9 with 6-7-8 is.
        let e = eval(Variant::Omaha4, "Th 9c 2d Kd", "5c 6d 7s 8h Qc");
        assert_eq!(e.category, Category::Straight);
        assert_eq!(e.strength.tiebreaks().next().unwrap().value(), 10);
    }

    #[test]
    fn omaha_limits_hole_and_board_cards() {
        // Four aces in hand: only two can play.
        assert_eq!(
            eval(Variant::Omaha4, "Ah Ad Ac As", "2c 7d 9s Jh Kc").category,
            Category::Pair
        );
        // Quads on board: only three of them can play.
        assert_eq!(
            eval(Variant::Omaha4, "Ah Kd Qc Js", "9c 9d 9s 9h 2c").category,
            Category::ThreeOfAKind
        );
        // Trips on board plus a pocket pair: a full house.
        let e = eval(Variant::Omaha4, "Kh Kd 2c 3s", "Ac Ad As 7h 8c");
        assert_eq!(e.category, Category::FullHouse);
    }

    #[test]
    fn omaha_picks_the_best_combination_within_a_category() {
        // The best combination wins, not the first one found: A-high flush, not 10-high.
        let ace_flush = strength(Variant::Omaha4, "Tc 9c Qc Ac", "3c 5h 6c 9s 7c");
        let jack_flush = strength(Variant::Omaha4, "Jc 2d 2c Ts", "3c 5h 6c 9s 7c");
        assert_eq!(ace_flush.category(), Category::Flush);
        assert_eq!(ace_flush.tiebreaks().next().unwrap().value(), 14);
        assert!(ace_flush > jack_flush);

        // Kings-and-fours over sixes-and-fours, in PLO, PLO5 and PLO6.
        assert!(
            strength(Variant::Omaha4, "Kh 2s Kc 5c", "2c 6h 4s 8s 4h")
                > strength(Variant::Omaha4, "3c 7h 9d 6s", "2c 6h 4s 8s 4h")
        );
        assert!(
            strength(Variant::Omaha5, "Kh 2h Kc 5c Ac", "2c 6h 4c 8s 4h")
                > strength(Variant::Omaha5, "3c 7c 9h 6s Jc", "2c 6h 4c 8s 4h")
        );
        assert!(
            strength(Variant::Omaha6, "7h Kh 7c 4s Qc Qs", "3c As 9h Ac 5s")
                > strength(Variant::Omaha6, "Jc Ks 2h 8c 2c 9s", "3c As 9h Ac 5s")
        );
    }

    #[test]
    fn omaha_on_partial_boards() {
        // Preflop: the best two hole cards.
        let e = eval(Variant::Omaha4, "Ah Ad Kc Qs", "");
        assert_eq!(e.category, Category::Pair);
        assert_eq!(
            e.strength
                .tiebreaks()
                .map(|r| r.value())
                .collect::<Vec<_>>(),
            [14]
        );
        // Flop and turn: two hole cards plus three of the board.
        assert_eq!(
            eval(Variant::Omaha4, "Ah Ad Kc Qs", "Ac 7d 2s").category,
            Category::ThreeOfAKind
        );
        assert_eq!(
            eval(Variant::Omaha4, "Ah Kh Qc Js", "2h 5h 8h 9c").category,
            Category::Flush
        );
        assert_eq!(
            eval(Variant::Omaha6, "Ah Kh Qc Js Tc 9d", "2h 5h 8h").category,
            Category::Flush
        );
    }

    // ---- Best cards and descriptions ----

    fn shown(variant: Variant, hole: &str, board: &str) -> (String, String, String) {
        let e = eval(variant, hole, board);
        let cards = e
            .cards
            .iter()
            .map(Card::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        (e.name().to_string(), e.description(), cards)
    }

    #[test]
    fn describes_every_category() {
        let cases = [
            (
                "Ah Kd",
                "9c 7s 2h 3d 4c",
                "High Card",
                "A High",
                "Ah Kd 9c 7s 4c",
            ),
            (
                "Ah Ad",
                "9c 7s 2h 3d 5c",
                "Pair",
                "Pair, A's",
                "Ah Ad 9c 7s 5c",
            ),
            (
                "Th Td",
                "9c 7s 2h 3d 5c",
                "Pair",
                "Pair, 10's",
                "Th Td 9c 7s 5c",
            ),
            (
                "Ah Ad",
                "9c 9s 2h 3d 5c",
                "Two Pair",
                "Two Pair, A's & 9's",
                "Ah Ad 9c 9s 5c",
            ),
            (
                "Ah Ad",
                "Ac 7s 2h 3d 5c",
                "Three of a Kind",
                "Three of a Kind, A's",
                "Ah Ad Ac 7s 5c",
            ),
            (
                "Ah Kd",
                "Qc Js Th 3d 4c",
                "Straight",
                "Straight, A High",
                "Ah Kd Qc Js Th",
            ),
            (
                "Ah Kh",
                "9h 7h 2h 3d 4c",
                "Flush",
                "Flush, A High",
                "Ah Kh 9h 7h 2h",
            ),
            (
                "Th 9h",
                "8h 7h 2h 3d 4c",
                "Flush",
                "Flush, 10 High",
                "Th 9h 8h 7h 2h",
            ),
            (
                "Ah Ad",
                "Ac 7s 7h 3d 4c",
                "Full House",
                "Full House, A's over 7's",
                "Ah Ad Ac 7s 7h",
            ),
            (
                "Th Td",
                "Tc 9s 9h 3d 4c",
                "Full House",
                "Full House, 10's over 9's",
                "Th Td Tc 9s 9h",
            ),
            (
                "9h 9d",
                "9c 9s Ah Kd 2c",
                "Four of a Kind",
                "Four of a Kind, 9's",
                "9h 9d 9c 9s Ah",
            ),
            (
                "9h Th",
                "Jh Qh Kh 3d 4c",
                "Straight Flush",
                "Straight Flush, K High",
                "Kh Qh Jh Th 9h",
            ),
            (
                "Ah Kh",
                "Qh Jh Th 3d 4c",
                "Straight Flush",
                "Royal Flush",
                "Ah Kh Qh Jh Th",
            ),
        ];
        for (hole, board, name, desc, cards) in cases {
            assert_eq!(
                shown(Variant::Holdem, hole, board),
                (name.into(), desc.into(), cards.into()),
                "{hole} | {board}"
            );
        }
    }

    #[test]
    fn corrections_to_todays_output() {
        // 1. The wheel's ace is an ace (pokersolver reports rank "1"; the engine then loses it).
        assert_eq!(
            shown(Variant::Holdem, "Ah 2d", "3c 4s 5h Kd 9c"),
            (
                "Straight".into(),
                "Straight, 5 High".into(),
                "5h 4s 3c 2d Ah".into()
            )
        );
        assert_eq!(
            shown(Variant::Holdem, "Ah 2h", "3h 4h 5h Kd 9c"),
            (
                "Straight Flush".into(),
                "Straight Flush, 5 High".into(),
                "5h 4h 3h 2h Ah".into()
            )
        );
        // 2. Two sets of trips: five best cards, not six.
        assert_eq!(
            shown(Variant::Holdem, "Ah Ad", "Ac Kd Kc Ks 2c").2,
            "Ah Ad Ac Kd Kc"
        );
        // 3. No suit letter in flush descriptions (covered above: "Flush, A High").
    }

    #[test]
    fn best_cards_from_seven() {
        // Six hearts: the top five.
        assert_eq!(
            shown(Variant::Holdem, "Ah Kh", "9h 7h 4h 2h 3c").2,
            "Ah Kh 9h 7h 4h"
        );
        // Three pairs: the third pair's higher card is the kicker.
        assert_eq!(
            shown(Variant::Holdem, "Ah Ad", "Kc Kd 3s 3h Qc").2,
            "Ah Ad Kc Kd Qc"
        );
        // Quads with trips beside: one card of the trips is the kicker.
        assert_eq!(
            shown(Variant::Holdem, "9h 9d", "9c 9s Ah Ad Ac").2,
            "9h 9d 9c 9s Ah"
        );
        // Straight with a paired card: one card per rank.
        assert_eq!(
            shown(Variant::Holdem, "8h 8d", "9c Ts Jh Qd 2c").2,
            "Qd Jh Ts 9c 8h"
        );
    }

    #[test]
    fn describes_short_hands() {
        assert_eq!(
            shown(Variant::Holdem, "Ah Kd", ""),
            ("High Card".into(), "A High".into(), "Ah Kd".into())
        );
        assert_eq!(
            shown(Variant::Holdem, "Ah Ad", ""),
            ("Pair".into(), "Pair, A's".into(), "Ah Ad".into())
        );
        assert_eq!(
            shown(Variant::Omaha4, "Ah Ad Kc Qs", ""),
            ("Pair".into(), "Pair, A's".into(), "Ah Ad".into())
        );
    }

    #[test]
    fn omaha_best_cards_use_two_hole_and_three_board() {
        // A-high flush from A♣ Q♣ plus three board clubs.
        assert_eq!(
            shown(Variant::Omaha4, "Tc 9c Qc Ac", "3c 5h 6c 9s 7c"),
            (
                "Flush".into(),
                "Flush, A High".into(),
                "Ac Qc 7c 6c 3c".into()
            )
        );
        assert_eq!(
            shown(Variant::Omaha4, "Kh 2s Kc 5c", "2c 6h 4s 8s 4h"),
            (
                "Two Pair".into(),
                "Two Pair, K's & 4's".into(),
                "Kh Kc 4s 4h 8s".into()
            )
        );
        // Exactly two hole cards are among the best five.
        let e = eval(Variant::Omaha6, "Ah Kh Qc Js Tc 9d", "2h 5h 8h 3c 4d");
        let hole = Card::parse_list("Ah Kh Qc Js Tc 9d").unwrap();
        assert_eq!(e.cards.iter().filter(|c| hole.contains(c)).count(), 2);
        assert_eq!(e.description(), "Flush, A High");
    }

    // ---- Showdown: winners and split pots ----

    fn showdown(variant: Variant, board: &str, holes: &[&str]) -> Result<Showdown, EvalError> {
        let board = Card::parse_list(board).unwrap();
        let holes: Vec<Vec<Card>> = holes.iter().map(|h| Card::parse_list(h).unwrap()).collect();
        let refs: Vec<&[Card]> = holes.iter().map(Vec::as_slice).collect();
        Evaluator.showdown(variant, &board, &refs)
    }

    fn winners(variant: Variant, board: &str, holes: &[&str]) -> Vec<usize> {
        showdown(variant, board, holes).unwrap().winners
    }

    #[test]
    fn single_winner() {
        assert_eq!(
            winners(
                Variant::Holdem,
                "2c 7d 9s Jh Kc",
                &["Ah Ad", "Kh Kd", "Qc Qd"]
            ),
            [1]
        );
        // Kicker decides.
        assert_eq!(
            winners(Variant::Holdem, "Ac 7d 9s 4h 2c", &["Ah Kd", "As Qd"]),
            [0]
        );
    }

    #[test]
    fn split_pots() {
        // The board plays for everyone.
        assert_eq!(
            winners(
                Variant::Holdem,
                "Ah Kh Qh Jh Th",
                &["2c 3d", "4c 5d", "6c 7d"]
            ),
            [0, 1, 2]
        );
        // Same hand, different suits.
        assert_eq!(
            winners(
                Variant::Holdem,
                "2c 7d 9s Jh Kc",
                &["Ah Qd", "As Qc", "3h 4h"]
            ),
            [0, 1]
        );
        // Kickers beyond the best five do not count.
        assert_eq!(
            winners(Variant::Holdem, "Ac Ad Kc Ks Qh", &["2c 3d", "Jh 9h"]),
            [0, 1]
        );
    }

    #[test]
    fn omaha_showdowns() {
        // The stronger hand of the same type wins.
        assert_eq!(
            winners(
                Variant::Omaha4,
                "3c 5h 6c 9s 7c",
                &["Tc 9c Qc Ac", "Jc 2d 2c Ts"]
            ),
            [0]
        );
        assert_eq!(
            winners(
                Variant::Omaha4,
                "2c 6h 4s 8s 4h",
                &["Kh 2s Kc 5c", "3c 7h 9d 6s"]
            ),
            [0]
        );
        // A straight beats three of a kind.
        assert_eq!(
            winners(
                Variant::Omaha4,
                "As 9c 5d Jh 4c",
                &["Ah Ad Kc Qs", "2d 3h 7s 8d"]
            ),
            [1]
        );
        // Board flush plays for no one in Omaha; the best two-plus-three wins.
        let s = showdown(
            Variant::Omaha4,
            "2h 5h 8h 9h Th",
            &["Ac Ad Kc Ks", "Qc Js 3d 4d"],
        )
        .unwrap();
        assert_eq!(s.winners, [1]); // Q-J with 8-9-T: a queen-high straight
        assert_eq!(s.hands[1].description(), "Straight, Q High");
    }

    #[test]
    fn showdown_returns_every_hand_in_order() {
        let s = showdown(Variant::Holdem, "2c 7d 9s Jh Kc", &["Ah Ad", "Kh Kd"]).unwrap();
        assert_eq!(s.hands.len(), 2);
        assert_eq!(s.hands[0].description(), "Pair, A's");
        assert_eq!(s.hands[1].description(), "Three of a Kind, K's");
    }

    #[test]
    fn showdown_validation() {
        assert_eq!(
            showdown(Variant::Holdem, "2c 7d 9s Jh Kc", &[]),
            Err(EvalError::NoPlayers)
        );
        // A card held by two players.
        assert_eq!(
            showdown(Variant::Holdem, "2c 7d 9s Jh Kc", &["Ah Ad", "Ah Kd"]),
            Err(EvalError::DuplicateCard("Ah".parse().unwrap()))
        );
        // One player with the wrong number of hole cards.
        assert_eq!(
            showdown(
                Variant::Omaha4,
                "2c 7d 9s Jh Kc",
                &["Ah Ad Ks Qs", "Qh Qd Jc"]
            ),
            Err(EvalError::WrongHoleCardCount {
                variant: Variant::Omaha4,
                expected: 4,
                got: 3
            })
        );
    }

    // ---- Validation ----

    #[test]
    fn rejects_wrong_hole_card_counts() {
        let cards = Card::parse_list("Ah Kh Qh").unwrap();
        assert_eq!(
            Evaluator.evaluate(Variant::Omaha4, &cards, &[]),
            Err(EvalError::WrongHoleCardCount {
                variant: Variant::Omaha4,
                expected: 4,
                got: 3
            })
        );
        assert_eq!(
            Evaluator.evaluate(Variant::Holdem, &cards, &[]),
            Err(EvalError::WrongHoleCardCount {
                variant: Variant::Holdem,
                expected: 2,
                got: 3
            })
        );
    }

    #[test]
    fn rejects_invalid_board_sizes() {
        let hole = Card::parse_list("Ah Kh").unwrap();
        for n in [1, 2, 6] {
            let board: Vec<Card> = Card::parse_list("2c 3c 4c 5c 6c 7c")
                .unwrap()
                .into_iter()
                .take(n)
                .collect();
            assert_eq!(
                Evaluator.evaluate(Variant::Holdem, &hole, &board),
                Err(EvalError::InvalidBoardSize(n))
            );
        }
    }

    #[test]
    fn rejects_duplicate_cards() {
        let hole = Card::parse_list("Ah Kh").unwrap();
        let board = Card::parse_list("Ah 2c 3d").unwrap();
        assert_eq!(
            Evaluator.evaluate(Variant::Holdem, &hole, &board),
            Err(EvalError::DuplicateCard("Ah".parse().unwrap()))
        );
        let hole = Card::parse_list("Ah Ah Kc Kd").unwrap();
        assert_eq!(
            Evaluator.evaluate(Variant::Omaha4, &hole, &[]),
            Err(EvalError::DuplicateCard("Ah".parse().unwrap()))
        );
    }
}
