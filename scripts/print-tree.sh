
#!/bin/sh
set -eu
if command -v tree >/dev/null 2>&1; then
  tree -a -I "target|data|.git"
else
  find . -maxdepth 4 -type f | sort
fi
