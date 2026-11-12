"""parse_addendum_seq đọc ngược số thứ tự phụ lục từ số đã dựng theo khuôn."""
from app.services import numbering


def test_reads_seq_back_from_default_style_number():
    assert numbering.parse_addendum_seq("PL03/01/ABC/2026", "PL{seq}/{parent}") == 3


def test_number_that_does_not_match_the_format_gives_none():
    assert numbering.parse_addendum_seq("XX03/01/ABC/2026", "PL{seq}/{parent}") is None


def test_seq_at_the_end_of_the_format_is_still_read():
    assert numbering.parse_addendum_seq("HD-1-PL07", "{parent}-PL{seq}") == 7


def test_no_default_format_uses_the_neutral_default():
    assert numbering.parse_addendum_seq("PL12/HD-X") == 12
