//! Deterministic Knuth–Plass line breaking.
//!
//! A paragraph is a list of [`Item`]s: boxes, glue and penalties, as in
//! Knuth and Plass, *Breaking Paragraphs into Lines* (1981). [`break_lines`]
//! picks the breaks that give the whole paragraph the fewest total demerits.
//!
//! Two first-fit breakers come with it, for when the best isn't needed:
//! [`greedy`] fills each justified line as full as it will shrink, and
//! [`ragged`] fills each unjustified line as full as it will go at its
//! natural width. Neither ever fails on a paragraph that ends properly.
//!
//! # The contract
//!
//! The tests in `trials/` hold the implementation to every rule below.
//!
//! ## Numbers
//!
//! Widths, stretch, shrink, line widths and penalty costs are integers
//! (`i64`) in whatever unit you like: font units, TeX's scaled points,
//! thousandths of an em. Sums of them are exact, so `L == W` means what it
//! says. The caller promises that no sum of widths, stretches or shrinks
//! over part of the paragraph, and no line width minus such a sum, overflows
//! `i64`.
//!
//! Everything computed from them (the adjustment ratio, badness, demerits)
//! is `f64`, as are the [`Params`].
//!
//! ## Where a line may break
//!
//! - At a **penalty** whose `cost` is below [`INFINITY`]. A penalty whose cost
//!   is `-INFINITY` or less is **forced**: every solution breaks there.
//! - At **glue** that comes straight after a box. Glue at index 0, glue after
//!   glue and glue after a penalty are not breaks.
//! - Never at a box.
//!
//! The paragraph must end with a forced break. A break is identified by the
//! index of the item it happens at.
//!
//! ## What a line contains
//!
//! The first line starts at index 0; nothing is discarded at the start of a
//! paragraph. After a break at `b`, the next line starts at the first index
//! after `b` that is a box or a forced penalty. The glue and penalties in
//! between are discarded.
//!
//! A line that starts at `s` and breaks at `b` must have `b >= s`. Lines that
//! would end inside a discarded run are not allowed.
//!
//! The line's **natural width** `L` is the sum of the widths of the boxes and
//! glue in `s..b`, plus the width of the item at `b` if it is a penalty.
//! Penalties that aren't broken at take up no width. The line's stretch `Y`
//! and shrink `Z` are the sums over the glue in `s..b`.
//!
//! Line `k`, counting from 0, has width `line_widths[k]`. Lines past the end
//! of `line_widths` use its last entry.
//!
//! ## The adjustment ratio
//!
//! With `W` the line's width:
//!
//! - `L == W`: `r = 0`.
//! - `L < W`: if any glue in the line has [`Stretch::Infinite`], `r = 0`
//!   (the infinite glue takes up the slack). Otherwise `r = (W - L) / Y` if
//!   `Y > 0`, else `+∞`. `Y` is the finite stretch only.
//! - `L > W`: `r = (W - L) / Z` if `Z > 0`, else `-∞`. Infinite stretch
//!   doesn't help a line that's too long.
//!
//! The division is exactly this: `W - L` and `Y` (or `Z`) computed in
//! `i64`, each converted to `f64`, then one `f64` division. Ratios are
//! checked bit for bit.
//!
//! A line is **feasible** if `r` is finite and `-1 <= r <= tolerance`. An
//! infinite ratio is never feasible, whatever the tolerance.
//!
//! ## Demerits
//!
//! Badness is `β = 100 |r|³`. With `l` the line penalty and `p` the cost of the
//! penalty broken at (`0` for glue) as an `f64`, a line's demerits are:
//!
//! - `(l + β)² + p²` if `p >= 0`,
//! - `(l + β)² - p²` if `-INFINITY < p < 0`,
//! - `(l + β)²` if the break is forced,
//!
//! plus `flagged_demerits` if this break and the previous one are both flagged
//! penalties (the start of the paragraph is not flagged), plus
//! `fitness_demerits` if this line's fitness class and the previous line's
//! differ by more than one.
//!
//! Fitness classes: **tight** (0) if `r < -0.5`, **decent** (1) if
//! `-0.5 <= r <= 0.5`, **loose** (2) if `0.5 < r <= 1`, **very loose** (3) if
//! `r > 1`. The line before the first counts as decent.
//!
//! ## The answer
//!
//! With `looseness == 0`: a solution with the fewest total demerits, where
//! every line is feasible. If several tie, any of them, but always the same
//! one for the same input.
//!
//! With `looseness != 0`: let `n` be the line count of the optimal solution.
//! Among the line counts `n + d` that have a feasible solution, with `d`
//! between `0` and `looseness` inclusive, take the one with `d` closest to
//! `looseness`, then the solution with the fewest demerits at that count.
//!
//! Optimality is guaranteed when, between any two consecutive legal breaks,
//! a line's natural width minus its shrink never goes down. (Boxes of positive
//! width, glue that can't shrink below zero, and hyphens no wider than the box
//! after them are all fine; negative kerns inside a word are fine as long as
//! the word's net width stays positive.) Without that condition nothing is
//! promised beyond validity: an answer that comes back must follow every rule
//! here, but the search may miss a better one, or miss every one and say
//! [`Error::Infeasible`].
//!
//! ## Determinism
//!
//! The same items, line widths and parameters give the same breaks and the
//! same ratios, bit for bit, every time. Multiplying every width, stretch,
//! shrink and line width by the same positive integer changes nothing at all,
//! as long as nothing overflows and every sum stays within `2^53` (past that,
//! `f64` can't hold every integer; powers of two are still exact).
//!
//! # First fit: [`greedy`] and [`ragged`]
//!
//! The two first-fit breakers follow the rules above for numbers, where a
//! line may break, what a line contains, which width it gets, determinism and
//! scaling. Demerits, tolerance and looseness don't apply; they take no
//! [`Params`].
//!
//! ## Which break ends a line
//!
//! A line **fits** if it isn't too long:
//!
//! - for [`greedy`], if its adjustment ratio, computed exactly as above, is at
//!   least `-1`: its glue can shrink enough. However loose a line is, even
//!   `+∞`, it fits.
//! - for [`ragged`], if its natural width `L` is at most `W`. Nothing
//!   shrinks.
//!
//! Each line is filled in turn. Starting from the line's start, take the
//! legal breaks it could end at, in order:
//!
//! - Stop at the first break where the line doesn't fit, and end the line at
//!   the break before it.
//! - Stop at a forced break where the line fits, and end the line there. (A
//!   forced break where it doesn't fit is a break where it doesn't fit: the
//!   line ends at the break before it, and the forced break ends the next
//!   line, or a later one.)
//! - If the line doesn't fit even at its first break (a word wider than the
//!   column, say), the line ends there anyway, too long.
//!
//! A line never looks past the first break where it's too long, even if
//! something further on (a negative kern) would bring it back within the
//! width. Finite penalty costs and `flagged` make no difference.
//!
//! ## The answer
//!
//! One [`Break`] per line. [`greedy`] reports each line's adjustment ratio
//! as above, whatever it is: below `-1` (or `-∞`) for a line that's too long,
//! `+∞` for a short line with nothing to stretch. [`ragged`] sets every line
//! at its natural width, so every ratio it reports is `0.0`.
//!
//! An empty paragraph has no lines. A paragraph that doesn't end with a
//! forced break is [`Error::Unterminated`]. Nothing else is an error: neither
//! breaker ever returns [`Error::Infeasible`].
//!
//! Both take time in proportion to the number of items, however long the
//! lines, and don't recurse.

#![forbid(unsafe_code)]

mod first_fit;

pub use first_fit::{greedy, ragged};

/// A penalty `cost` at or above this never breaks; at or below its negative,
/// always breaks.
pub const INFINITY: i64 = 10_000;

/// One element of a paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Item {
    /// Something with a fixed width: a word, a syllable, a kern.
    Box { width: i64 },
    /// Space that can stretch and shrink.
    Glue { width: i64, stretch: Stretch, shrink: i64 },
    /// A possible break. `width` is only used if the line breaks here
    /// (a hyphen, say). `flagged` marks hyphen-like breaks.
    Penalty { width: i64, cost: i64, flagged: bool },
}

/// How far a piece of glue can stretch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stretch {
    /// Up to this much at a ratio of 1.
    Finite(i64),
    /// Without limit, like TeX's `\hfil`. A line with any of it never has a
    /// positive ratio.
    Infinite,
}

/// How to judge a paragraph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    /// The largest adjustment ratio a line may have.
    pub tolerance: f64,
    /// Added to every line's badness before squaring: the paper's `l`.
    pub line_penalty: f64,
    /// Charged when two lines in a row end at flagged penalties: the paper's α.
    pub flagged_demerits: f64,
    /// Charged when neighbouring lines' fitness classes differ by more than
    /// one: the paper's γ.
    pub fitness_demerits: f64,
    /// `0` for the optimum; `+1` asks for one more line, `-1` for one fewer.
    pub looseness: i32,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            tolerance: 2.0,
            line_penalty: 10.0,
            flagged_demerits: 3000.0,
            fitness_demerits: 3000.0,
            looseness: 0,
        }
    }
}

/// Where a line ends, and how much its glue is stretched (`ratio > 0`) or
/// shrunk (`ratio < 0`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Break {
    /// The index of the item the line breaks at.
    pub position: usize,
    /// The line's adjustment ratio.
    pub ratio: f64,
}

/// Why there's no answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// No set of breaks makes every line feasible.
    Infeasible,
    /// The paragraph doesn't end with a forced break.
    Unterminated,
}

/// Break a paragraph into lines.
///
/// Returns one [`Break`] per line, in order; the last is the paragraph's
/// final forced break. An empty paragraph has no lines.
///
/// # Panics
///
/// If `line_widths` is empty and `items` isn't.
pub fn break_lines(
    items: &[Item],
    line_widths: &[i64],
    params: &Params,
) -> Result<Vec<Break>, Error> {
    let _ = (items, line_widths, params);
    todo!("this one's yours")
}
