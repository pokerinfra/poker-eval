#![doc = include_str!("../README.md")]

mod card;
mod describe;
mod equity;
mod error;
mod evaluator;
mod rank;
mod strength;
mod variant;

pub use card::{Card, Rank, Suit};
pub use equity::{
    Equity, EquityCalculator, EquityMethod, EquityRequest, EquityResult, PlayerEquity,
};
pub use error::EvalError;
pub use evaluator::{EvaluatedHand, Evaluator, HandEvaluator, Showdown};
pub use strength::{Category, HandStrength};
pub use variant::Variant;

#[cfg(feature = "wasm")]
mod wasm;
