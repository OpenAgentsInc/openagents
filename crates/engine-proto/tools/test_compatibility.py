import unittest
from pathlib import Path
from compatibility import check, parse

BASE = Path(__file__).resolve().parents[1] / 'compatibility/scene-v1.descriptor.pb'


class CompatibilityTest(unittest.TestCase):
    def setUp(self):
        self.before = parse(BASE.read_bytes())
        self.after = parse(BASE.read_bytes())
    def test_additive_field_is_allowed(self):
        message = self.after.file[0].message_type[0]
        field = message.field.add()
        field.name, field.number, field.type, field.label = 'extra', 90, 9, 1
        self.assertEqual(check(self.before, self.after), [])
    def test_type_and_presence_changes_are_refused(self):
        self.after.file[0].message_type[0].field[0].type = 9
        self.assertTrue(check(self.before, self.after))
        self.after = parse(BASE.read_bytes())
        self.after.file[0].message_type[0].field[0].label = 3
        self.assertTrue(check(self.before, self.after))
    def test_removal_requires_name_and_number_reservation(self):
        message = self.after.file[0].message_type[0]
        old = message.field.pop()
        self.assertTrue(check(self.before, self.after))
        message.reserved_name.append(old.name)
        reserved = message.reserved_range.add()
        reserved.start, reserved.end = old.number, old.number + 1
        self.assertEqual(check(self.before, self.after), [])
    def test_enum_reuse_is_refused(self):
        self.after.file[0].enum_type[0].value[1].name = 'SOMETHING_ELSE'
        self.assertTrue(check(self.before, self.after))
    def test_message_removal_is_refused(self):
        self.after.file[0].message_type.pop()
        self.assertTrue(check(self.before, self.after))


if __name__ == '__main__':
    unittest.main()
