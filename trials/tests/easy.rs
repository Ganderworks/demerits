//! Easy: does it break lines at all?
#![cfg(feature = "easy")]

use trials::*;

fn params() -> Params {
    Params::default()
}

fn positions(breaks: &[Break]) -> Vec<usize> {
    breaks.iter().map(|b| b.position).collect()
}

#[test]
fn a_short_paragraph_is_one_line() {
    let items = text("Great art Thou, O Lord");
    let breaks = break_lines(&items, &[40 * CH], &params()).unwrap();
    assert_eq!(breaks, vec![Break { position: items.len() - 1, ratio: 0.0 }]);
}

#[test]
fn an_exact_fit_needs_no_glue() {
    let items = [bx(10), forced()];
    assert_eq!(break_lines(&items, &[10], &params()), Ok(vec![Break { position: 1, ratio: 0.0 }]));
}

#[test]
fn a_forced_break_is_always_taken() {
    // Both lines fit exactly, and the break between them is forced.
    let items = [bx(3), forced(), bx(3), forced()];
    let breaks = break_lines(&items, &[3], &params()).unwrap();
    assert_eq!(positions(&breaks), vec![1, 3]);
}

#[test]
fn a_line_breaks_at_the_space() {
    // [B3, G, B3, P+inf, fill, P-inf]: "aaa" fills the first line exactly.
    let items = text("aaa bbb");
    assert_eq!(
        break_lines(&items, &[3 * CH], &params()),
        Ok(vec![Break { position: 1, ratio: 0.0 }, Break { position: 5, ratio: 0.0 }])
    );
}

#[test]
fn a_box_too_wide_for_the_line_is_infeasible() {
    assert_eq!(break_lines(&[bx(10), forced()], &[5], &params()), Err(Error::Infeasible));
}

#[test]
fn every_line_is_legal_and_feasible() {
    let items = text(CONFESSIONS);
    for width in [30 * CH, 40 * CH, 50 * CH, 60 * CH] {
        let breaks = break_lines(&items, &[width], &params()).unwrap();
        if let Err(e) = validate(&items, &[width], &params(), &breaks) {
            panic!("at width {width}: {e}");
        }
    }
}

#[test]
fn a_small_paragraph_comes_out_optimal() {
    let items = text("Great art Thou, O Lord, and greatly to be praised");
    for width in [17 * CH, 20 * CH, 26 * CH, 30 * CH] {
        assert_eq!(check_brute(&items, &[width], &params()), Verdict::Solved, "at width {width}");
    }
}
