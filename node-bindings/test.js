const addon = require('./lib');
const util = require('util');

function inspect(obj) {
  return util.inspect(obj, {
    depth: Infinity,
    colors: process.stdout.isTTY,
    maxArrayLength: null,
    maxStringLength: null,
  });
}

function humanizeByteSize(bytes) {
  const UNITS = ['B', 'kB', 'MB', 'GB'];
  const FACTOR_STEP = 1024;

  let unit = '';
  for (let i = 0; i < UNITS.length; i++) {
    unit = UNITS[i];
    if (bytes < FACTOR_STEP) break;
    if (i < UNITS.length - 1) bytes /= FACTOR_STEP;
  }

  return `${bytes.toFixed(2)}${unit}`;
}

addon.init_logging();

let backend = new addon.Backend();

(async () => {
  while (true) {
    let message_buf;

    try {
      message_buf = await new Promise((resolve, reject) => {
        backend.recv_message((err, message) => {
          if (err != null) reject(err);
          else resolve(message);
        });
      });
    } catch (err) {
      if (err.code === 'CROSSLOCALE_ERR_BACKEND_DISCONNECTED') {
        return;
      }
      throw err;
    }

    let message = JSON.parse(message_buf.toString('utf8'));
    console.log(`recv[${humanizeByteSize(message_buf.length)}]`, inspect(message));
    console.log(`recv[${humanizeByteSize(message_buf.length)}]`);
  }
})();

for (let [request_index, request] of [
  { type: 'get_backend_info' },
  { type: 'open_project', dir: 'tmp' },
  { type: 'get_project_meta', project_id: 1 },
  { type: 'list_files', project_id: 1, file_type: 'tr_file' },
  {
    type: 'query_fragments',
    project_id: 1,
    from_game_file: 'data/maps/hideout/entrance.json',
    select_fields: {
      fragments: ['id', 'game_file_path', 'json_path'],
    },
  },
].entries()) {
  let message = [1, request_index + 1, request.type, request];
  delete request.type;
  let message_str = JSON.stringify(message);
  console.log(`send[${humanizeByteSize(Buffer.byteLength(message_str))}]`, inspect(message));
  backend.send_message(message_str);
}

backend.close();
