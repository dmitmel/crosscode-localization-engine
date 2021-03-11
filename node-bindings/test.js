const addon = require('./build/Debug/crosslocale.node');
console.log(addon);

addon.init_logging();

let backend = new addon.Backend();
let req = JSON.stringify({
  type: 'req',
  id: 1,
  data: {
    type: 'Backend/info',
  },
});
console.log(req);
backend.send_message(req);
console.log(JSON.parse(backend.recv_message()));
