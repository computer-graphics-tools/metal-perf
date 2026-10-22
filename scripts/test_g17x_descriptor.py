"""Cross-check the general G17X compute reduction against its end-pass reduction."""

import unittest

from g17x_descriptor import compute_candidates, zero_input_end_pass


class ComputeCandidatesTest(unittest.TestCase):
    def test_zero_arguments_reduce_to_end_pass_formula(self):
        for option in (0, 1, 2):
            for device_word in (0, 4, 62, 63, 0x80000004):
                for multiplier in (0, 96, 2016):
                    with self.subTest(option=option, device_word=device_word,
                                      multiplier=multiplier):
                        record = {
                            "config": {
                                "device_count_0x1c44": 1,
                                "compute_option_0x4327": option,
                                "compute_word_0x4328": device_word,
                                "primary_mask_0x1be0": "0x3ff",
                                "alternate_mask_0x1bf0": "0x0",
                            },
                            "compute_args": {f"f{i}": 0 for i in range(1, 7)},
                            "spill_words": {
                                "0x48": multiplier,
                                "0x50": 2016,
                                "0x60": 960,
                            },
                        }
                        general = compute_candidates(record)
                        end_pass = zero_input_end_pass(record)
                        self.assertEqual(general["compute_candidate_pages_0x14"],
                                         end_pass["zero_input_candidate_pages_0x14"])
                        self.assertEqual(general["compute_candidate_pages_0x18"],
                                         end_pass["zero_input_candidate_pages_0x18"])


if __name__ == "__main__":
    unittest.main()
