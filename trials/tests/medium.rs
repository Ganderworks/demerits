//! Medium: is it really Knuth–Plass, or greedy in a nice coat?
#![cfg(feature = "medium")]

use trials::*;

#[test]
fn ratios_are_what_the_glue_has_to_do() {
    // [B16, G(4, +8, -2), B16]: natural 36, stretch 8, shrink 2.
    let items = [bx(16), glue(4, 8, 2), bx(16), forced()];
    let p = Params::default();
    assert_eq!(break_lines(&items, &[44], &p), Ok(vec![Break { position: 3, ratio: 1.0 }]));
    assert_eq!(break_lines(&items, &[35], &p), Ok(vec![Break { position: 3, ratio: -0.5 }]));
    assert_eq!(break_lines(&items, &[36], &p), Ok(vec![Break { position: 3, ratio: 0.0 }]));
}

#[test]
fn tolerance_decides_what_is_feasible() {
    // One line, ratio (20 - 13) / 1 = 7. Breaking at the glue leaves a lone
    // box with no stretch, which is never feasible.
    let items = [bx(6), glue(1, 1, 1), bx(6), forced()];
    let p = |tolerance| Params { tolerance, ..Params::default() };
    assert_eq!(break_lines(&items, &[20], &p(5.0)), Err(Error::Infeasible));
    assert_eq!(break_lines(&items, &[20], &p(8.0)), Ok(vec![Break { position: 3, ratio: 7.0 }]));
}

#[test]
fn it_beats_greedy() {
    let items = text(CONFESSIONS);
    let params = Params::default();
    let mut beaten = Vec::new();
    for width in 20..=60 {
        let widths = [width * CH];
        if check(&items, &widths, &params) != Verdict::Solved {
            continue;
        }
        let ours = break_lines(&items, &widths, &params).unwrap();
        let ours = validate(&items, &widths, &params, &ours).unwrap().demerits;
        let theirs: Vec<usize> = greedy(&items, &widths).unwrap().iter().map(|b| b.position).collect();
        match evaluate(&items, &widths, &params, &theirs) {
            Ok(g) if close(g.demerits, ours) => {}
            _ => beaten.push(width),
        }
    }
    // Not a check on you (check() already held you to the optimum); a check
    // that this test means something.
    assert!(beaten.len() >= 10, "greedy only lost at widths {beaten:?}");
}

#[test]
fn line_penalty_trades_lines_for_spacing() {
    let mut rng = Rng::new(0x11e);
    let mut differ = 0;
    for _ in 0..200 {
        let items = paragraph(&mut rng, Shape { words: 40, ..Shape::default() });
        let widths = random_widths(&mut rng, false, false, 18, 35);
        let cheap = Params { line_penalty: 0.0, tolerance: 5.0, ..Params::default() };
        let dear = Params { line_penalty: 1000.0, ..cheap };
        if check(&items, &widths, &cheap) != Verdict::Solved || check(&items, &widths, &dear) != Verdict::Solved {
            continue;
        }
        let a = break_lines(&items, &widths, &cheap).unwrap().len();
        let b = break_lines(&items, &widths, &dear).unwrap().len();
        assert!(b <= a, "a dearer line made more lines ({b} > {a})\n{}", repro(&items, &widths, &dear));
        if b < a {
            differ += 1;
        }
    }
    assert!(differ >= 2, "line_penalty only mattered {differ} times");
}

#[test]
fn penalties_steer_the_breaks() {
    let mut rng = Rng::new(0xbad_c0de);
    let mut solved = 0;
    for _ in 0..400 {
        let shape = Shape { words: rng.int(3, 11) as usize, hyphens: 0.0, penalties: 0.5, ..Shape::default() };
        let items = paragraph(&mut rng, shape);
        let widths = random_widths(&mut rng, false, false, 8, 20);
        let params = Params { tolerance: 3.0, ..Params::default() };
        if check_brute(&items, &widths, &params) == Verdict::Solved {
            solved += 1;
        }
    }
    assert!(solved >= 200, "only {solved} of 400 had a clear answer");
}

#[test]
fn small_paragraphs_match_brute_force() {
    let mut rng = Rng::new(42);
    let mut solved = 0;
    for _ in 0..500 {
        let shape = Shape { words: rng.int(2, 12) as usize, hyphens: 0.0, penalties: 0.0, ..Shape::default() };
        let items = paragraph(&mut rng, shape);
        let widths = random_widths(&mut rng, false, false, 8, 24);
        let params = random_params(&mut rng);
        if check_brute(&items, &widths, &params) == Verdict::Solved {
            solved += 1;
        }
    }
    assert!(solved >= 250, "only {solved} of 500 had a clear answer");
}
