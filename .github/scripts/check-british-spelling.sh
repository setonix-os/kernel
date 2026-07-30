#!/usr/bin/env bash
# Enforces CLAUDE.md §11.6 — British English in all documentation and comments.
#
# Derived from the `/check-spelling` slash command in the maintainer's tron_grid
# project, promoted from a thing you must remember to run into a thing CI will
# not let you forget. The word list is that command's table.
#
# Scope and deliberate exclusions:
#
#   vendor/            Vendored MIT code is preserved verbatim (CLAUDE.md §11.5).
#                      Rewording an upstream's American spelling would corrupt
#                      the very provenance the licence obliges us to keep.
#   LICENCE            Legal text, quoted exactly.
#   CODE_OF_CONDUCT.md Off limits by house rule.
#   target/            Build output.
#
# Code identifiers may legitimately be American where they match a Rust or
# hardware-API convention (`Serialize`, `VkColorSpaceKHR`, `initialize` as a
# protocol method name). Those live in ALLOWED_PATTERNS below, and each addition
# should name the convention it is honouring.

set -euo pipefail

export LC_ALL=C

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "${script_dir}/../.." && pwd)
cd "${repo_root}"

# American form -> British form.
declare -A WORDS=(
    [color]=colour              [colors]=colours              [colored]=coloured
    [behavior]=behaviour        [behaviors]=behaviours
    [optimize]=optimise         [optimized]=optimised         [optimization]=optimisation
    [center]=centre             [centered]=centred
    [meter]=metre
    [synchronize]=synchronise   [synchronized]=synchronised
    [initialize]=initialise     [initialized]=initialised     [initialization]=initialisation
    [analyze]=analyse           [analyzed]=analysed
    [organize]=organise         [organization]=organisation
    [recognize]=recognise
    [customize]=customise
    [utilize]=utilise
    [minimize]=minimise         [maximize]=maximise
    [normalize]=normalise
    [authorize]=authorise       [authorization]=authorisation
    [serialize]=serialise       [deserialize]=deserialise
    [finalize]=finalise
    [paralyze]=paralyse
    [catalog]=catalogue
    [dialog]=dialogue
    [gray]=grey
    [defense]=defence           [offense]=offence             [pretense]=pretence
    [fulfill]=fulfil
    [enrollment]=enrolment
    [modeling]=modelling
    [traveling]=travelling
    [canceled]=cancelled
    [labeled]=labelled
    [sanitize]=sanitise         [sanitized]=sanitised
    [virtualization]=virtualisation
)

# Lines matching any of these are exempt. Keep each entry justified.
ALLOWED_PATTERNS=(
    'serde'                     # serde::Serialize / Deserialize are API names
    'Serialize|Deserialize'     # ditto, where serde is not on the same line
    'notifications/initialized' # JSON-RPC spec identifier
    'CARGO_TERM_COLOR'          # cargo environment variable
    'FORCE_COLOR|NO_COLOR'      # de facto environment-variable standards
    '--color'                   # command-line flags of external tools
    'colorspace|ColorSpace'     # graphics API names (Vulkan, DRM)
    'virtualization='           # QEMU -machine property name, quoted literally
)

allow_re=$(IFS='|'; echo "${ALLOWED_PATTERNS[*]}")

# This script is excluded from its own scan: the word table below is, by
# necessity, a list of American spellings.
mapfile -t files < <(
    git ls-files -- \
        '*.md' '*.rs' '*.toml' '*.yml' '*.yaml' '*.sh' '*.s' '*.S' '*.ld' '*.json' '*.jsonc' \
        ':!:vendor/**' ':!:LICENCE' ':!:CODE_OF_CONDUCT.md' ':!:target/**' \
        ':!:.github/scripts/check-british-spelling.sh'
)

if [[ ${#files[@]} -eq 0 ]]; then
    echo "OK: no files to check"
    exit 0
fi

violations=0

for american in "${!WORDS[@]}"; do
    british=${WORDS[$american]}

    # -w whole words, -i case-insensitive (catches Color, COLOR, color).
    while IFS=: read -r file line text; do
        [[ -z "$file" ]] && continue
        if [[ -n "$allow_re" ]] && printf '%s' "$text" | grep -qE "$allow_re"; then
            continue
        fi
        printf '%s:%s: "%s" -> use "%s"\n' "$file" "$line" "$american" "$british"
        printf '    %s\n' "$(printf '%s' "$text" | sed 's/^[[:space:]]*//' | cut -c1-120)"
        violations=$((violations + 1))
    done < <(grep -rniwn -- "$american" "${files[@]}" 2>/dev/null || true)
done

if [[ $violations -ne 0 ]]; then
    echo >&2
    echo "ERROR: $violations British-spelling violation(s) — see CLAUDE.md §11.6." >&2
    echo "If a hit is a legitimate API identifier, add it to ALLOWED_PATTERNS in" >&2
    echo "$0 with a comment naming the convention it honours." >&2
    exit 1
fi

echo "OK: British spelling clean across ${#files[@]} file(s)"
