#!/usr/bin/env python
from __future__ import annotations

import os.path
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.realpath(__file__)), "src"))

from crosslocale.mod_tools import run_main

run_main()
