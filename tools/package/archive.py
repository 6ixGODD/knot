"""Normalize COFF archive names and host-bearing debug records."""
from collections import Counter
from pathlib import PureWindowsPath
import shutil


def contains_workspace_path(path, roots):
    needles = {value.encode(encoding).lower() for root in roots
               for value in (str(root), root.as_posix(), str(root).replace('/', '\\'))
               for encoding in ('utf-8', 'utf-16-le')}
    overlap = max(map(len, needles)) - 1
    with path.open('rb') as stream:
        tail = b''
        while chunk := stream.read(1024 * 1024):
            data = (tail + chunk).lower()
            if any(needle in data for needle in needles):
                return True
            tail = data[-overlap:]
    return False


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
