#!/usr/bin/env python3
"""Enforce the line-width rules rustfmt cannot express.

`AGENTS.md` caps every Rust line at 100 columns, and `crates/AGENTS.md` §3 caps
whole-line comments at "80 chars including sigils, excluding indentation — or
the 100-char limit including indentation, whichever is smaller."

rustfmt enforces neither for comments: `max_width` does not apply to comment
text, and `wrap_comments`/`comment_width` are nightly-only. Without this check
the rules are prose that drifts, which is how they came to be violated in 331
places before anything reported it.

Exits non-zero and prints one line per violation, in a format editors can jump
to. Pass paths to check a subset; the default is every `.rs` file under
`crates/`.
"""

import pathlib
import re
import sys

LINE_LIMIT = 100
COMMENT_LIMIT = 80

# A whole-line comment: the first non-whitespace on the line opens a comment.
# `////` is a divider rule, not prose, so it is left alone.
WHOLE_LINE_COMMENT = re.compile(r"^(\s*)(///|//!|//)(?!/)")


def violations(path: pathlib.Path):
    for number, line in enumerate(path.read_text().splitlines(), 1):
        if len(line) > LINE_LIMIT:
            yield f"{path}:{number}: line is {len(line)} columns (limit {LINE_LIMIT})"
        match = WHOLE_LINE_COMMENT.match(line)
        if match:
            body = line[len(match.group(1)) :]
            if len(body) > COMMENT_LIMIT:
                yield (
                    f"{path}:{number}: whole-line comment is {len(body)} chars "
                    f"excluding indentation (limit {COMMENT_LIMIT})"
                )


def main(argv: list[str]) -> int:
    roots = [pathlib.Path(argument) for argument in argv[1:]] or [pathlib.Path("crates")]
    files = sorted(
        path
        for root in roots
        for path in ([root] if root.is_file() else root.rglob("*.rs"))
    )
    found = [message for path in files for message in violations(path)]
    for message in found:
        print(message)
    if found:
        print(f"\n{len(found)} line-width violation(s) in {len(files)} file(s).")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
