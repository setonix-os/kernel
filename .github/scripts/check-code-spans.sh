#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Two Markdown hygiene rules that markdownlint does not enforce:
#
#   1. No inline code span may break across a line end. A renderer joins the two
#      halves with a space, so a span split mid-identifier shows a name that does
#      not exist, and a grep for the identifier misses the line. Re-wrap the
#      paragraph so the span sits on one line.
#   2. No typographic quotes (U+2018, U+2019, U+201C, U+201D) in our own prose.
#      House style is straight quotes; curly ones arrive by pasting from a word
#      processor and break exact-text searches.
#
# Scope: every tracked *.md file outside vendor/, whose upstream text is
# preserved verbatim (CONSTITUTION.md §11.5). CODE_OF_CONDUCT.md is also exempt
# from rule 2 only: it is upstream text kept verbatim, typography included.
#
# How rule 1 decides. Spans are matched the way CommonMark matches them, one
# paragraph at a time and outside fenced code blocks: a run of N backticks opens
# a span that the next run of exactly N backticks closes, so a double-backtick
# span may contain a single backtick; a run with no partner is literal text; a
# backslash escapes a backtick outside a span but not inside one. A span is
# reported when its content contains a line break. That is the precise form of
# "an odd number of single-backtick delimiters on a line that continues into the
# next", and unlike a per-line count it is not fooled by a stray backtick or by
# a double-backtick span. A heading, table row, list item, thematic break,
# blank line or change of blockquote depth ends the paragraph, because a span
# cannot cross any of them.
#
# Resolves paths relative to the repository root regardless of the caller's CWD.
# Needs only bash, git and a POSIX awk — the devcontainer's is mawk.

set -euo pipefail

export LC_ALL=C

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "${script_dir}/../.." && pwd -P) # physical: safe.directory matches it
cd "${repo_root}"

# The list comes from git, and a failure inside a process substitution is
# invisible to set -e. Capture it, fail loudly, and refuse an empty list: in CI
# the container's root does not own the checkout, git rejects it as "dubious
# ownership", and this script once passed by checking nothing.
if ! listing=$(git -c safe.directory="${repo_root}" ls-files -- '*.md' ':!:vendor/**'); then
    echo "ERROR: git ls-files failed in ${repo_root}; no file was checked." >&2
    exit 1
fi
mapfile -t files < <(printf '%s' "${listing}")

if [[ ${#files[@]} -eq 0 ]]; then
    echo "ERROR: git listed no Markdown files; refusing to pass on an empty set." >&2
    exit 1
fi

# shellcheck disable=SC2016 # the program is awk, and its $ fields are awk's
program='
function flush(    text, i, n, k, m, c, rest, at, open_line) {
    if (np == 0) return
    text = para[1]
    for (k = 2; k <= np; k++) text = text "\n" para[k]
    i = 1
    n = length(text)
    while (i <= n) {
        rest = substr(text, i)
        at = match(rest, /[`\\]/)
        if (at == 0) break
        i += at - 1
        c = substr(text, i, 1)
        if (c == "\\") { i += 2; continue }
        m = run_length(text, i)
        k = closing_run(text, i + m, m)
        if (k == 0) { i += m; continue }
        if (index(substr(text, i + m, k - i - m), "\n") > 0) {
            open_line = pstart + newlines_before(text, i)
            report(open_line, "inline code span breaks across a line end; keep it on one line", para[open_line - pstart + 1])
        }
        i = k + m
    }
    np = 0
}

function run_length(s, i,    m) {
    m = 0
    while (substr(s, i + m, 1) == "`") m++
    return m
}

# Position of the next run of exactly m backticks at or after j, or 0.
function closing_run(s, j, m,    rest, at, r) {
    while (1) {
        rest = substr(s, j)
        at = index(rest, "`")
        if (at == 0) return 0
        j += at - 1
        r = run_length(s, j)
        if (r == m) return j
        j += r
    }
}

function newlines_before(s, i,    k, count) {
    count = 0
    for (k = 1; k < i; k++) if (substr(s, k, 1) == "\n") count++
    return count
}

function report(line, message, text) {
    sub(/^[ \t]+/, "", text)
    printf "%s:%d: %s\n    %s\n", file, line, message, substr(text, 1, 120)
    violations++
}

function add(line) {
    if (np == 0) pstart = FNR
    para[++np] = line
}

FNR == 1 {
    flush()
    file = FILENAME
    fence = ""
    depth = 0
}

{
    raw = $0

    if (file != "CODE_OF_CONDUCT.md") {
        if (index(raw, LSQ) || index(raw, RSQ) || index(raw, LDQ) || index(raw, RDQ))
            report(FNR, "typographic quote; use a straight quote", raw)
    }

    # Strip blockquote markers, remembering how deep this line is quoted.
    line = raw
    d = 0
    while (match(line, /^[ \t]*>[ \t]?/)) { line = substr(line, RLENGTH + 1); d++ }

    if (fence != "") {
        if (match(line, /^[ \t]*(```+|~~~+)[ \t]*$/)) {
            close_run = line
            gsub(/[ \t]/, "", close_run)
            if (substr(close_run, 1, 1) == substr(fence, 1, 1) && length(close_run) >= length(fence)) fence = ""
        }
        next
    }
    # A backtick fence whose info string holds a backtick is not a fence
    # (CommonMark 4.5); treating it as one would hide the rest of the file.
    if (match(line, /^[ \t]*(```+|~~~+)/)) {
        run = substr(line, RSTART, RLENGTH)
        sub(/^[ \t]+/, "", run)
        if (!(substr(run, 1, 1) == "`" && index(substr(line, RSTART + RLENGTH), "`") > 0)) {
            flush()
            fence = run
            next
        }
    }

    if (d != depth) { flush(); depth = d }

    if (line ~ /^[ \t]*$/) { flush(); next }
    if (line ~ /^ ? ? ?(-[ \t]*-[ \t]*-[- \t]*|\*[ \t]*\*[ \t]*\*[* \t]*|_[ \t]*_[ \t]*_[_ \t]*)$/) { flush(); next }
    if (line ~ /^ ? ? ?#+([ \t]|$)/ || line ~ /^[ \t]*\|/) { flush(); add(line); flush(); next }
    if (line ~ /^[ \t]*([-*+]|[0-9]+[.)])([ \t]|$)/) flush()
    add(line)
}

END {
    flush()
    exit (violations > 0)
}
'

status=0
awk -v LSQ=$'\xe2\x80\x98' -v RSQ=$'\xe2\x80\x99' -v LDQ=$'\xe2\x80\x9c' -v RDQ=$'\xe2\x80\x9d' \
    "$program" "${files[@]}" || status=$?

if [[ $status -ne 0 ]]; then
    echo >&2
    echo "ERROR: Markdown hygiene violation(s) above — see the header of $0." >&2
    exit 1
fi

echo "OK: code spans whole and quotes straight across ${#files[@]} Markdown file(s)"
