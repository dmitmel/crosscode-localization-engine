// This is the version supported by nwjs 0.35.5 which comes with nodejs 11.6.0.
#define NAPI_VERSION 3
#define NODE_ADDON_API_DISABLE_DEPRECATED
#include <napi.h>

#include <crosslocale.h>

const uint32_t SUPPORTED_FFI_BRIDGE_VERSION = 3;

// NOTE: About the radical usage of crosslocale_backend_t which may seem
// thread-unsafe on the first glance: the implementation details are employed
// here because the bridge code is small enough and the alternative is to
// define a mountain of C structs. Strictly speaking crosslocale_backend_t is
// comprised of two halves: the sender channel and the receiver channel. The
// Rust struct also has a handle to the backend thread, but it is unused both
// internally and through the public C API, and exists just to prevent dropping
// that thread. Also note that there are just two "methods" on
// crosslocale_backend_t, for sending and receiving, and each one works only
// with their half of the backend. In any case, this leaves us basically with
// the type (mpsc::Sender<String>, mpsc::Receiver<String>) with a connection to
// two independent channels. As such, even though that pair is contained within
// a single struct, it's kind of safe to interact with each half on separate
// threads, as long as each side is synchronized (i.e. receiving on two threads
// isn't safe).

// NOTE 2: Actually, I later implemented synchronization on the Rust side,
// meaning that both mpsc::Sender and mpsc::Receiver are hidden behind
// std::sync::Mutexes because this turned out to be easier. Well, this will
// make the general FFI bridge very slightly slower when threading is not a
// concern, but who cares, it is intended to be used only through the nodejs
// binding anyway.

// NOTE 3: Dammit, I remembered that std::sync::Mutex can't be shared between
// multiple threads and normally needs a wrapper such as std::sync::Arc. Very
// well, I'll do this in C++ then.

class FfiBackendException : public std::exception {
public:
  crosslocale_result code;

  FfiBackendException(crosslocale_result code) : code(code) {}

  bool is_ok() const noexcept { return this->code == CROSSLOCALE_OK; };

  const char* what() const noexcept override {
    return (char*)crosslocale_error_description(this->code);
  }

  const char* id() const noexcept { return (char*)crosslocale_error_id_str(this->code); }

  Napi::Error to_node_error(Napi::Env env) const {
    Napi::Error obj = Napi::Error::New(env, this->what());
    obj.Set("errno", Napi::Number::New(env, this->code));
    if (const char* id_str = this->id()) {
      obj.Set("code", Napi::String::New(env, id_str));
    }
    return obj;
  }
};

void throw_ffi_result(crosslocale_result res) {
  if (res != CROSSLOCALE_OK) {
    throw FfiBackendException(res);
  }
}

class BackendMessage {
public:
  Napi::Value to_js_value(Napi::Env env) const {
    Napi::Value js_value = to_js_value_impl(env, this->raw);
    return js_value != nullptr ? js_value : env.Undefined();
  }

  const crosslocale_message& get_raw() const { return this->raw; }

  void operator=(const BackendMessage&) = delete;
  BackendMessage(const BackendMessage&) = delete;

protected:
  BackendMessage(crosslocale_message raw) : raw(raw) {}

  crosslocale_message raw;

  static Napi::Value to_js_value_impl(Napi::Env env, crosslocale_message raw) {
    switch (raw.type) {
    case CROSSLOCALE_MESSAGE_NIL:
      return env.Null();
    case CROSSLOCALE_MESSAGE_BOOL:
      return Napi::Boolean::New(env, raw.as.value_bool);
    case CROSSLOCALE_MESSAGE_I64:
      return Napi::Number::New(env, (double)raw.as.value_i64);
    case CROSSLOCALE_MESSAGE_F64:
      return Napi::Number::New(env, raw.as.value_f64);
    case CROSSLOCALE_MESSAGE_STR:
      return Napi::String::New(env, (char*)raw.as.value_str.ptr, raw.as.value_str.len);

    case CROSSLOCALE_MESSAGE_LIST: {
      Napi::EscapableHandleScope scope(env);
      Napi::Array js_array = Napi::Array::New(env, raw.as.value_list.len);
      if (raw.as.value_list.len <= UINT32_MAX) {
        for (uint32_t i = 0; i < (uint32_t)raw.as.value_list.len; i++) {
          crosslocale_message value = raw.as.value_list.ptr[i];
          Napi::Value js_value = to_js_value_impl(env, value);
          if (js_value != nullptr) {
            js_array.Set(i, js_value);
          }
        }
      } else {
        for (size_t i = 0; i < raw.as.value_list.len; i++) {
          crosslocale_message value = raw.as.value_list.ptr[i];
          Napi::Value js_value = to_js_value_impl(env, value);
          if (js_value != nullptr) {
            js_array.Set(Napi::Number::New(env, (double)i), js_value);
          }
        }
      }
      return scope.Escape(js_array);
    }

    case CROSSLOCALE_MESSAGE_DICT: {
      Napi::EscapableHandleScope scope(env);
      Napi::Object js_object = Napi::Object::New(env);
      for (size_t i = 0; i < raw.as.value_dict.len; i++) {
        crosslocale_message_str key = raw.as.value_dict.keys[i];
        crosslocale_message value = raw.as.value_dict.values[i];
        Napi::Value js_value = to_js_value_impl(env, value);
        if (js_value != nullptr) {
          js_object.Set(Napi::String::New(env, (char*)key.ptr, key.len), js_value);
        }
      }
      return scope.Escape(js_object);
    }

    case CROSSLOCALE_MESSAGE_INVALID:
      throw std::logic_error("encountered an explicitly invalid value");
    }
    return Napi::Value();
  }
};

class BackendMessageFromJs : public BackendMessage {
public:
  BackendMessageFromJs(Napi::Env env, Napi::Value value)
      : BackendMessage(from_js_value_impl(env, value)) {}

  ~BackendMessageFromJs() { free_raw_value(this->raw); }

  void operator=(const BackendMessageFromJs&) = delete;
  BackendMessageFromJs(const BackendMessageFromJs&) = delete;

protected:
  static void free_raw_value(crosslocale_message raw) {
    switch (raw.type) {
    case CROSSLOCALE_MESSAGE_NIL:
    case CROSSLOCALE_MESSAGE_BOOL:
    case CROSSLOCALE_MESSAGE_I64:
    case CROSSLOCALE_MESSAGE_F64:
      break;

    case CROSSLOCALE_MESSAGE_STR:
      delete[] raw.as.value_str.ptr;
      break;

    case CROSSLOCALE_MESSAGE_LIST:
      for (size_t i = 0; i < raw.as.value_list.len; i++) {
        free_raw_value(raw.as.value_list.ptr[i]);
      }
      delete[] raw.as.value_list.ptr;
      break;

    case CROSSLOCALE_MESSAGE_DICT:
      for (size_t i = 0; i < raw.as.value_dict.len; i++) {
        delete[] raw.as.value_dict.keys[i].ptr;
        free_raw_value(raw.as.value_dict.values[i]);
      }
      delete[] raw.as.value_dict.keys;
      delete[] raw.as.value_dict.values;
      break;

    case CROSSLOCALE_MESSAGE_INVALID:
      break;
    }
  }

  static crosslocale_message_str from_js_str(Napi::String value) {
    Napi::Env env = value.Env();
    size_t length;
    napi_status status = napi_get_value_string_utf8(env, value, nullptr, 0, &length);
    NAPI_THROW_IF_FAILED(env, status);
    char* data = new char[length + 1];
    status = napi_get_value_string_utf8(env, value, &data[0], length + 1, nullptr);
    NAPI_THROW_IF_FAILED(env, status);
    crosslocale_message_str str;
    str.len = length;
    str.ptr = (uint8_t*)data;
    return str;
  }

  // <https://codereview.stackexchange.com/q/260759/254963>
  static crosslocale_message from_js_value_impl(Napi::Env env, Napi::Value js_value) {
    Napi::HandleScope scope(env);

    crosslocale_message msg;
    switch (js_value.Type()) {
    case napi_undefined:
    case napi_null:
      msg.type = CROSSLOCALE_MESSAGE_NIL;
      msg.as.value_bool = false;
      return msg;

    case napi_boolean: {
      bool b = js_value.As<Napi::Boolean>().Value();
      msg.type = CROSSLOCALE_MESSAGE_BOOL;
      msg.as.value_bool = b;
      return msg;
    }

    case napi_number: {
      double n_float = js_value.As<Napi::Number>().DoubleValue();
      int64_t n_int = (uint64_t)n_float;
      if ((double)n_int == n_float) {
        msg.type = CROSSLOCALE_MESSAGE_I64;
        msg.as.value_i64 = n_int;
        return msg;
      } else {
        msg.type = CROSSLOCALE_MESSAGE_F64;
        msg.as.value_f64 = n_float;
        return msg;
      }
    }

    case napi_string: {
      crosslocale_message_str s = from_js_str(js_value.As<Napi::String>());
      msg.type = CROSSLOCALE_MESSAGE_STR;
      msg.as.value_str = s;
      return msg;
    }

    case napi_object: {
      if (js_value.IsArray()) {
        Napi::Array js_array = js_value.As<Napi::Array>();
        uint32_t len = js_array.Length();
        crosslocale_message* data = new crosslocale_message[len];
        for (size_t i = 0; i < len; i++) {
          crosslocale_message element = from_js_value_impl(env, js_array.Get(i));
          if (element.type != CROSSLOCALE_MESSAGE_INVALID) {
            data[i] = element;
          }
        }
        msg.type = CROSSLOCALE_MESSAGE_LIST;
        msg.as.value_list.len = len;
        msg.as.value_list.ptr = data;
        return msg;
      } else {
        Napi::Object js_object = js_value.As<Napi::Object>();
        Napi::Array js_keys = js_object.GetPropertyNames();
        uint32_t len = js_keys.Length();
        crosslocale_message_str* keys = new crosslocale_message_str[len];
        crosslocale_message* values = new crosslocale_message[len];
        for (size_t i = 0; i < len; i++) {
          Napi::Value js_key = js_keys.Get(i);
          crosslocale_message value = from_js_value_impl(env, js_object.Get(js_key));
          if (value.type != CROSSLOCALE_MESSAGE_INVALID) {
            crosslocale_message_str key = from_js_str(js_key.As<Napi::String>());
            keys[i] = key;
            values[i] = value;
          }
        }
        msg.type = CROSSLOCALE_MESSAGE_DICT;
        msg.as.value_dict.len = len;
        msg.as.value_dict.keys = keys;
        msg.as.value_dict.values = values;
        return msg;
      }
    }

    case napi_symbol:
    case napi_function:
    case napi_external:
    case napi_bigint:
      break;
    }
    msg.type = CROSSLOCALE_MESSAGE_INVALID;
    msg.as.value_bool = false;
    return msg;
  }
};

class BackendMessageFromRust : public BackendMessage {
public:
  BackendMessageFromRust(crosslocale_message raw) : BackendMessage(raw) {}

  ~BackendMessageFromRust() { crosslocale_message_free(&this->raw); }

  void operator=(const BackendMessageFromRust&) = delete;
  BackendMessageFromRust(const BackendMessageFromRust&) = delete;
};

class FfiBackend {
public:
  FfiBackend() {
    crosslocale_backend* raw = nullptr;
    throw_ffi_result(crosslocale_backend_new(&raw));
    this->raw = raw;
  }

  ~FfiBackend() {
    if (this->raw != nullptr) {
      crosslocale_backend* raw = this->raw;
      this->raw = nullptr;
      throw_ffi_result(crosslocale_backend_free(raw));
    }
  }

  void operator=(const FfiBackend&) = delete;
  FfiBackend(const FfiBackend&) = delete;

  std::unique_ptr<BackendMessageFromRust> recv_message() {
    std::lock_guard<std::mutex> guard(this->recv_mutex);
    crosslocale_message message;
    throw_ffi_result(crosslocale_backend_recv_message(this->raw, &message));
    return std::unique_ptr<BackendMessageFromRust>(new BackendMessageFromRust(message));
  }

  void send_message(const BackendMessage& message) {
    std::lock_guard<std::mutex> guard(this->send_mutex);
    throw_ffi_result(crosslocale_backend_send_message(this->raw, &message.get_raw()));
  }

  void close() {
    std::lock_guard<std::mutex> guard(this->send_mutex);
    throw_ffi_result(crosslocale_backend_close(this->raw));
  }

  bool is_closed() {
    std::lock_guard<std::mutex> guard(this->send_mutex);
    bool result = false;
    throw_ffi_result(crosslocale_backend_is_closed(this->raw, &result));
    return result;
  }

  static void init_logging() { throw_ffi_result(crosslocale_init_logging()); }

private:
  crosslocale_backend* raw = nullptr;
  std::mutex send_mutex;
  std::mutex recv_mutex;
};

Napi::Value init_logging(const Napi::CallbackInfo& info) {
  FfiBackend::init_logging();
  return Napi::Value();
}

class NodeRecvMessageWorker : public Napi::AsyncWorker {
public:
  NodeRecvMessageWorker(Napi::Function& callback, std::shared_ptr<FfiBackend> inner)
      : Napi::AsyncWorker(callback), inner(inner) {}

  ~NodeRecvMessageWorker() {}

  void operator=(const NodeRecvMessageWorker&) = delete;
  NodeRecvMessageWorker(const NodeRecvMessageWorker&) = delete;

  void Execute() override {
    this->has_error = false;
    try {
      this->message = this->inner->recv_message();
    } catch (const FfiBackendException& e) {
      this->error = e;
      this->has_error = true;
    }
  }

  std::vector<napi_value> GetResult(Napi::Env env) override {
    if (!this->has_error) {
      return {env.Null(), this->message->to_js_value(env)};
    } else {
      Napi::Error obj = this->error.to_node_error(env);
      return {obj.Value()};
    }
  }

private:
  std::shared_ptr<FfiBackend> inner;
  std::unique_ptr<BackendMessageFromRust> message;
  FfiBackendException error = CROSSLOCALE_OK;
  bool has_error = false;
};

class NodeBackend : public Napi::ObjectWrap<NodeBackend> {
public:
  static Napi::Object Init(Napi::Env env, Napi::Object exports) {
    Napi::Function ctor = DefineClass(env,
      "Backend",
      {
        InstanceMethod("send_message", &NodeBackend::send_message),
        InstanceMethod("recv_message", &NodeBackend::recv_message),
        InstanceMethod("recv_message_sync", &NodeBackend::recv_message_sync),
        InstanceMethod("close", &NodeBackend::close),
        InstanceMethod("is_closed", &NodeBackend::is_closed),
      });

    exports.Set("Backend", ctor);
    return exports;
  }

  NodeBackend(const Napi::CallbackInfo& info) : Napi::ObjectWrap<NodeBackend>(info) {
    Napi::Env env = info.Env();
    if (!(info.Length() == 0)) {
      NAPI_THROW_VOID(Napi::TypeError::New(env, "constructor()"));
    }
    this->inner = std::make_shared<FfiBackend>();
  }

  void operator=(const NodeBackend&) = delete;
  NodeBackend(const NodeBackend&) = delete;

  ~NodeBackend() {}

private:
  std::shared_ptr<FfiBackend> inner;

  Napi::Value send_message(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    if (!(info.Length() == 1)) {
      NAPI_THROW(Napi::TypeError::New(env, "send_message(value: any): void"), Napi::Value());
    }

    BackendMessageFromJs message(env, info[0]);
    try {
      this->inner->send_message(message);
    } catch (const FfiBackendException& e) {
      throw e.to_node_error(env);
    }

    return Napi::Value();
  }

  Napi::Value recv_message(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    if (!(info.Length() == 1 && info[0].IsFunction())) {
      NAPI_THROW(
        Napi::TypeError::New(env, "recv_message(callback: Function): void"), Napi::Value());
    }

    Napi::Function callback(env, info[0]);
    NodeRecvMessageWorker* worker = new NodeRecvMessageWorker(callback, this->inner);
    worker->Queue();

    return Napi::Value();
  }

  Napi::Value recv_message_sync(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    if (!(info.Length() == 0)) {
      NAPI_THROW(Napi::TypeError::New(env, "recv_message_sync(): any"), Napi::Value());
    }

    std::unique_ptr<BackendMessageFromRust> message;
    try {
      message = this->inner->recv_message();
    } catch (const FfiBackendException& e) {
      throw e.to_node_error(env);
    }
    return message->to_js_value(env);
  }

  Napi::Value close(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    if (!(info.Length() == 0)) {
      NAPI_THROW_VOID(Napi::TypeError::New(env, "close(): void"));
    }

    try {
      this->inner->close();
    } catch (const FfiBackendException& e) {
      throw e.to_node_error(env);
    }
    return Napi::Value();
  }

  Napi::Value is_closed(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    if (!(info.Length() == 0)) {
      NAPI_THROW_VOID(Napi::TypeError::New(env, "close(): void"));
    }

    bool is_closed = false;
    try {
      is_closed = this->inner->is_closed();
    } catch (const FfiBackendException& e) {
      throw e.to_node_error(env);
    }
    return Napi::Boolean::New(env, is_closed);
  }
};

Napi::Object Init(Napi::Env env, Napi::Object exports) {
  if (CROSSLOCALE_FFI_BRIDGE_VERSION != SUPPORTED_FFI_BRIDGE_VERSION) {
    NAPI_THROW(Napi::Error::New(env,
                 "Incompatible FFI bridge version! Check if a correct "
                 "crosslocale dynamic library is installed!"),
      Napi::Object());
  }

  exports.Set(Napi::String::New(env, "FFI_BRIDGE_VERSION"),
    Napi::Number::New(env, CROSSLOCALE_FFI_BRIDGE_VERSION));
  exports.Set(Napi::String::New(env, "VERSION"),
    Napi::String::New(env, (char*)CROSSLOCALE_VERSION_PTR, CROSSLOCALE_VERSION_LEN));
  exports.Set(Napi::String::New(env, "NICE_VERSION"),
    Napi::String::New(env, (char*)CROSSLOCALE_NICE_VERSION_PTR, CROSSLOCALE_NICE_VERSION_LEN));
  exports.Set(Napi::String::New(env, "PROTOCOL_VERSION"),
    Napi::Number::New(env, CROSSLOCALE_PROTOCOL_VERSION));
  exports.Set(Napi::String::New(env, "init_logging"), Napi::Function::New(env, init_logging));
  return NodeBackend::Init(env, exports);
}

NODE_API_MODULE(crosscode_localization_engine, Init)
