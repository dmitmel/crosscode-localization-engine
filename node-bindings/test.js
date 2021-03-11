const addon = require('./lib');

addon.init_logging();

let backend = new addon.Backend();
let req = {
  type: 'req',
  id: 1,
  data: {
    type: 'Backend/info',
  },
};
console.log(req);
backend.send_message(JSON.stringify(req));
let res = JSON.parse(backend.recv_message());
console.log(res);
