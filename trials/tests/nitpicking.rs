//! Nitpicking: every rule in the contract, one at a time, at its edges.
#![cfg(feature = "nitpicking")]

use trials::*;

fn p() -> Params {
    Params::default()
}

fn positions(r: Result<Vec<Break>, Error>) -> Result<Vec<usize>, Error> {
    r.map(|bs| bs.iter().map(|b| b.position).collect())
}

// --- where a line may break

#[test]
fn glue_at_the_start_is_not_a_break_and_counts_toward_the_first_line() {
    let items = [glue(2.0, 0.0, 0.0), bx(3.0), forced()];
    assert_eq!(break_lines(&items, &[5.0], &p()), Ok(vec![Break { position: 2, ratio: 0.0 }]));
    assert_eq!(break_lines(&items, &[3.0], &p()), Err(Error::Infeasible));
}

#[test]
fn glue_after_glue_is_not_a_break() {
    // Breaking at index 2 would fit perfectly, if it were allowed.
    let items = [bx(3.0), glue(1.0, 0.0, 0.0), glue(0.0, 0.0, 0.0), bx(3.0), forbidden(), fill(), forced()];
    assert_eq!(break_lines(&items, &[4.0], &p()), Err(Error::Infeasible));
}

#[test]
fn glue_after_a_penalty_is_not_a_break() {
    let items = [bx(3.0), penalty(0.0, 0.0, false), glue(1.0, 0.0, 0.0), bx(3.0), forbidden(), fill(), forced()];
    assert_eq!(break_lines(&items, &[4.0], &p()), Err(Error::Infeasible));
}

#[test]
fn a_cost_of_exactly_infinity_never_breaks() {
    let at = |cost| [bx(4.0), penalty(0.0, cost, false), bx(4.0), forced()];
    assert_eq!(break_lines(&at(INFINITY), &[4.0], &p()), Err(Error::Infeasible));
    assert_eq!(break_lines(&at(1e9), &[4.0], &p()), Err(Error::Infeasible));
    assert_eq!(positions(break_lines(&at(9999.0), &[4.0], &p())), Ok(vec![1, 3]));
}

#[test]
fn a_cost_of_minus_infinity_or_less_always_breaks() {
    // Not breaking would fit exactly; breaking leaves a lone box with no
    // stretch, which is infeasible. A forced break wins anyway.
    for cost in [-INFINITY, -20_000.0, f64::NEG_INFINITY] {
        let items = [bx(4.0), penalty(0.0, cost, false), bx(4.0), forced()];
        assert_eq!(break_lines(&items, &[8.0], &p()), Err(Error::Infeasible), "cost {cost}");
    }
    // Just above -INFINITY it's only a very attractive option.
    let items = [bx(4.0), penalty(0.0, -9999.0, false), bx(4.0), forced()];
    assert_eq!(positions(break_lines(&items, &[8.0], &p())), Ok(vec![3]));
}

// --- what a line contains

#[test]
fn glue_and_penalties_after_a_break_are_discarded() {
    let items = [bx(4.0), glue(1.0, 0.0, 0.0), penalty(0.0, 0.0, false), glue(1.0, 0.0, 0.0), bx(4.0), forced()];
    assert_eq!(
        break_lines(&items, &[4.0], &p()),
        Ok(vec![Break { position: 1, ratio: 0.0 }, Break { position: 5, ratio: 0.0 }])
    );
}

#[test]
fn a_penalty_takes_up_width_only_where_the_line_breaks() {
    // Broken at: the hyphen fills the first line exactly.
    let items = [bx(3.0), penalty(1.0, 0.0, true), bx(3.0), forbidden(), fill(), forced()];
    assert_eq!(
        break_lines(&items, &[4.0], &p()),
        Ok(vec![Break { position: 1, ratio: 0.0 }, Break { position: 5, ratio: 0.0 }])
    );
    // Not broken at: a penalty in the middle of a line takes no room.
    let items = [bx(3.0), penalty(2.0, 0.0, true), bx(3.0), forbidden(), fill(), forced()];
    assert_eq!(break_lines(&items, &[6.0], &p()), Ok(vec![Break { position: 5, ratio: 0.0 }]));
}

#[test]
fn no_line_ends_inside_a_discarded_run() {
    // The -100 at index 2 would be a tempting (empty) line after the break at
    // the glue, but it sits in the run discarded after that break.
    let items = [bx(4.0), glue(1.0, 0.0, 0.0), penalty(0.0, -100.0, false), bx(4.0), forbidden(), fill(), forced()];
    assert_eq!(positions(break_lines(&items, &[4.0], &p())), Ok(vec![1, 6]));
}

#[test]
fn not_even_when_an_empty_line_would_fit() {
    // The second line is 0 wide, so an empty line there fits perfectly, and
    // the -100 makes it a bargain. It's still not allowed: the -100 sits in
    // the run discarded after the break at 1. The answer puts the zero-width
    // box on the second line instead.
    //   0 B4, 1 G, 2 P(-100), 3 G, 4 B0, 5 G0, 6 B4, 7..9 finish
    for hyphen in [0.0, 1.0] {
        let items = [
            bx(4.0),
            glue(1.0, 0.0, 0.0),
            penalty(hyphen, -100.0, false),
            glue(1.0, 0.0, 0.0),
            bx(0.0),
            glue(0.0, 0.0, 0.0),
            bx(4.0),
            forbidden(),
            fill(),
            forced(),
        ];
        assert_eq!(positions(break_lines(&items, &[4.0, 0.0, 4.0], &p())), Ok(vec![1, 5, 9]), "penalty width {hyphen}");
    }
}

#[test]
fn repeating_the_last_width_changes_nothing() {
    // Ties may break differently, so compare what the answers cost.
    let items = text(&hyphenate(CONFESSIONS));
    let cost = |widths: &[f64]| {
        break_lines(&items, widths, &p()).map(|bs| {
            let ev = validate(&items, widths, &p(), &bs).unwrap();
            (bs.len(), ev.demerits)
        })
    };
    for w in [20.0, 27.5, 33.0] {
        for (short, long) in [(vec![w], vec![w, w, w]), (vec![40.0, w], vec![40.0, w, w, w, w, w])] {
            match (cost(&short), cost(&long)) {
                (Ok((n1, d1)), Ok((n2, d2))) => {
                    assert_eq!(n1, n2, "{short:?} vs {long:?}");
                    assert!(close(d1, d2), "{short:?}: {d1} vs {long:?}: {d2}");
                }
                (a, b) => assert_eq!(a.is_err(), b.is_err(), "{short:?}: {a:?} vs {long:?}: {b:?}"),
            }
        }
    }
}

// --- the adjustment ratio

#[test]
fn a_ratio_of_exactly_the_tolerance_is_feasible() {
    let items = [bx(6.0), glue(1.0, 1.0, 1.0), bx(6.0), forced()];
    let t = |tolerance| Params { tolerance, ..p() };
    assert_eq!(break_lines(&items, &[20.0], &t(7.0)), Ok(vec![Break { position: 3, ratio: 7.0 }]));
    assert_eq!(break_lines(&items, &[20.0], &t(6.999)), Err(Error::Infeasible));
}

#[test]
fn a_ratio_of_exactly_minus_one_is_feasible() {
    let items = [bx(6.0), glue(1.0, 1.0, 1.0), bx(6.0), forced()];
    assert_eq!(break_lines(&items, &[12.0], &p()), Ok(vec![Break { position: 3, ratio: -1.0 }]));
    assert_eq!(break_lines(&items, &[11.999], &p()), Err(Error::Infeasible));
}

#[test]
fn an_infinite_ratio_is_never_feasible() {
    let anything = Params { tolerance: f64::INFINITY, ..p() };
    assert_eq!(break_lines(&[bx(5.0), forced()], &[10.0], &anything), Err(Error::Infeasible));
    assert_eq!(break_lines(&[bx(5.0), forced()], &[3.0], &anything), Err(Error::Infeasible));
}

#[test]
fn exact_ratios_land_on_the_class_and_tolerance_boundaries() {
    // Halves and quarters everywhere: ratios of exactly -1, -0.5, 0.5, 1 and
    // the tolerance turn up all the time, and the class you give them decides
    // the fitness demerits.
    let mut rng = Rng::new(0xd1ad1c);
    let mut solved = 0;
    for case in 0..3000 {
        let shape = Shape {
            words: rng.int(2, 9) as usize,
            hyphens: 0.3,
            penalties: 0.1,
            dyadic: true,
            ..Shape::default()
        };
        let items = paragraph(&mut rng, shape);
        if (0..items.len()).filter(|&i| is_legal_break(&items, i)).count() > 16 {
            continue;
        }
        let widths = random_widths(&mut rng, case % 3 == 0, true, 4.0, 16.0);
        let params = Params {
            tolerance: rng.pick(&[0.5, 1.0, 2.0, 3.0]),
            line_penalty: rng.pick(&[0.0, 1.0, 10.0]),
            flagged_demerits: rng.pick(&[0.0, 1e6]),
            fitness_demerits: rng.pick(&[0.0, 1e6, 1e9]),
            looseness: 0,
        };
        if check_brute(&items, &widths, &params) == Verdict::Solved {
            solved += 1;
        }
    }
    assert!(solved >= 450, "only {solved} cases had a clear answer");
}

// --- the answer

#[test]
fn the_same_input_gives_the_same_answer() {
    let mut rng = Rng::new(99);
    for _ in 0..50 {
        let items = paragraph(&mut rng, Shape { words: 60, dyadic: true, ..Shape::default() });
        let widths = random_widths(&mut rng, true, true, 15.0, 30.0);
        let params = random_params(&mut rng);
        let first = break_lines(&items, &widths, &params);
        let copy = items.clone();
        for _ in 0..3 {
            assert_eq!(break_lines(&copy, &widths, &params), first);
        }
    }
}

#[test]
fn an_empty_paragraph_has_no_lines() {
    assert_eq!(break_lines(&[], &[10.0], &p()), Ok(vec![]));
}

#[test]
fn a_paragraph_must_end_with_a_forced_break() {
    for items in [
        vec![bx(3.0)],
        vec![bx(3.0), glue(1.0, 1.0, 1.0)],
        vec![bx(3.0), penalty(0.0, 0.0, false)],
        vec![bx(3.0), penalty(0.0, -9999.0, false)],
        vec![bx(3.0), forced(), bx(3.0)],
    ] {
        assert_eq!(break_lines(&items, &[10.0], &p()), Err(Error::Unterminated), "{items:?}");
    }
}

#[test]
#[should_panic]
fn no_line_widths_is_a_bug_in_the_caller() {
    let _ = break_lines(&[bx(3.0), forced()], &[], &p());
}

#[test]
fn errors_are_real_errors() {
    fn is_error<E: std::error::Error + Send + Sync + 'static>(_: &E) {}
    is_error(&Error::Infeasible);
    let (a, b) = (Error::Infeasible.to_string(), Error::Unterminated.to_string());
    assert!(!a.is_empty() && !b.is_empty() && a != b, "{a:?} / {b:?}");
    let boxed: Box<dyn std::error::Error> = Box::new(Error::Unterminated);
    assert_eq!(boxed.to_string(), b);
}
