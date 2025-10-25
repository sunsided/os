#!/usr/bin/env bash
set -euo pipefail

OUTFILE="TODO.md"
TMPFILE="$(mktemp)"

# Write header
{
  printf "# TODOs\n\n"
  printf "_Auto-generated from code comments. Do not edit manually, recreate with \`just todo\`._\n\n"
} > "$TMPFILE"

# Collect all tracked and untracked Rust files, respecting .gitignore
mapfile -t FILES < <(git ls-files -co --exclude-standard -- '*.rs')

FOUND=0
FIRST_SECTION=1

for FILE in "${FILES[@]}"; do
  [ -f "$FILE" ] || continue

  PERL_OUT=$(
    perl -ne '
      if (m/todo\b(.*)/i) {
        my $txt = $1;
        $txt =~ s/^\s*[:\-–—]*\s*//;           # trim separators after TODO
        $txt =~ s/^\s*(\/\/\/|\/\/|#|\*)\s*//; # trim comment leaders
        $txt =~ s/\s*\*\/\s*$//;               # trim block comment closer
        $txt =~ s/^\s*$/(no details)/;
        print "$.:$txt\n";
      }
    ' "$FILE"
  )

  # Only include files with TODOs
  if [ -n "$PERL_OUT" ]; then
    FOUND=1
    if [ $FIRST_SECTION -eq 1 ]; then
      FIRST_SECTION=0
    else
      printf "\n" >> "$TMPFILE"
    fi

    URL_FILE=${FILE// /%20}
    printf "## [%s](./%s)\n\n" "$FILE" "$URL_FILE" >> "$TMPFILE"

    while IFS= read -r LINE; do
      LN="${LINE%%:*}"
      TXT="${LINE#*:}"
      printf -- "- Line [%s](./%s#L%s): %s\n" "$LN" "$URL_FILE" "$LN" "$TXT" >> "$TMPFILE"
    done <<< "$PERL_OUT"
  fi
done

if [ "$FOUND" -eq 0 ]; then
  printf "\n_No TODOs found._\n" >> "$TMPFILE"
fi

mv "$TMPFILE" "$OUTFILE"

# Normalize EOF: exactly one trailing newline
perl -0777 -pe 's/\n+\z/\n/' -i "$OUTFILE"
