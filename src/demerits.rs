use std::collections::BTreeMap;

use crate::{Break, Error, Item, Params, Stretch};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fitness {
    WayLoose = 3,
    Loose = 2,
    Normal = 1,
    Tight = 0,
}

/// A feasible break: the end of the best way found to set the paragraph up to
/// `position` in `line` lines, the last of them in class `fitness`.
struct Node {
    position: usize,
    start: usize, // where the line after this break starts
    line: usize,  // lines so far
    fitness: Fitness,
    ratio: f64, // of the line that ends here
    totals: ItemStat,
    t_demerits: f64,
    prev: Option<usize>, // the break before this one
}

/// Running totals over the items before some index. `inf` counts glues with
/// infinite stretch, so a line has some iff the count goes up across it.
#[derive(Clone, Copy)]
struct ItemStat {
    width: i64,
    stretch: i64,
    inf: i64,
    shrink: i64,
}

struct Lists {
    nodes: Vec<Node>,   // all nodes, active and passive
    active: Vec<usize>, // indices into `nodes`, oldest first
}

#[derive(Clone, Copy)]
struct Demerit {
    score: f64,          // fewest total demerits found so far for this class
    node: Option<usize>, // the active node that gave them
    ratio: f64,          // of the line from that node
}

/// The best way to reach one break, per fitness class.
struct Demerits {
    way_loose: Demerit,
    loose: Demerit,
    normal: Demerit,
    tight: Demerit,
}

impl Demerits {
    fn new() -> Demerits {
        Demerits {
            way_loose: Demerit::new(),
            loose: Demerit::new(),
            normal: Demerit::new(),
            tight: Demerit::new(),
        }
    }

    fn class_mut(&mut self, fitness: Fitness) -> &mut Demerit {
        match fitness {
            Fitness::WayLoose => &mut self.way_loose,
            Fitness::Loose => &mut self.loose,
            Fitness::Normal => &mut self.normal,
            Fitness::Tight => &mut self.tight,
        }
    }

    /// Every class, in a fixed order.
    fn classes(&self) -> [(Fitness, Demerit); 4] {
        [
            (Fitness::Tight, self.tight),
            (Fitness::Normal, self.normal),
            (Fitness::Loose, self.loose),
            (Fitness::WayLoose, self.way_loose),
        ]
    }
}

impl Demerit {
    fn new() -> Demerit {
        Demerit {
            score: f64::INFINITY,
            node: None,
            ratio: 0.0,
        }
    }
}

pub const INF: i64 = 10_000;
pub const NEG_INF: i64 = -10_000;

fn is_forced(item: &Item) -> bool {
    matches!(*item, Item::Penalty { cost, .. } if cost <= NEG_INF)
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
    let Some(last) = items.last() else {
        return Ok(Vec::new());
    };
    if !is_forced(last) {
        return Err(Error::Unterminated);
    }
    assert!(!line_widths.is_empty(), "line_widths is empty");

    let mut lists = Lists {
        nodes: vec![Node {
            position: 0,
            start: 0, // nothing is discarded at the start of a paragraph
            line: 0,
            fitness: Fitness::Normal,
            ratio: 0.0,
            totals: ItemStat {
                width: 0,
                stretch: 0,
                inf: 0,
                shrink: 0,
            },
            t_demerits: 0.0,
            prev: None,
        }],
        active: vec![0],
    };
    // Totals over items[..i].
    let mut sig = ItemStat {
        width: 0,
        stretch: 0,
        inf: 0,
        shrink: 0,
    };
    // NOTE: b from paper is `i` here.
    for (i, item) in items.iter().enumerate() {
        match *item {
            Item::Box { width } => sig.width += width,
            Item::Glue {
                width,
                stretch,
                shrink,
            } => {
                if i > 0 && matches!(items[i - 1], Item::Box { .. }) {
                    main_loop(&mut lists, items, line_widths, params, i, &sig);
                }
                sig.width += width;
                match stretch {
                    Stretch::Finite(s) => sig.stretch += s,
                    Stretch::Infinite => sig.inf += 1,
                }
                sig.shrink += shrink;
            }
            Item::Penalty { cost, .. } => {
                // Legal break unless the cost is +INF or more; forced ones too.
                if cost < INF {
                    main_loop(&mut lists, items, line_widths, params, i, &sig);
                }
            }
        }
        if lists.active.is_empty() {
            return Err(Error::Infeasible);
        }
    }

    // The last item is forced, so every active node is a break there.
    let end = choose(&lists, params.looseness).ok_or(Error::Infeasible)?;
    let mut breaks = Vec::new();
    let mut n = Some(end);
    while let Some(k) = n {
        let node = &lists.nodes[k];
        if node.prev.is_none() {
            break; // the start of the paragraph isn't a break
        }
        breaks.push(Break {
            position: node.position,
            ratio: node.ratio,
        });
        n = node.prev;
    }
    breaks.reverse();
    Ok(breaks)
}

/// Pick the final node: the fewest demerits, or with `looseness` the line
/// count closest to it that has a solution.
fn choose(lists: &Lists, looseness: i32) -> Option<usize> {
    // On ties, the first: always the same one for the same input.
    let fewest = |line: Option<usize>| {
        let mut best: Option<usize> = None;
        for &k in &lists.active {
            let node = &lists.nodes[k];
            if line.is_some_and(|l| node.line != l) {
                continue;
            }
            if best.is_none_or(|b| node.t_demerits < lists.nodes[b].t_demerits) {
                best = Some(k);
            }
        }
        best
    };
    let optimum = fewest(None)?;
    let n = lists.nodes[optimum].line as i64;
    let step = -(looseness.signum() as i64);
    let mut d = looseness as i64;
    while d != 0 {
        if n + d >= 1 {
            if let Some(k) = fewest(Some((n + d) as usize)) {
                return Some(k);
            }
        }
        d += step;
    }
    Some(optimum)
}

/// Try every active node against a legal break at `b`: record the best way
/// to reach `b` per line count and fitness class, retire nodes that can't
/// reach past `b`, and add the new nodes.
fn main_loop(
    lists: &mut Lists,
    items: &[Item],
    line_widths: &[i64],
    params: &Params,
    b: usize,
    sig: &ItemStat,
) {
    let forced = is_forced(&items[b]);
    let last_width = line_widths.len() - 1;
    // Keyed by the new node's line count. Past the last line width every line
    // is alike, so without looseness those counts can share a key.
    let mut best: BTreeMap<usize, Demerits> = BTreeMap::new();
    let mut still_active = Vec::with_capacity(lists.active.len());

    for &a in &lists.active {
        let node = &lists.nodes[a];
        if b < node.start {
            // b is inside the run discarded after node; no line ends there.
            still_active.push(a);
            continue;
        }
        let r = adjustment_ratio(items, line_widths, sig, node, b);
        if r.is_finite() && r >= -1.0 && r <= params.tolerance {
            let total = node.t_demerits + demerits(params, items, node, b, r);
            let line = node.line + 1;
            let key = if params.looseness == 0 {
                line.min(last_width)
            } else {
                line
            };
            let slot = best
                .entry(key)
                .or_insert_with(Demerits::new)
                .class_mut(fitness(r));
            if total < slot.score {
                *slot = Demerit {
                    score: total,
                    node: Some(a),
                    ratio: r,
                };
            }
        }
        // A line that's already too long only gets longer.
        if !(forced || r < -1.0) {
            still_active.push(a);
        }
    }
    lists.active = still_active;

    if best.is_empty() {
        return;
    }
    let (totals, start) = item_totals(items, b, sig);
    for demerits in best.values() {
        for (fitness, d) in demerits.classes() {
            let Some(a) = d.node else { continue };
            lists.nodes.push(Node {
                position: b,
                start,
                line: lists.nodes[a].line + 1,
                fitness,
                ratio: d.ratio,
                totals,
                t_demerits: d.score,
                prev: Some(a),
            });
            lists.active.push(lists.nodes.len() - 1);
        }
    }
}

/// The adjustment ratio of the line from active node `a` to a break at `b`,
/// with `sig` the totals over items[..b].
fn adjustment_ratio(
    items: &[Item],
    line_widths: &[i64],
    sig: &ItemStat,
    a: &Node,
    b: usize,
) -> f64 {
    let mut line_width = sig.width - a.totals.width;
    if let Item::Penalty { width, .. } = items[b] {
        line_width += width;
    }
    // Lines past the end of line_widths use its last entry.
    let desired_line_width = line_widths[a.line.min(line_widths.len() - 1)];

    if line_width < desired_line_width {
        if sig.inf > a.totals.inf {
            return 0.0; // infinite glue takes up the slack
        }
        let line_stretch = sig.stretch - a.totals.stretch;
        if line_stretch > 0 {
            (desired_line_width - line_width) as f64 / line_stretch as f64
        } else {
            f64::INFINITY
        }
    } else if line_width > desired_line_width {
        let line_shrink = sig.shrink - a.totals.shrink;
        if line_shrink > 0 {
            (desired_line_width - line_width) as f64 / line_shrink as f64
        } else {
            f64::NEG_INFINITY
        }
    } else {
        0.0
    }
}

fn fitness(r: f64) -> Fitness {
    // Only called on feasible lines, so r >= -1.
    if r < -0.5 {
        Fitness::Tight
    } else if r <= 0.5 {
        Fitness::Normal
    } else if r <= 1.0 {
        Fitness::Loose
    } else {
        Fitness::WayLoose
    }
}

/// The totals just after a break at `b`: `sig` plus the glue that the break
/// discards, up to the next box or forced penalty, which is where the next
/// line starts. The paper's tw, ty, tz.
fn item_totals(items: &[Item], b: usize, sig: &ItemStat) -> (ItemStat, usize) {
    let mut t = *sig;
    for (i, item) in items.iter().enumerate().skip(b) {
        match *item {
            Item::Box { .. } => return (t, i),
            Item::Glue {
                width,
                stretch,
                shrink,
            } => {
                t.width += width;
                match stretch {
                    Stretch::Finite(s) => t.stretch += s,
                    Stretch::Infinite => t.inf += 1,
                }
                t.shrink += shrink;
            }
            Item::Penalty { cost, .. } => {
                if cost <= NEG_INF && i > b {
                    return (t, i);
                }
            }
        }
    }
    (t, items.len())
}

fn is_flagged(item: &Item) -> bool {
    matches!(item, Item::Penalty { flagged: true, .. })
}

/// The demerits of the line from active node `a` to a break at `b` with ratio `r`.
fn demerits(params: &Params, items: &[Item], a: &Node, b: usize, r: f64) -> f64 {
    // Plain multiplication rather than powi, so results are bit-for-bit
    // the same everywhere.
    let r_abs = r.abs();
    let badness = 100.0 * r_abs * r_abs * r_abs;
    let base = (params.line_penalty + badness) * (params.line_penalty + badness);
    let mut demerits = match items[b] {
        Item::Penalty { cost, .. } if cost <= NEG_INF => base, // forced
        Item::Penalty { cost, .. } if cost >= 0 => base + (cost as f64) * (cost as f64),
        Item::Penalty { cost, .. } => base - (cost as f64) * (cost as f64),
        _ => base, // glue: p = 0
    };
    // The start of the paragraph (line 0) is never flagged.
    if a.line > 0 && is_flagged(&items[a.position]) && is_flagged(&items[b]) {
        demerits += params.flagged_demerits;
    }
    if (fitness(r) as i64 - a.fitness as i64).abs() > 1 {
        demerits += params.fitness_demerits;
    }
    demerits
}
