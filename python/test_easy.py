"""Easy: does it break lines at all?"""

import unittest

from trials import *


def positions(breaks):
    return [b.position for b in breaks]


class Easy(unittest.TestCase):
    def test_a_short_paragraph_is_one_line(self):
        items = text("Great art Thou, O Lord")
        self.assertEqual(break_lines(items, [40 * CH], Params()), [Break(len(items) - 1, 0.0)])

    def test_an_exact_fit_needs_no_glue(self):
        self.assertEqual(break_lines([bx(10), forced()], [10], Params()), [Break(1, 0.0)])

    def test_a_forced_break_is_always_taken(self):
        # Both lines fit exactly, and the break between them is forced.
        items = [bx(3), forced(), bx(3), forced()]
        self.assertEqual(positions(break_lines(items, [3], Params())), [1, 3])

    def test_a_line_breaks_at_the_space(self):
        # [B3, G, B3, P+inf, fill, P-inf]: "aaa" fills the first line exactly.
        items = text("aaa bbb")
        self.assertEqual(break_lines(items, [3 * CH], Params()), [Break(1, 0.0), Break(5, 0.0)])

    def test_a_box_too_wide_for_the_line_is_infeasible(self):
        with self.assertRaises(Infeasible):
            break_lines([bx(10), forced()], [5], Params())

    def test_every_line_is_legal_and_feasible(self):
        items = text(CONFESSIONS)
        for width in [30 * CH, 40 * CH, 50 * CH, 60 * CH]:
            with self.subTest(width=width):
                validate(items, [width], Params(), break_lines(items, [width], Params()))

    def test_a_small_paragraph_comes_out_optimal(self):
        items = text("Great art Thou, O Lord, and greatly to be praised")
        for width in [17 * CH, 20 * CH, 26 * CH, 30 * CH]:
            with self.subTest(width=width):
                self.assertEqual(check_brute(items, [width], Params()), Verdict.SOLVED)


if __name__ == "__main__":
    unittest.main()
