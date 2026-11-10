//! Very hard: thousands of random paragraphs against the oracle.
#![cfg(feature = "very-hard")]

use trials::*;

fn run(seed: u64, cases: usize, mut make: impl FnMut(&mut Rng, usize) -> (Vec<Item>, Vec<f64>, Params)) {
    let mut rng = Rng::new(seed);
    let (mut solved, mut infeasible, mut ambiguous) = (0, 0, 0);
    for case in 0..cases {
        let (items, widths, params) = make(&mut rng, case);
        match check(&items, &widths, &params) {
            Verdict::Solved => solved += 1,
            Verdict::Infeasible => infeasible += 1,
            Verdict::Ambiguous => ambiguous += 1,
        }
    }
    assert!(solved * 2 >= cases, "only {solved} of {cases} solved ({infeasible} infeasible, {ambiguous} ambiguous)");
}

#[test]
fn two_thousand_paragraphs() {
    run(1, 2000, |rng, case| {
        let shape = Shape { words: rng.int(15, 60) as usize, ..Shape::default() };
        let items = paragraph(rng, shape);
        let widths = random_widths(rng, case % 4 == 0, false, 15.0, 40.0);
        let params = Params { tolerance: rng.pick(&[2.0, 3.0, 5.0, 10.0]), ..random_params(rng) };
        (items, widths, params)
    });
}

#[test]
fn looseness_everywhere() {
    run(2, 800, |rng, case| {
        let shape = Shape { words: rng.int(15, 50) as usize, hyphens: 0.5, ..Shape::default() };
        let items = paragraph(rng, shape);
        let widths = random_widths(rng, case % 3 == 0, false, 15.0, 35.0);
        let mut params = random_params(rng);
        params.tolerance = rng.pick(&[3.0, 5.0, 10.0]);
        params.looseness = rng.int(-3, 3) as i32;
        (items, widths, params)
    });
}

#[test]
fn long_paragraphs() {
    run(3, 25, |rng, case| {
        let shape = Shape { words: 400, hyphens: 0.4, penalties: 0.05, forced: 0.005, ..Shape::default() };
        let items = paragraph(rng, shape);
        let widths = random_widths(rng, case % 2 == 0, false, 30.0, 70.0);
        let params = Params { tolerance: rng.pick(&[2.0, 3.0, 5.0]), ..random_params(rng) };
        (items, widths, params)
    });
}

#[test]
fn narrow_columns() {
    let mut rng = Rng::new(4);
    let (mut infeasible, mut solved) = (0, 0);
    for case in 0..400 {
        let shape = Shape { words: rng.int(10, 30) as usize, hyphens: 0.7, ..Shape::default() };
        let items = paragraph(&mut rng, shape);
        let widths = random_widths(&mut rng, case % 2 == 0, false, 14.0, 20.0);
        let params = Params { tolerance: rng.pick(&[2.0, 5.0, 10.0]), ..random_params(&mut rng) };
        match check(&items, &widths, &params) {
            Verdict::Infeasible => infeasible += 1,
            Verdict::Solved => solved += 1,
            Verdict::Ambiguous => {}
        }
    }
    // Plenty of these can't be set at all; you have to say so.
    assert!(infeasible >= 150 && solved >= 30, "{infeasible} infeasible, {solved} solved");
}
