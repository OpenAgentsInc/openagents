"""Empty family or split groups report None statistics instead of dividing by zero."""
import importlib.util
import pathlib

SPEC = importlib.util.spec_from_file_location(
    'report_kev_candidate', pathlib.Path(__file__).with_name('report-kev-candidate.py'))
report = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(report)


def test_summarize_with_no_items():
    out = report.summarize([], {})
    assert out['expected'] == 0 and out['correct'] == 0
    assert out['accuracy'] is None and out['wilson_95'] is None


def test_paired_with_no_items():
    out = report.paired([], {}, {})
    assert out['candidate_only_correct'] == 0 and out['reference_only_correct'] == 0
    assert out['accuracy_difference'] is None
    assert out['paired_item_bootstrap_95'] is None
    assert out['mcnemar_exact_two_sided'] is None


if __name__ == '__main__':
    test_summarize_with_no_items()
    test_paired_with_no_items()
    print('ok')
