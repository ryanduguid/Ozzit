"""Relationship targets resolved to the archive entries they name.

Open Packaging Conventions (ECMA-376 Part 2) resolve a relationship's Target
against the part that holds the relationship: an absolute target
("/xl/worksheets/sheet1.xml") from the package root, a relative one
("worksheets/sheet1.xml") from the source part's folder. Excel writes relative
targets; other producers write absolute ones, and prefixing "xl/" to either
form misreads the one it was not written for.
"""

from __future__ import annotations

import posixpath


def resolve_target(source: str, target: str) -> str:
    """The archive entry a relationship held by `source` names with `target`."""
    return posixpath.normpath(posixpath.join(posixpath.dirname(source), target)).lstrip("/")
