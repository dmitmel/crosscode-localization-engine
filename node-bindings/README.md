```sh
node-gyp rebuild -- -f 'make' -f 'compile_commands_json'
ln -sv ./Release/compile_commands.json .
```
