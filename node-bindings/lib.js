var fs = require('fs');
var PROFILES = ['Debug', 'Release'];
var existing_addon_path = null;
for (var i = 0; i < PROFILES.length; i++) {
  var addon_path = './build/' + PROFILES[i] + '/crosslocale.node';
  try {
    fs.accessSync(addon_path, fs.constants.F_OK);
  } catch (e) {
    if (e.code === 'ENOENT' && e.path === addon_path) {
      continue;
    }
    throw e;
  }
  existing_addon_path = addon_path;
}
if (existing_addon_path == null) {
  throw new Error('Native addon not found!');
}
module.exports = require(existing_addon_path);
