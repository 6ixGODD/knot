#!/usr/bin/env python3
"""Instrument target C/C++ without instrumenting rustc's host proc macros."""
import os
from pathlib import Path
import sys


def main():
    compiler = 'clang++' if Path(sys.argv[0]).name == 'asan-cxx' else 'clang'
    flags = os.environ.get('CARGO_ENCODED_RUSTFLAGS', '').split('\x1f')
    extra = ['-fsanitize=address', '-fno-omit-frame-pointer'] if '-Zsanitizer=address' in flags else []
    os.execvp(compiler, [compiler, *extra, *sys.argv[1:]])


if __name__ == '__main__':
    main()
