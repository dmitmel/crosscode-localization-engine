// This is the version supported by nwjs 0.35.5 which comes with nodejs 11.6.0.
#define NAPI_VERSION 3
#define NODE_ADDON_API_DISABLE_DEPRECATED
#include <napi.h>

#include <crosslocale.h>

const uint32_t SUPPORTED_FFI_BRIDGE_VERSION = 1;

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
  crosslocale_result_t code;

  FfiBackendException(crosslocale_result_t code) : code(code) {}

  bool is_ok() const noexcept { return this->code == CROSSLOCALE_OK; };

  const char* what() const noexcept override {
    // The switch statement can't be used here because the following constants
    // are extern, but C++ requires the values used for case branches to be
    // strictly known at compile-time.
    if (this->code == CROSSLOCALE_OK) {
      return "this isn't actually an error";
    } else if (this->code == CROSSLOCALE_ERR_GENERIC_RUST_PANIC) {
      return "generic Rust panic";
    } else if (this->code == CROSSLOCALE_ERR_BACKEND_DISCONNECTED) {
      return "the backend thread has disconnected";
    } else if (this->code == CROSSLOCALE_ERR_NON_UTF8_STRING) {
      return "a provided string wasn't properly utf8-encoded";
    } else if (this->code == CROSSLOCALE_ERR_SPAWN_THREAD_FAILED) {
      return "failed to spawn the backend thread";
    } else {
      return "unkown error";
    }
  }

  const char* id() const noexcept {
#define BackendException_check_id(id)                                                              \
  if (this->code == id) {                                                                          \
    return #id;                                                                                    \
  }

    BackendException_check_id(CROSSLOCALE_OK);
    BackendException_check_id(CROSSLOCALE_ERR_GENERIC_RUST_PANIC);
    BackendException_check_id(CROSSLOCALE_ERR_BACKEND_DISCONNECTED);
    BackendException_check_id(CROSSLOCALE_ERR_NON_UTF8_STRING);
    BackendException_check_id(CROSSLOCALE_ERR_SPAWN_THREAD_FAILED);
    return nullptr;

#undef BackendException_check_id
  }

  Napi::Error ToNodeError(Napi::Env env) const {
    Napi::Error obj = Napi::Error::New(env, this->what());
    obj.Set("errno", Napi::Number::New(env, this->code));
    if (const char* id_str = this->id()) {
      obj.Set("code", Napi::String::New(env, id_str));
    }
    return obj;
  }
};

void throw_ffi_result(crosslocale_result_t res) {
  if (res != CROSSLOCALE_OK) {
    throw FfiBackendException(res);
  }
}

class RustString {
public:
  RustString(uint8_t* buf, size_t len, size_t cap) : buf(buf), len(len), cap(cap) {}

  ~RustString() { throw_ffi_result(crosslocale_message_free(this->buf, this->len, this->cap)); }

  void operator=(const RustString&) = delete;
  RustString(const RustString&) = delete;

  uint8_t* get_buf() { return this->buf; }
  char* get_char_buf() { return (char*)this->buf; }
  size_t get_len() { return this->len; }
  size_t get_cap() { return this->cap; }

private:
  uint8_t* buf;
  size_t len;
  size_t cap;
};

class FfiBackend {
public:
  FfiBackend() {
    crosslocale_backend_t* raw = nullptr;
    throw_ffi_result(crosslocale_backend_new(&raw));
    this->raw = raw;
  }

  ~FfiBackend() {
    if (this->raw != nullptr) {
      crosslocale_backend_t* raw = this->raw;
      this->raw = nullptr;
      throw_ffi_result(crosslocale_backend_free(raw));
    }
  }

  void operator=(const FfiBackend&) = delete;
  FfiBackend(const FfiBackend&) = delete;

  std::unique_ptr<RustString> recv_message() {
    std::lock_guard<std::mutex> guard(this->recv_mutex);
    uint8_t* message_buf = nullptr;
    size_t message_len = 0;
    size_t message_cap = 0;
    throw_ffi_result(
        crosslocale_backend_recv_message(this->raw, &message_buf, &message_len, &message_cap));
    return std::make_unique<RustString>(message_buf, message_len, message_cap);
  }

  void send_message(const uint8_t* buf, size_t len) {
    std::lock_guard<std::mutex> guard(this->send_mutex);
    throw_ffi_result(crosslocale_backend_send_message(this->raw, buf, len));
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
  crosslocale_backend_t* raw = nullptr;
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
      this->message_str = this->inner->recv_message();
    } catch (const FfiBackendException& e) {
      this->error = e;
      this->has_error = true;
    }
  }

  std::vector<napi_value> GetResult(Napi::Env env) override {
    if (!this->has_error) {
      return {env.Null(), Napi::Buffer<uint8_t>::Copy(env, this->message_str->get_buf(),
                                                      this->message_str->get_len())};
    } else {
      Napi::Error obj = error.ToNodeError(env);
      return {obj.Value()};
    }
  }

private:
  std::shared_ptr<FfiBackend> inner;
  std::unique_ptr<RustString> message_str;
  FfiBackendException error = CROSSLOCALE_OK;
  bool has_error = false;
};

class NodeBackend : public Napi::ObjectWrap<NodeBackend> {
public:
  static Napi::Object Init(Napi::Env env, Napi::Object exports) {
    Napi::Function ctor =
        DefineClass(env, "Backend",
                    {
                        InstanceMethod("send_message", &NodeBackend::send_message),
                        InstanceMethod("recv_message", &NodeBackend::recv_message),
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
    if (!(info.Length() == 1 && (info[0].IsBuffer() || info[0].IsString()))) {
      NAPI_THROW(Napi::TypeError::New(env, "send_message(text: Buffer | string): void"),
                 Napi::Value());
    }

    if (info[0].IsBuffer()) {
      Napi::Buffer<uint8_t> message(env, info[0]);
      uint8_t* data = message.Data();
      size_t len = message.Length();
      try {
        this->inner->send_message(data, len);
      } catch (const FfiBackendException& e) {
        throw e.ToNodeError(env);
      }
    } else {
      Napi::String message(env, info[0]);
      std::string std_message = message.Utf8Value();
      uint8_t* data = (uint8_t*)std_message.data();
      size_t len = std_message.length();
      try {
        this->inner->send_message(data, len);
      } catch (const FfiBackendException& e) {
        throw e.ToNodeError(env);
      }
    }

    return Napi::Value();
  }

  Napi::Value recv_message(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    if (!(info.Length() == 1 && info[0].IsFunction())) {
      NAPI_THROW(Napi::TypeError::New(env, "recv_message(callback: Function): void"),
                 Napi::Value());
    }

    Napi::Function callback(env, info[0]);
    NodeRecvMessageWorker* worker = new NodeRecvMessageWorker(callback, this->inner);
    worker->Queue();

    return Napi::Value();
  }

  Napi::Value close(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    if (!(info.Length() == 0)) {
      NAPI_THROW_VOID(Napi::TypeError::New(env, "close(): void"));
    }

    try {
      this->inner->close();
    } catch (const FfiBackendException& e) {
      throw e.ToNodeError(env);
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
      throw e.ToNodeError(env);
    }
    return Napi::Boolean::New(env, is_closed);
  }
};

Napi::Object Init(Napi::Env env, Napi::Object exports) {
  if (CROSSLOCALE_FFI_BRIDGE_VERSION != SUPPORTED_FFI_BRIDGE_VERSION) {
    NAPI_THROW(Napi::Error::New(env, "Incompatible FFI bridge version! Check if a correct "
                                     "crosslocale dynamic library is installed!"),
               Napi::Object());
  }

  exports.Set(Napi::String::New(env, "FFI_BRIDGE_VERSION"),
              Napi::Number::New(env, CROSSLOCALE_FFI_BRIDGE_VERSION));
  exports.Set(Napi::String::New(env, "VERSION"),
              Napi::String::New(env, (char*)CROSSLOCALE_VERSION_PTR, CROSSLOCALE_VERSION_LEN));
  exports.Set(Napi::String::New(env, "PROTOCOL_VERSION"),
              Napi::Number::New(env, CROSSLOCALE_PROTOCOL_VERSION));
  exports.Set(Napi::String::New(env, "init_logging"), Napi::Function::New(env, init_logging));
  return NodeBackend::Init(env, exports);
}

NODE_API_MODULE(crosscode_localization_engine, Init)
