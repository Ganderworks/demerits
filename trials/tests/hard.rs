//! Hard: hyphens, fitness classes, line shapes and looseness.
#![cfg(feature = "hard")]

use trials::*;

fn positions(breaks: &[Break]) -> Vec<usize> {
    breaks.iter().map(|b| b.position).collect()
}

/// How many times two lines in a row end at hyphens.
fn hyphen_ladders(items: &[Item], breaks: &[Break]) -> usize {
    breaks.windows(2).filter(|w| is_flagged(&items[w[0].position]) && is_flagged(&items[w[1].position])).count()
}

/// How many times neighbouring lines' fitness classes jump by more than one.
fn fitness_jumps(breaks: &[Break]) -> usize {
    let classes: Vec<u8> = std::iter::once(1).chain(breaks.iter().map(|b| fitness(b.ratio))).collect();
    classes.windows(2).filter(|w| w[0].abs_diff(w[1]) > 1).count()
}

#[test]
fn hyphens_on_consecutive_lines_cost_flagged_demerits() {
    let items = text(&hyphenate(CONFESSIONS));
    let (mut free, mut dear) = (0, 0);
    for width in 15..=45 {
        let widths = [width * CH];
        let a = Params { flagged_demerits: 0.0, fitness_demerits: 0.0, tolerance: 3.0, ..Params::default() };
        let b = Params { flagged_demerits: 1e7, ..a };
        if check(&items, &widths, &a) != Verdict::Solved || check(&items, &widths, &b) != Verdict::Solved {
            continue;
        }
        free += hyphen_ladders(&items, &break_lines(&items, &widths, &a).unwrap());
        dear += hyphen_ladders(&items, &break_lines(&items, &widths, &b).unwrap());
    }
    assert!(free > dear, "free hyphens made {free} ladders, expensive ones {dear}");
}

#[test]
fn neighbouring_lines_keep_similar_spacing() {
    let items = text(&hyphenate(CONFESSIONS));
    let (mut free, mut dear) = (0, 0);
    for width in 15..=45 {
        let widths = [width * CH];
        let a = Params { fitness_demerits: 0.0, tolerance: 3.0, ..Params::default() };
        let b = Params { fitness_demerits: 1e7, ..a };
        if check(&items, &widths, &a) != Verdict::Solved || check(&items, &widths, &b) != Verdict::Solved {
            continue;
        }
        free += fitness_jumps(&break_lines(&items, &widths, &a).unwrap());
        dear += fitness_jumps(&break_lines(&items, &widths, &b).unwrap());
    }
    assert!(free > dear, "free jumps: {free}, expensive jumps: {dear}");
}

#[test]
fn each_line_gets_its_own_width() {
    let items = text(&hyphenate(CONFESSIONS));
    let shapes: [&[i64]; 5] = [&[20, 30, 40], &[40, 20], &[15, 25, 35, 45], &[50, 50, 24], &[25]];
    let params = Params { tolerance: 3.0, ..Params::default() };
    for shape in shapes {
        let widths: Vec<i64> = shape.iter().map(|w| w * CH).collect();
        assert_ne!(check(&items, &widths, &params), Verdict::Infeasible, "widths {shape:?}");
    }
}

#[test]
fn the_last_width_repeats() {
    // Line 0 is 5 wide, every line after it is 3: "aaaaa" then "bbb" "ccc".
    let items = text("aaaaa bbb ccc");
    let breaks = break_lines(&items, &[5 * CH, 3 * CH], &Params::default()).unwrap();
    assert_eq!(positions(&breaks), vec![1, 3, 7]);
}

#[test]
fn looseness_adds_and_removes_lines() {
    let mut rng = Rng::new(0x1005e);
    let mut both = 0;
    for _ in 0..400 {
        let items = paragraph(&mut rng, Shape { words: 40, hyphens: 0.5, ..Shape::default() });
        let widths = random_widths(&mut rng, false, false, 18, 35);
        let p = |looseness| Params { looseness, tolerance: 5.0, ..Params::default() };
        let verdicts = [check(&items, &widths, &p(0)), check(&items, &widths, &p(1)), check(&items, &widths, &p(-1))];
        if verdicts.iter().any(|v| *v != Verdict::Solved) {
            continue;
        }
        let n = break_lines(&items, &widths, &p(0)).unwrap().len();
        let more = break_lines(&items, &widths, &p(1)).unwrap().len();
        let fewer = break_lines(&items, &widths, &p(-1)).unwrap().len();
        if more == n + 1 && fewer + 1 == n {
            both += 1;
        }
    }
    assert!(both >= 8, "only {both} paragraphs could go both ways");
}

#[test]
fn looseness_never_overshoots() {
    let items = text(&hyphenate(CONFESSIONS));
    for looseness in [-100, -3, 2, 7, 100] {
        for width in [22, 31, 44] {
            let p = Params { looseness, tolerance: 3.0, ..Params::default() };
            check(&items, &[width * CH], &p);
        }
    }
}

#[test]
fn forced_breaks_in_the_middle() {
    let mut rng = Rng::new(0xf0c5);
    let mut solved = 0;
    for _ in 0..300 {
        let shape = Shape { words: rng.int(10, 40) as usize, forced: 0.1, ..Shape::default() };
        let items = paragraph(&mut rng, shape);
        let widths = random_widths(&mut rng, false, false, 12, 30);
        let params = Params { tolerance: 5.0, ..random_params(&mut rng) };
        if check(&items, &widths, &params) == Verdict::Solved {
            solved += 1;
        }
    }
    assert!(solved >= 50, "only {solved} of 300 were solvable");
}

#[test]
fn hyphenated_paragraphs_match_brute_force() {
    let mut rng = Rng::new(0x4d1f);
    let mut solved = 0;
    for case in 0..500 {
        let shape = Shape { words: rng.int(2, 6) as usize, hyphens: 0.5, penalties: 0.2, ..Shape::default() };
        let items = paragraph(&mut rng, shape);
        if (0..items.len()).filter(|&i| is_legal_break(&items, i)).count() > 18 {
            continue;
        }
        let widths = random_widths(&mut rng, case % 3 == 0, false, 6, 18);
        let mut params = random_params(&mut rng);
        params.looseness = if case % 4 == 0 { rng.int(-2, 2) as i32 } else { 0 };
        if check_brute(&items, &widths, &params) == Verdict::Solved {
            solved += 1;
        }
    }
    assert!(solved >= 80, "only {solved} of 500 had a clear answer");
}
