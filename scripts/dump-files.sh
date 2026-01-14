
#!/bin/sh
set -eu
# Print all tracked source/config files to stdout in a deterministic order.
find . -type f \
  ! -path "./target/*" \
  ! -path "./data/*" \
  ! -path "./.git/*" \
  | LC_ALL=C sort \
  | while IFS= read -r f; do
      echo "===== ${f} ====="
      sed -e 's/	/    /g' "$f"
      echo
    done
