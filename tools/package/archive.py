"""Normalize COFF archive names and host-bearing debug records."""
from collections import Counter
from pathlib import PureWindowsPath
import shutil


def contains_workspace_path(path, roots):
    needles = {(value.encode(encoding).lower(), encoding) for root in roots
               for value in (str(root), root.as_posix(), str(root).replace('/', '\\'))
               for encoding in ('utf-8', 'utf-16-le')}
    patterns = []
    for needle, encoding in needles:
        # A relative component such as src/workspace is not the absolute
        # /workspace root. Likewise /workspace-other is a different root.
        before = {char.encode(encoding) for char in 'abcdefghijklmnopqrstuvwxyz0123456789_./\\:-'}
        after = {char.encode(encoding) for char in 'abcdefghijklmnopqrstuvwxyz0123456789_.-'}
        patterns.append((needle, 2 if encoding == 'utf-16-le' else 1, before, after))
    overlap = max(len(needle) for needle, _ in needles) + 4
    with path.open('rb') as stream:
        tail = b''
        while True:
            chunk = stream.read(1024 * 1024)
            data = (tail + chunk).lower()
            for needle, width, before, after in patterns:
                offset = 0
                while (start := data.find(needle, offset)) >= 0:
                    end = start + len(needle)
                    offset = start + 1
                    # The overlap's leading bytes lost their preceding context;
                    # those matches were already considered in the prior chunk.
                    if tail and start < width:
                        continue
                    if chunk and end + width > len(data):
                        continue
                    if data[max(0, start-width):start] not in before and data[end:end+width] not in after:
                        return True
            if not chunk:
                return False
            tail = data[-overlap:]


def normalize_coff(runner, librarian, source, destination, objcopy, roots):
    members = runner.run('archive-members', [librarian, '/nologo', '/list', source]).splitlines()
    names = [PureWindowsPath(name).name for name in members]
    absolute = [name for name in members if PureWindowsPath(name).is_absolute()]
    counts = Counter(names)
    if not members or any(counts[PureWindowsPath(name).name] != 1 for name in absolute):
        raise RuntimeError('Empty archive or ambiguous absolute member basenames')
    if not absolute:
        shutil.copy2(source, destination)
        return
    work = runner.out / 'archive-members'
    work.mkdir()
    for index, name in enumerate(absolute):
        runner.run(f'archive-extract-{index}', [librarian, '/nologo', source,
                   '/extract:' + name, '/out:' + PureWindowsPath(name).name], cwd=work)
        member = work / PureWindowsPath(name).name
        if contains_workspace_path(member, roots):
            # MASM does not honor the C compiler's path mapping. Remove only
            # debug sections from this extracted copy, preserving code/data.
            runner.run(f'archive-strip-debug-{index}', [objcopy, '--strip-debug', member.name], cwd=work)
            if contains_workspace_path(member, roots):
                raise RuntimeError('Non-debug workspace path remains in '+member.name)
    # /REMOVE is applied after combining inputs, so removal and re-addition
    # deliberately use separate invocations. Source Cargo artifacts stay intact.
    remove = work / 'remove.rsp'
    remove.write_text('\n'.join(['"' + str(source) + '"', '/out:remaining.lib'] +
                                ['/remove:"' + name + '"' for name in absolute]), encoding='utf-8')
    runner.run('archive-remove', [librarian, '/nologo', '@remove.rsp'], cwd=work)
    add = work / 'add.rsp'
    add.write_text('\n'.join(['remaining.lib', '/out:"' + str(destination) + '"'] +
                             ['"' + PureWindowsPath(name).name + '"' for name in absolute]), encoding='utf-8')
    runner.run('archive-rebuild', [librarian, '/nologo', '@add.rsp'], cwd=work)
    observed = runner.run('archive-members-final', [librarian, '/nologo', '/list', destination]).splitlines()
    if Counter(observed) != Counter(names):
        raise RuntimeError('Archive member inventory changed during normalization')
