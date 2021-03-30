#!/usr/bin/env node
const paths = require('path');
console.log(paths.normalize(process.argv[2]));
