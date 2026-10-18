//! Errors returned by the library.

use std::fmt;

use crate::card::Card;
use crate::variant::Variant;

/// Why a request could not be evaluated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvalError {
    /// A card string that is not a valid card, e.g. `"1h"` or `"Ax"`.
    InvalidCard(String),
    /// The same card appears more than once across the hole cards, board and dead cards.
    DuplicateCard(Card),
    /// A hand of a size the library does not rank (it ranks 1–7 cards).
    InvalidHandSize(usize),
    /// The wrong number of hole cards for the variant (e.g. 3 cards in PLO).
    WrongHoleCardCount {
        variant: Variant,
        expected: usize,
        got: usize,
    },
    /// A board that is not 0, 3, 4 or 5 cards.
    InvalidBoardSize(usize),
    /// A showdown or equity request with no players.
    NoPlayers,
    /// An equity request needs more cards than are left in the deck
    /// (e.g. too many unknown opponents).
    NotEnoughCards { needed: usize, available: usize },
    /// A Monte Carlo request with zero trials.
    InvalidTrials,
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::InvalidCard(text) => write!(f, "invalid card: {text:?}"),
            EvalError::DuplicateCard(card) => write!(f, "duplicate card: {card}"),
            EvalError::InvalidHandSize(n) => {
                write!(f, "cannot rank a hand of {n} cards (expected 1 to 7)")
            }
            EvalError::WrongHoleCardCount {
                variant,
                expected,
                got,
            } => {
                write!(f, "{variant} needs {expected} hole cards, got {got}")
            }
            EvalError::InvalidBoardSize(n) => {
                write!(f, "invalid board of {n} cards (expected 0, 3, 4 or 5)")
            }
            EvalError::NoPlayers => write!(f, "no players given"),
            EvalError::NotEnoughCards { needed, available } => {
                write!(
                    f,
                    "needs {needed} more cards but only {available} are left in the deck"
                )
            }
            EvalError::InvalidTrials => write!(f, "Monte Carlo needs at least one trial"),
        }
    }
}

impl std::error::Error for EvalError {}
