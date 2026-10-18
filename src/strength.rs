//! Hand categories and the comparable hand strength.

use std::fmt;

use crate::card::Rank;

/// The type of a poker hand, weakest first. Comparing categories orders them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Category {
    HighCard = 1,
    Pair,
    TwoPair,
    ThreeOfAKind,
    Straight,
    Flush,
    FullHouse,
    FourOfAKind,
    StraightFlush,
}

impl Category {
    /// All categories, weakest first.
    pub const ALL: [Category; 9] = [
        Category::HighCard,
        Category::Pair,
        Category::TwoPair,
        Category::ThreeOfAKind,
        Category::Straight,
        Category::Flush,
        Category::FullHouse,
        Category::FourOfAKind,
        Category::StraightFlush,
    ];

    /// The display name, e.g. `"Three of a Kind"`.
    pub const fn name(self) -> &'static str {
        match self {
            Category::HighCard => "High Card",
            Category::Pair => "Pair",
            Category::TwoPair => "Two Pair",
            Category::ThreeOfAKind => "Three of a Kind",
            Category::Straight => "Straight",
            Category::Flush => "Flush",
            Category::FullHouse => "Full House",
            Category::FourOfAKind => "Four of a Kind",
            Category::StraightFlush => "Straight Flush",
        }
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// How strong a hand is. Higher wins; equal means a tie.
///
/// Layout (fits in 24 bits): the category in bits 20–23, then up to five
/// tie-break rank values (2–14, or 0 when a short hand has no card there),
/// four bits each, most significant first. The tie-breaks are, per category:
///
/// | Category | Tie-breaks |
/// |---|---|
/// | High card, Flush | the five ranks, high to low |
/// | Pair | pair rank, then three kickers |
/// | Two pair | high pair, low pair, kicker |
/// | Three of a kind | trips rank, then two kickers |
/// | Straight, Straight flush | top card (5 for A-2-3-4-5) |
/// | Full house | trips rank, pair rank |
/// | Four of a kind | quads rank, kicker |
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HandStrength(u32);

impl HandStrength {
    /// Builds a strength from a category and exactly five tie-break slots
    /// (0 = empty). Allocation-free.
    #[inline]
    pub(crate) const fn from_ranks(category: Category, r: [u8; 5]) -> HandStrength {
        HandStrength(
            ((category as u32) << 20)
                | ((r[0] as u32) << 16)
                | ((r[1] as u32) << 12)
                | ((r[2] as u32) << 8)
                | ((r[3] as u32) << 4)
                | (r[4] as u32),
        )
    }

    /// The raw comparable value, e.g. for storage or for crossing into JavaScript.
    pub const fn value(self) -> u32 {
        self.0
    }

    /// The hand's category.
    pub fn category(self) -> Category {
        Category::ALL[((self.0 >> 20) - 1) as usize]
    }

    /// The tie-break ranks in order, skipping empty slots (see the table above).
    pub fn tiebreaks(self) -> impl Iterator<Item = Rank> {
        (0..5)
            .map(move |i| ((self.0 >> (16 - 4 * i)) & 0xF) as u8)
            .filter(|&v| v != 0)
            .map(Rank::from_value)
    }
}
