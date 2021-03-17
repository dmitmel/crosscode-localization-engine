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
    let message_str;

    try {
      message_str = await new Promise((resolve, reject) => {
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

    let message = JSON.parse(message_str);
    console.log(`recv[${humanizeByteSize(Buffer.byteLength(message_str))}]`, inspect(message));
  }
})();

for (let [request_index, request] of [
  { type: 'Backend/info' },
  { type: 'Project/open', dir: 'tmp' },
  { type: 'Project/get_meta', project_id: 1 },
  { type: 'Project/list_tr_files', project_id: 1 },
  {
    type: 'VirtualGameFile/list_fragments',
    project_id: 1,
    file_path: 'data/maps/hideout/entrance.json',
  },
].entries()) {
  let message = {
    type: 'req',
    id: request_index + 1,
    data: request,
  };
  let message_str = JSON.stringify(message);
  console.log(`send[${humanizeByteSize(Buffer.byteLength(message_str))}]`, inspect(message));
  backend.send_message(message_str);
}

backend.close();
