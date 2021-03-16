#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../"
cbindgen --config cbindgen.toml src/ffi.rs "$@" \
  | clang-format --assume-filename=ffi/crosslocale.h \
  > ffi/crosslocale.h
