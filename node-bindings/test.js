const addon = require('./lib');

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
    console.log('recv', message);
  }
})();

for (let [request_index, request] of [
  { type: 'Backend/info' },
  { type: 'Project/open', dir: 'tmp' },
  { type: 'Project/get_meta', project_id: 1 },
  { type: 'Project/list_tr_files', project_id: 1 },
].entries()) {
  let message = {
    type: 'req',
    id: request_index + 1,
    data: request,
  };
  console.log('send', message);
  let message_str = JSON.stringify(message);
  backend.send_message(message_str);
}

backend.close();
