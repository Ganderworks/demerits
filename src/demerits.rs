use std::range;

use crate::{Break, Error, Item, Params, Stretch};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fitness {
    WayLoose = 3,
    Loose = 2,
    Normal = 1,
    Tight = 0,
}

struct Node {
    position: usize,
    line: usize,
    fitness: Fitness,
    t_width: i64,
    t_stretch: i64, // finite stretch only
    t_shrink: i64,
    t_demerits: f64,
    prev: Option<usize>,
    next: Option<usize>,
}

struct Lists {
    nodes: Vec<Node>,      // all nodes, active and passive
    active: Option<usize>, // head of the active list
}

pub const INF: i64 = 10_000;
pub const NEG_INF: i64 = -10_000;

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
    let mut nodes = Lists::new();
    let mut A: &Node = &Node {
        position: 0,
        line: 0,
        fitness: Fitness::Normal,
        t_width: 0,
        t_stretch: 0,
        t_shrink: 0,
        t_demerits: 0.0,
        next: Option::None,
        prev: Option::None,
    };
    let mut P: &Node;
    let mut sum_width: i64 = 0;
    let mut sum_stretch: i64 = 0; // finite stretch only
    let mut sum_infinite: i64 = 0; // count of infinite-stretch glues
    let mut sum_shrink: i64 = 0;
    // NOTE: b from paper is `i` here.
    for (i, item) in items.iter().enumerate() {
        match *item {
            Item::Box { width } => sum_width += width,
            Item::Glue {
                width,
                stretch,
                shrink,
            } => {
                // TODO: unuglify
                if i > 0 && matches!(items[i - 1], Item::Box { .. }) {
                    main_loop(&nodes)
                }
                sum_width += width;
                // NOTE: we only need to store the latest infinite stretch.
                // TODO: Implement when you know what is going on.
                match stretch {
                    Stretch::Finite(s) => sum_stretch += s,
                    Stretch::Infinite => sum_infinite += 1,
                }
                sum_shrink += shrink;
            }
            Item::Penalty {
                width,
                cost,
                flagged,
            } => {
                // Legal break unless the cost is +INF or more; forced ones too.
                if cost < INF {
                    main_loop(&nodes);
                }
            }
        }
    }
    todo!("return something")
}

fn main_loop(nodes: &Lists) {
    let mut next_a: &Node;

    loop {
        let mut demerits_wayloose = INF;
        let mut demerits_loose = INF;
        let mut demerits_normal = INF;
        let mut demerits_tight = INF;
        let mut demerits_min = INF;
        loop {
            next_a = &a.next();
            // TODO: compute the adjustment ratio r from a to b
            let r = 1.0; // NOTE: placeholder
        }
    }
}

// TODO: deactivate, demerits, fitness, insert_nodes, complain

fn deactivate(node: &Node) {
    todo!();
}

/// The adjustment ratio of the line from active node `a` to a break at `b`.
fn adjustment_ratio(
    items: &[Item],
    line_widths: &[i64],
    sum_width: i64,
    sum_stretch: i64,
    sum_shrink: i64,
    infinite_stretch: bool, // any Stretch::Infinite glue in this line
    a: &Node,
    b: usize,
) -> f64 {
    let mut line_width = sum_width - a.t_width;
    if let Item::Penalty { width, .. } = items[b] {
        line_width += width;
    }
    // Lines past the end of line_widths use its last entry.
    let desired_line_width = line_widths[a.line.min(line_widths.len() - 1)];

    if line_width < desired_line_width {
        if infinite_stretch {
            return 0.0;
        }
        let line_stretch = sum_stretch - a.t_stretch;
        if line_stretch > 0 {
            (desired_line_width - line_width) as f64 / line_stretch as f64
        } else {
            f64::INFINITY
        }
    } else if line_width > desired_line_width {
        let line_shrink = sum_shrink - a.t_shrink;
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

fn main_loop_iguess() {}
