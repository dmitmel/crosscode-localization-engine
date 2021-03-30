#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../"
cbindgen --config cbindgen.toml src/ffi.rs --output ffi/crosslocale.h
if command -v clang-format &>/dev/null; then
  clang-format -i ffi/crosslocale.h
fi
