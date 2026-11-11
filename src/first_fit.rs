//! First fit: [`greedy`] and [`ragged`]. The rules are in the crate docs,
//! under "First fit".

use crate::{Break, Error, INFINITY, Item, Stretch};

/// Break a paragraph into justified lines, first fit: each line takes as much
/// as its glue can shrink to hold.
///
/// Returns one [`Break`] per line, in order, with each line's adjustment
/// ratio; the last is the paragraph's final forced break. A line that's too
/// long even at its first break gets a ratio below `-1`. An empty paragraph
/// has no lines.
///
/// # Errors
///
/// [`Error::Unterminated`] if the paragraph doesn't end with a forced break.
/// Never [`Error::Infeasible`].
///
/// # Panics
///
/// If `line_widths` is empty and `items` isn't.
pub fn greedy(items: &[Item], line_widths: &[i64]) -> Result<Vec<Break>, Error> {
    first_fit(items, line_widths, Mode::Justified)
}

/// Break a paragraph into unjustified lines, first fit: each line takes as
/// much as fits at its natural width. For ragged-right text, and for wrapping
/// text in an editor.
///
/// Returns one [`Break`] per line, in order; the last is the paragraph's
/// final forced break. Nothing stretches or shrinks, so every ratio is `0.0`.
/// A line that's too long even at its first break sticks out. An empty
/// paragraph has no lines.
///
/// # Errors
///
/// [`Error::Unterminated`] if the paragraph doesn't end with a forced break.
/// Never [`Error::Infeasible`].
///
/// # Panics
///
/// If `line_widths` is empty and `items` isn't.
pub fn ragged(items: &[Item], line_widths: &[i64]) -> Result<Vec<Break>, Error> {
    first_fit(items, line_widths, Mode::Ragged)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Justified,
    Ragged,
}

/// What's on a line so far, from its start up to (not including) the item
/// being looked at.
#[derive(Debug, Clone, Copy, Default)]
struct Line {
    natural: i64,
    /// The finite stretch only.
    stretch: i64,
    /// Whether any glue stretches without limit.
    infinite: bool,
    shrink: i64,
}

impl Line {
    fn add(&mut self, item: &Item) {
        match *item {
            Item::Box { width } => self.natural += width,
            Item::Glue { width, stretch, shrink } => {
                self.natural += width;
                match stretch {
                    Stretch::Finite(y) => self.stretch += y,
                    Stretch::Infinite => self.infinite = true,
                }
                self.shrink += shrink;
            }
            // A penalty takes up room only where the line breaks.
            Item::Penalty { .. } => {}
        }
    }

    /// The adjustment ratio if the line, `natural` wide in all, is set to
    /// `width`.
    fn ratio(&self, natural: i64, width: i64) -> f64 {
        if natural < width {
            if self.infinite {
                0.0
            } else if self.stretch > 0 {
                (width - natural) as f64 / self.stretch as f64
            } else {
                f64::INFINITY
            }
        } else if natural > width {
            if self.shrink > 0 { (width - natural) as f64 / self.shrink as f64 } else { f64::NEG_INFINITY }
        } else {
            0.0
        }
    }
}

fn is_forced(item: &Item) -> bool {
    matches!(*item, Item::Penalty { cost, .. } if cost <= -INFINITY)
}

fn is_break(items: &[Item], i: usize) -> bool {
    match items[i] {
        Item::Penalty { cost, .. } => cost < INFINITY,
        Item::Glue { .. } => i > 0 && matches!(items[i - 1], Item::Box { .. }),
        Item::Box { .. } => false,
    }
}

fn first_fit(items: &[Item], line_widths: &[i64], mode: Mode) -> Result<Vec<Break>, Error> {
    if items.is_empty() {
        return Ok(Vec::new());
    }
    assert!(!line_widths.is_empty(), "no line widths: every line needs one");
    if !items.last().is_some_and(is_forced) {
        return Err(Error::Unterminated);
    }
    let last = items.len() - 1;
    let mut out = Vec::new();
    let mut start = 0;
    loop {
        let width = line_widths[out.len().min(line_widths.len() - 1)];
        let mut line = Line::default();
        let mut chosen: Option<Break> = None;
        // The last item is a forced break, so this always stops by then.
        for (i, item) in items.iter().enumerate().skip(start) {
            if is_break(items, i) {
                let natural = match *item {
                    Item::Penalty { width, .. } => line.natural + width,
                    _ => line.natural,
                };
                let (fits, ratio) = match mode {
                    Mode::Justified => {
                        let r = line.ratio(natural, width);
                        (r >= -1.0, r)
                    }
                    Mode::Ragged => (natural <= width, 0.0),
                };
                if !fits && chosen.is_some() {
                    break;
                }
                chosen = Some(Break { position: i, ratio });
                if !fits || is_forced(item) {
                    break;
                }
            }
            line.add(item);
        }
        let b = chosen.expect("every line has a break: the paragraph ends with a forced one");
        out.push(b);
        if b.position == last {
            return Ok(out);
        }
        // Discard the glue and penalties after the break, up to the next box
        // or forced break.
        start = b.position + 1;
        while !matches!(items[start], Item::Box { .. }) && !is_forced(&items[start]) {
            start += 1;
        }
    }
}
