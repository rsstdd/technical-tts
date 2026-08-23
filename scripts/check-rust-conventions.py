#!/usr/bin/env python3
"""Enforce the `crates/AGENTS.md` rules rustfmt cannot express.

`AGENTS.md` caps every Rust line at 100 columns, and `crates/AGENTS.md` §3 caps
whole-line comments at "80 chars including sigils, excluding indentation — or
the 100-char limit including indentation, whichever is smaller."

`crates/AGENTS.md` §3 also requires that doc comments precede attributes.

rustfmt enforces none of these. `max_width` does not apply to comment text,
`wrap_comments`/`comment_width` are nightly-only, and rustfmt preserves the
order it is given for an item's doc comments and attributes. Without this
check the rules are prose that drifts, which is how the comment width came to
be violated in 331 places and the doc order on four public types before
anything reported either.

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

# The last line of an attribute, whether written on one line or broken across
# several. A blank line between an attribute and a doc block means they belong
# to different items, so only a directly adjacent pair is a violation.
ATTRIBUTE_END = ("]",)


def doc_order_violations(path: pathlib.Path, lines: list[str]):
    """Yields each doc block that an attribute for the same item sits above."""
    number = 0
    while number < len(lines):
        if not lines[number].lstrip().startswith("///"):
            number += 1
            continue
        start = number
        while number < len(lines) and lines[number].lstrip().startswith("///"):
            number += 1
        above = lines[start - 1].strip() if start else ""
        preceding = "".join(lines[max(0, start - 8) : start])
        closes_attribute = above.endswith(ATTRIBUTE_END) and "#[" in preceding
        if above.startswith("#[") or closes_attribute:
            item = lines[number].strip() if number < len(lines) else "?"
            yield (
                f"{path}:{start + 1}: doc comment follows the attribute `{above}`; "
                f"`crates/AGENTS.md` §3 puts doc comments before attributes ({item})"
            )


def violations(path: pathlib.Path):
    lines = path.read_text().splitlines()
    yield from doc_order_violations(path, lines)
    for number, line in enumerate(lines, 1):
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
        print(f"\n{len(found)} convention violation(s) in {len(files)} file(s).")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
