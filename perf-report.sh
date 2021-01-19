#!/usr/bin/env bash
set -euo pipefail

exec perf report --hierarchy -M intel
