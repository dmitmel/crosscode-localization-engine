#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../"
exec cbindgen --config cbindgen.toml src/ffi.rs --output ffi/crosslocale.h "$@"
