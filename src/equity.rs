//! Equity: how often each hand wins, by exact enumeration or seeded Monte Carlo.
//!
//! A *run-out* is one way of filling in everything still unknown: the missing
//! board cards, plus the hole cards of any unknown opponent. Exact equity visits
//! every run-out once; Monte Carlo samples them at random. Evaluation uses the
//! allocation-free strength path — no best-cards or descriptions in the loop.

use crate::card::{Card, Rank, Suit};
use crate::error::EvalError;
use crate::evaluator::check_no_duplicates;
use crate::strength::HandStrength;
use crate::variant::Variant;

/// What to calculate equity for.
#[derive(Debug, Clone, PartialEq)]
pub struct EquityRequest {
    pub variant: Variant,
    /// Each player's hole cards. An empty `Vec` is an unknown opponent, dealt a random hand.
    pub players: Vec<Vec<Card>>,
    /// The known board cards: 0, 3, 4 or 5.
    pub board: Vec<Card>,
    /// Cards known to be out of play (e.g. a folded hand that was shown).
    pub dead: Vec<Card>,
    pub method: EquityMethod,
}

/// How to calculate it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquityMethod {
    /// Every possible run-out. Exact, but the count grows quickly with unknown cards.
    Exact,
    /// `trials` random run-outs. The same `seed` always gives the same answer.
    MonteCarlo { trials: u32, seed: u64 },
    /// Exact when there are at most `max_exact` run-outs, otherwise Monte Carlo.
    Auto {
        max_exact: u64,
        trials: u32,
        seed: u64,
    },
}

/// One player's result. All three are fractions of run-outs, from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerEquity {
    /// Won outright.
    pub win: f64,
    /// Shared the pot (any split).
    pub tie: f64,
    /// Share of the pot on average: wins plus the player's share of each split.
    pub equity: f64,
}

/// The result for every player, in request order.
#[derive(Debug, Clone, PartialEq)]
pub struct EquityResult {
    pub players: Vec<PlayerEquity>,
    /// How many run-outs were evaluated.
    pub runouts: u64,
    /// `true` if every run-out was evaluated (exact), `false` for Monte Carlo.
    pub exact: bool,
}

/// The equity contract. Inject it rather than constructing a calculator inline.
pub trait EquityCalculator {
    fn equity(&self, request: &EquityRequest) -> Result<EquityResult, EvalError>;
}

/// The library's equity calculator.
#[derive(Debug, Clone, Copy, Default)]
pub struct Equity;

impl EquityCalculator for Equity {
    fn equity(&self, request: &EquityRequest) -> Result<EquityResult, EvalError> {
        let setup = Setup::new(request)?;
        match request.method {
            EquityMethod::Exact => Ok(setup.exact()),
            EquityMethod::MonteCarlo { trials, seed } => setup.monte_carlo(trials, seed),
            EquityMethod::Auto {
                max_exact,
                trials,
                seed,
            } => {
                if setup.runout_count() <= max_exact {
                    Ok(setup.exact())
                } else {
                    setup.monte_carlo(trials, seed)
                }
            }
        }
    }
}

impl Equity {
    /// The number of run-outs `Exact` would evaluate for `request` (saturating at `u64::MAX`).
    /// Useful to decide up front whether exact equity is affordable.
    pub fn runout_count(&self, request: &EquityRequest) -> Result<u64, EvalError> {
        Ok(Setup::new(request)?.runout_count())
    }
}

/// A validated request, with the deck of cards still available.
struct Setup<'a> {
    variant: Variant,
    request: &'a EquityRequest,
    /// Cards not held by a known player, on the board or dead.
    deck: Vec<Card>,
    /// Indices of unknown opponents.
    unknown: Vec<usize>,
    /// Board cards still to come.
    board_needed: usize,
}

impl<'a> Setup<'a> {
    fn new(request: &'a EquityRequest) -> Result<Setup<'a>, EvalError> {
        let variant = request.variant;
        let hole_count = variant.hole_cards();
        if request.players.is_empty() {
            return Err(EvalError::NoPlayers);
        }
        for hole in &request.players {
            if !hole.is_empty() && hole.len() != hole_count {
                return Err(EvalError::WrongHoleCardCount {
                    variant,
                    expected: hole_count,
                    got: hole.len(),
                });
            }
        }
        if !matches!(request.board.len(), 0 | 3 | 4 | 5) {
            return Err(EvalError::InvalidBoardSize(request.board.len()));
        }
        check_no_duplicates(
            request
                .players
                .iter()
                .flatten()
                .chain(&request.board)
                .chain(&request.dead),
        )?;

        let mut used = 0u64;
        for card in request
            .players
            .iter()
            .flatten()
            .chain(&request.board)
            .chain(&request.dead)
        {
            used |= 1 << card.index();
        }
        let deck: Vec<Card> = full_deck()
            .filter(|c| used & (1 << c.index()) == 0)
            .collect();

        let unknown: Vec<usize> = (0..request.players.len())
            .filter(|&i| request.players[i].is_empty())
            .collect();
        let board_needed = 5 - request.board.len();
        let needed = unknown.len() * hole_count + board_needed;
        if needed > deck.len() {
            return Err(EvalError::NotEnoughCards {
                needed,
                available: deck.len(),
            });
        }
        Ok(Setup {
            variant,
            request,
            deck,
            unknown,
            board_needed,
        })
    }

    /// Unknown hands are dealt first (ordered, one player after another), then the board.
    fn runout_count(&self) -> u64 {
        let h = self.variant.hole_cards() as u64;
        let mut remaining = self.deck.len() as u64;
        let mut count: u128 = 1;
        for _ in &self.unknown {
            count = count.saturating_mul(choose(remaining, h));
            remaining -= h;
        }
        count
            .saturating_mul(choose(remaining, self.board_needed as u64))
            .min(u64::MAX as u128) as u64
    }

    fn exact(&self) -> EquityResult {
        let mut tally = Tally::new(self.request.players.len());
        let mut runout = Runout::new(self);
        self.enumerate(0, 0, &mut runout, &mut tally);
        tally.result(true)
    }

    /// Recursively fills slot `slot` (each unknown player's hand, then the board)
    /// with every combination of not-yet-used deck cards, starting the search at
    /// deck position `from` within a slot so each combination is visited once.
    fn enumerate(&self, slot: usize, used: u64, runout: &mut Runout, tally: &mut Tally) {
        let h = self.variant.hole_cards();
        if slot < self.unknown.len() {
            self.choose_into(
                0,
                h,
                used,
                &mut Vec::with_capacity(h),
                &mut |cards, used| {
                    runout.holes[self.unknown[slot]].copy_from_slice(cards);
                    self.enumerate(slot + 1, used, runout, tally);
                },
            );
        } else {
            let known = self.request.board.len();
            self.choose_into(
                0,
                self.board_needed,
                used,
                &mut Vec::with_capacity(5),
                &mut |cards, _| {
                    runout.board[known..].copy_from_slice(cards);
                    tally.add(runout.strengths(self.variant));
                },
            );
        }
    }

    /// Calls `emit` with every `k`-card combination of deck cards not in `used`,
    /// starting from deck index `from`.
    fn choose_into(
        &self,
        from: usize,
        k: usize,
        used: u64,
        picked: &mut Vec<Card>,
        emit: &mut dyn FnMut(&[Card], u64),
    ) {
        if picked.len() == k {
            emit(picked, used);
            return;
        }
        for i in from..self.deck.len() {
            let bit = 1u64 << self.deck[i].index();
            if used & bit != 0 {
                continue;
            }
            picked.push(self.deck[i]);
            self.choose_into(i + 1, k, used | bit, picked, emit);
            picked.pop();
        }
    }

    fn monte_carlo(&self, trials: u32, seed: u64) -> Result<EquityResult, EvalError> {
        if trials == 0 {
            return Err(EvalError::InvalidTrials);
        }
        let h = self.variant.hole_cards();
        let known = self.request.board.len();
        let mut rng = SplitMix64(seed);
        let mut deck = self.deck.clone();
        let mut runout = Runout::new(self);
        let mut tally = Tally::new(self.request.players.len());
        let needed = self.unknown.len() * h + self.board_needed;

        for _ in 0..trials {
            // Partial Fisher–Yates: the first `needed` cards become a uniform random draw.
            for i in 0..needed {
                let j = i + rng.below(deck.len() - i);
                deck.swap(i, j);
            }
            for (n, &player) in self.unknown.iter().enumerate() {
                runout.holes[player].copy_from_slice(&deck[n * h..(n + 1) * h]);
            }
            let start = self.unknown.len() * h;
            runout.board[known..].copy_from_slice(&deck[start..start + self.board_needed]);
            tally.add(runout.strengths(self.variant));
        }
        Ok(tally.result(false))
    }
}

/// The cards of one run-out: every player's hand and the full five-card board.
struct Runout {
    holes: Vec<Vec<Card>>,
    board: [Card; 5],
    strengths: Vec<HandStrength>,
}

impl Runout {
    fn new(setup: &Setup) -> Runout {
        let filler = setup.deck[0];
        let h = setup.variant.hole_cards();
        let holes = setup
            .request
            .players
            .iter()
            .map(|p| {
                if p.is_empty() {
                    vec![filler; h]
                } else {
                    p.clone()
                }
            })
            .collect();
        let mut board = [filler; 5];
        board[..setup.request.board.len()].copy_from_slice(&setup.request.board);
        Runout {
            holes,
            board,
            strengths: Vec::with_capacity(setup.request.players.len()),
        }
    }

    fn strengths(&mut self, variant: Variant) -> &[HandStrength] {
        let selection = variant.selection();
        self.strengths.clear();
        for hole in &self.holes {
            self.strengths.push(selection.strength(hole, &self.board));
        }
        &self.strengths
    }
}

/// Running totals of wins, ties and pot shares.
struct Tally {
    wins: Vec<u64>,
    ties: Vec<u64>,
    shares: Vec<f64>,
    runouts: u64,
}

impl Tally {
    fn new(players: usize) -> Tally {
        Tally {
            wins: vec![0; players],
            ties: vec![0; players],
            shares: vec![0.0; players],
            runouts: 0,
        }
    }

    fn add(&mut self, strengths: &[HandStrength]) {
        self.runouts += 1;
        let best = *strengths.iter().max().expect("at least one player");
        let winners = strengths.iter().filter(|&&s| s == best).count();
        let share = 1.0 / winners as f64;
        for (i, &s) in strengths.iter().enumerate() {
            if s == best {
                if winners == 1 {
                    self.wins[i] += 1;
                } else {
                    self.ties[i] += 1;
                }
                self.shares[i] += share;
            }
        }
    }

    fn result(self, exact: bool) -> EquityResult {
        let n = self.runouts as f64;
        let players = (0..self.wins.len())
            .map(|i| PlayerEquity {
                win: self.wins[i] as f64 / n,
                tie: self.ties[i] as f64 / n,
                equity: self.shares[i] / n,
            })
            .collect();
        EquityResult {
            players,
            runouts: self.runouts,
            exact,
        }
    }
}

/// Small, fast, seedable generator (SplitMix64).
/// Not for dealing real games — only for simulation.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` (Lemire's multiply-shift; the bias for n ≤ 52 is below 1e-17).
    fn below(&mut self, n: usize) -> usize {
        ((self.next() as u128 * n as u128) >> 64) as usize
    }
}

fn full_deck() -> impl Iterator<Item = Card> {
    Rank::ALL
        .into_iter()
        .flat_map(|r| Suit::ALL.map(|s| Card::new(r, s)))
}

/// n choose k, as u128 (exact for every value this library needs).
fn choose(n: u64, k: u64) -> u128 {
    if k > n {
        return 0;
    }
    (0..k).fold(1u128, |acc, i| acc * (n - i) as u128 / (i + 1) as u128)
}
