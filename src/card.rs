//! Cards: rank, suit, and parsing from strings such as `"Ah"`, `"Td"` or `"10d"`.

use std::fmt;
use std::str::FromStr;

use crate::error::EvalError;

/// A card rank. The numeric value is the rank's strength: `Two` = 2 … `Ace` = 14.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Rank {
    Two = 2,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
    Ace,
}

impl Rank {
    /// All ranks, lowest first.
    pub const ALL: [Rank; 13] = [
        Rank::Two,
        Rank::Three,
        Rank::Four,
        Rank::Five,
        Rank::Six,
        Rank::Seven,
        Rank::Eight,
        Rank::Nine,
        Rank::Ten,
        Rank::Jack,
        Rank::Queen,
        Rank::King,
        Rank::Ace,
    ];

    /// The rank's strength: 2 for `Two` … 14 for `Ace`.
    pub const fn value(self) -> u8 {
        self as u8
    }

    /// The rank with strength `value` (2–14).
    pub(crate) const fn from_value(value: u8) -> Rank {
        Rank::ALL[(value - 2) as usize]
    }

    /// The character used in card strings: `"2"`–`"9"`, `"T"`, `"J"`, `"Q"`, `"K"`, `"A"`.
    pub const fn symbol(self) -> char {
        match self {
            Rank::Two => '2',
            Rank::Three => '3',
            Rank::Four => '4',
            Rank::Five => '5',
            Rank::Six => '6',
            Rank::Seven => '7',
            Rank::Eight => '8',
            Rank::Nine => '9',
            Rank::Ten => 'T',
            Rank::Jack => 'J',
            Rank::Queen => 'Q',
            Rank::King => 'K',
            Rank::Ace => 'A',
        }
    }
}

/// A card suit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    /// All suits.
    pub const ALL: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];

    /// The character used in card strings: `c`, `d`, `h`, `s`.
    pub const fn symbol(self) -> char {
        match self {
            Suit::Clubs => 'c',
            Suit::Diamonds => 'd',
            Suit::Hearts => 'h',
            Suit::Spades => 's',
        }
    }
}

/// A playing card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Card {
    pub rank: Rank,
    pub suit: Suit,
}

impl Card {
    pub const fn new(rank: Rank, suit: Suit) -> Card {
        Card { rank, suit }
    }

    /// A unique index 0–51 (`rank` major), used for bit sets of cards.
    pub const fn index(self) -> u8 {
        (self.rank as u8 - 2) * 4 + self.suit as u8
    }

    /// The card with index `index` (0–51, as returned by [`Card::index`]), if valid.
    pub const fn from_index(index: u8) -> Option<Card> {
        if index > 51 {
            return None;
        }
        Some(Card {
            rank: Rank::ALL[(index / 4) as usize],
            suit: Suit::ALL[(index % 4) as usize],
        })
    }

    /// Parses a space-separated list such as `"Ah Kd Tc"`.
    pub fn parse_list(text: &str) -> Result<Vec<Card>, EvalError> {
        text.split_whitespace().map(str::parse).collect()
    }
}

impl FromStr for Card {
    type Err = EvalError;

    /// Accepts a rank (`2`–`9`, `T` or `10`, `J`, `Q`, `K`, `A`) followed by a suit
    /// (`c`, `d`, `h`, `s`). Case-insensitive: `"Ah"`, `"ah"`, `"10D"` and `"tD"` all parse.
    fn from_str(text: &str) -> Result<Card, EvalError> {
        let invalid = || EvalError::InvalidCard(text.to_string());
        let lower = text.trim().to_ascii_lowercase();
        if lower.len() < 2 || !lower.is_ascii() {
            return Err(invalid());
        }
        let (rank_text, suit_text) = lower.split_at(lower.len() - 1);

        let rank = match rank_text {
            "2" => Rank::Two,
            "3" => Rank::Three,
            "4" => Rank::Four,
            "5" => Rank::Five,
            "6" => Rank::Six,
            "7" => Rank::Seven,
            "8" => Rank::Eight,
            "9" => Rank::Nine,
            "t" | "10" => Rank::Ten,
            "j" => Rank::Jack,
            "q" => Rank::Queen,
            "k" => Rank::King,
            "a" => Rank::Ace,
            _ => return Err(invalid()),
        };
        let suit = match suit_text {
            "c" => Suit::Clubs,
            "d" => Suit::Diamonds,
            "h" => Suit::Hearts,
            "s" => Suit::Spades,
            _ => return Err(invalid()),
        };
        Ok(Card { rank, suit })
    }
}

impl fmt::Display for Card {
    /// Writes the canonical form, e.g. `Ah`, `Td`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.rank.symbol(), self.suit.symbol())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_rank_and_suit_spelling() {
        assert_eq!(
            "Ah".parse::<Card>().unwrap(),
            Card::new(Rank::Ace, Suit::Hearts)
        );
        assert_eq!(
            "Td".parse::<Card>().unwrap(),
            Card::new(Rank::Ten, Suit::Diamonds)
        );
        assert_eq!(
            "10d".parse::<Card>().unwrap(),
            Card::new(Rank::Ten, Suit::Diamonds)
        );
        assert_eq!(
            "2c".parse::<Card>().unwrap(),
            Card::new(Rank::Two, Suit::Clubs)
        );
        assert_eq!(
            "ks".parse::<Card>().unwrap(),
            Card::new(Rank::King, Suit::Spades)
        );
        assert_eq!(
            "QH".parse::<Card>().unwrap(),
            Card::new(Rank::Queen, Suit::Hearts)
        );
    }

    #[test]
    fn rejects_invalid_cards() {
        for bad in ["", "A", "1h", "11h", "Ax", "hA", "Ahh", "A♥"] {
            assert_eq!(
                bad.parse::<Card>(),
                Err(EvalError::InvalidCard(bad.to_string())),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn displays_canonical_form() {
        assert_eq!(
            Card::parse_list("10h ad 2c")
                .unwrap()
                .iter()
                .map(Card::to_string)
                .collect::<Vec<_>>(),
            ["Th", "Ad", "2c"]
        );
    }

    #[test]
    fn indices_are_unique_and_dense() {
        let mut seen = [false; 52];
        for rank in Rank::ALL {
            for suit in Suit::ALL {
                let i = Card::new(rank, suit).index() as usize;
                assert!(!seen[i]);
                seen[i] = true;
            }
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn from_index_round_trips() {
        for i in 0..52u8 {
            assert_eq!(Card::from_index(i).unwrap().index(), i);
        }
        assert_eq!(Card::from_index(52), None);
    }
}
