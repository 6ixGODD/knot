"""Reject absolute build paths without rejecting relative source inventories."""
from pathlib import Path, PurePosixPath, PureWindowsPath
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'tools/package'))
from archive import contains_workspace_path


class ArchivePaths(unittest.TestCase):
    def check_bytes(self, data, roots, expected):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'input'
            path.write_bytes(data)
            self.assertEqual(contains_workspace_path(path, roots), expected)

    def test_absolute_and_relative(self):
        for encoding in ('utf-8', 'utf-16-le'):
            for value, expected in [('/workspace/a.rs', True),
                                    ('"/workspace"', True),
                                    ('src/workspace/a.rs', False),
                                    ('tests/workspace_graph.rs', False),
                                    ('/workspace-other/a.rs', False)]:
                with self.subTest(encoding=encoding, value=value):
                    self.check_bytes(value.encode(encoding), [PurePosixPath('/workspace')], expected)

    def test_windows_spellings(self):
        for value in ('D:/repo/src/a.rs', 'd:\\repo\\src\\a.rs'):
            for encoding in ('utf-8', 'utf-16-le'):
                self.check_bytes(value.encode(encoding), [PureWindowsPath('D:/repo')], True)

    def test_chunk_boundaries(self):
        for encoding in ('utf-8', 'utf-16-le'):
            root = '/workspace'.encode(encoding)
            for offset in range(-2, 3):
                prefix = b'\x00' * (1024 * 1024 - len(root) + offset)
                self.check_bytes(prefix + root + '/a'.encode(encoding), [PurePosixPath('/workspace')], True)
                self.check_bytes(prefix + root + '-other/a'.encode(encoding), [PurePosixPath('/workspace')], False)

    def test_relative_path_at_overlap_start(self):
        for offset in range(-4, 5):
            data = b'\x00' * (1024 * 1024 - 24 - 3 + offset)
            self.check_bytes(data + b'src/workspace/a' + b'\x00' * 64,
                             [PurePosixPath('/workspace')], False)


if __name__ == '__main__':
    unittest.main()
