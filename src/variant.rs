//! Game variants and their hand-selection rules.
//!
//! Each variant decides which cards a player may combine into a hand. That rule
//! is a Strategy (`HandSelection`): Hold'em uses any of the available cards,
//! Omaha must use exactly two hole cards and three board cards. Adding a variant
//! later (Short Deck, Hi-Lo) means adding a selection, not touching the ranking.

use std::fmt;
use std::str::FromStr;

use crate::card::Card;
use crate::error::EvalError;
use crate::rank::rank_unchecked;
use crate::strength::HandStrength;

/// A poker variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variant {
    /// Texas Hold'em: 2 hole cards, any five of the seven play.
    Holdem,
    /// Pot Limit Omaha: 4 hole cards, exactly 2 of them plus 3 board cards.
    Omaha4,
    /// Five-card Omaha: 5 hole cards, exactly 2 plus 3.
    Omaha5,
    /// Six-card Omaha: 6 hole cards, exactly 2 plus 3.
    Omaha6,
}

impl Variant {
    /// How many hole cards each player holds.
    pub const fn hole_cards(self) -> usize {
        match self {
            Variant::Holdem => 2,
            Variant::Omaha4 => 4,
            Variant::Omaha5 => 5,
            Variant::Omaha6 => 6,
        }
    }

    /// The short code, also used by the JavaScript build: `NLH`, `PLO`, `PLO5`, `PLO6`.
    pub const fn code(self) -> &'static str {
        match self {
            Variant::Holdem => "NLH",
            Variant::Omaha4 => "PLO",
            Variant::Omaha5 => "PLO5",
            Variant::Omaha6 => "PLO6",
        }
    }

    pub(crate) fn selection(self) -> &'static dyn HandSelection {
        match self {
            Variant::Holdem => &AnyCards,
            Variant::Omaha4 | Variant::Omaha5 | Variant::Omaha6 => &TwoPlusThree,
        }
    }

    /// Checks hole-card count and board size for this variant.
    pub(crate) fn validate(self, hole: &[Card], board: &[Card]) -> Result<(), EvalError> {
        if hole.len() != self.hole_cards() {
            return Err(EvalError::WrongHoleCardCount {
                variant: self,
                expected: self.hole_cards(),
                got: hole.len(),
            });
        }
        if !matches!(board.len(), 0 | 3 | 4 | 5) {
            return Err(EvalError::InvalidBoardSize(board.len()));
        }
        Ok(())
    }
}

impl FromStr for Variant {
    type Err = String;

    /// Parses the codes `NLH`, `PLO`, `PLO5`, `PLO6` (case-insensitive).
    fn from_str(text: &str) -> Result<Variant, String> {
        match text.to_ascii_uppercase().as_str() {
            "NLH" => Ok(Variant::Holdem),
            "PLO" => Ok(Variant::Omaha4),
            "PLO5" => Ok(Variant::Omaha5),
            "PLO6" => Ok(Variant::Omaha6),
            _ => Err(format!(
                "unknown variant: {text:?} (expected NLH, PLO, PLO5 or PLO6)"
            )),
        }
    }
}

impl fmt::Display for Variant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

/// The best hand a player can make, and the cards it is made from.
pub(crate) struct Selected {
    pub strength: HandStrength,
    /// The cards the best hand is chosen from (first `len`): every available card
    /// for Hold'em, the winning 2 + 3 combination for Omaha. Fixed-size: no allocation.
    cards: [Card; 7],
    len: usize,
}

impl Selected {
    fn new(strength: HandStrength, from: &[Card]) -> Selected {
        let mut cards = [from[0]; 7];
        cards[..from.len()].copy_from_slice(from);
        Selected {
            strength,
            cards,
            len: from.len(),
        }
    }

    /// The cards the best hand is chosen from.
    pub fn pool(&self) -> &[Card] {
        &self.cards[..self.len]
    }
}

/// Which cards a player may combine into a hand.
pub(crate) trait HandSelection: Sync {
    /// The best hand's strength only — the allocation-free hot path used by equity.
    fn strength(&self, hole: &[Card], board: &[Card]) -> HandStrength;

    /// The best hand's strength and the cards it is made from.
    fn best(&self, hole: &[Card], board: &[Card]) -> Selected;
}

/// Hold'em: any of the hole and board cards.
struct AnyCards;

impl HandSelection for AnyCards {
    #[inline]
    fn strength(&self, hole: &[Card], board: &[Card]) -> HandStrength {
        let mut cards = [hole[0]; 7];
        let n = hole.len() + board.len();
        cards[..hole.len()].copy_from_slice(hole);
        cards[hole.len()..n].copy_from_slice(board);
        rank_unchecked(&cards[..n])
    }

    fn best(&self, hole: &[Card], board: &[Card]) -> Selected {
        let mut cards = [hole[0]; 7];
        let n = hole.len() + board.len();
        cards[..hole.len()].copy_from_slice(hole);
        cards[hole.len()..n].copy_from_slice(board);
        Selected::new(rank_unchecked(&cards[..n]), &cards[..n])
    }
}

/// Omaha: exactly two hole cards plus exactly three board cards.
/// With no board yet (preflop), the best two hole cards.
struct TwoPlusThree;

impl TwoPlusThree {
    /// The strongest combination: its strength and the cards in it (2 preflop, else 5).
    /// On a tie the first combination is kept, which is harmless — tied combinations
    /// have identical strength.
    #[inline]
    fn scan(hole: &[Card], board: &[Card]) -> (HandStrength, [Card; 5], usize) {
        let mut best = (
            HandStrength::from_ranks(crate::strength::Category::HighCard, [0; 5]),
            [hole[0]; 5],
            0,
        );
        let mut hand = [hole[0]; 5];
        for i in 0..hole.len() {
            for j in i + 1..hole.len() {
                hand[0] = hole[i];
                hand[1] = hole[j];
                if board.is_empty() {
                    let s = rank_unchecked(&hand[..2]);
                    if best.2 == 0 || s > best.0 {
                        best = (s, hand, 2);
                    }
                    continue;
                }
                for a in 0..board.len() {
                    for b in a + 1..board.len() {
                        for c in b + 1..board.len() {
                            hand[2] = board[a];
                            hand[3] = board[b];
                            hand[4] = board[c];
                            let s = rank_unchecked(&hand);
                            if best.2 == 0 || s > best.0 {
                                best = (s, hand, 5);
                            }
                        }
                    }
                }
            }
        }
        best
    }
}

impl HandSelection for TwoPlusThree {
    #[inline]
    fn strength(&self, hole: &[Card], board: &[Card]) -> HandStrength {
        Self::scan(hole, board).0
    }

    fn best(&self, hole: &[Card], board: &[Card]) -> Selected {
        let (strength, hand, n) = Self::scan(hole, board);
        Selected::new(strength, &hand[..n])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_variant_codes() {
        assert_eq!("NLH".parse::<Variant>(), Ok(Variant::Holdem));
        assert_eq!("plo".parse::<Variant>(), Ok(Variant::Omaha4));
        assert_eq!("PLO5".parse::<Variant>(), Ok(Variant::Omaha5));
        assert_eq!("PLO6".parse::<Variant>(), Ok(Variant::Omaha6));
        assert!("PLO7".parse::<Variant>().is_err());
    }
}
