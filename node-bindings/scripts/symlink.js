#!/usr/bin/env node
const fs = require('fs');
fs.symlinkSync(process.argv[2], process.argv[3]);
