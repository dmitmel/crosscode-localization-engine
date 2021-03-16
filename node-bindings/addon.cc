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

class FfiBackend {
public:
  FfiBackend() { throw_ffi_result(crosslocale_backend_new(&this->raw)); }

  ~FfiBackend() {
    if (this->raw != nullptr) {
      throw_ffi_result(crosslocale_backend_free(this->raw));
      this->raw = nullptr;
    }
  }

  void operator=(const FfiBackend&) = delete;
  FfiBackend(const FfiBackend&) = delete;

  std::string recv_message() {
    std::lock_guard<std::mutex> guard(this->recv_mutex);
    uint8_t* message_buf = nullptr;
    size_t message_len = 0;
    size_t message_cap = 0;
    throw_ffi_result(
        crosslocale_backend_recv_message(this->raw, &message_buf, &message_len, &message_cap));
    // Note that this copies the original string.
    std::string str((char*)message_buf, message_len);
    throw_ffi_result(crosslocale_message_free(message_buf, message_len, message_cap));
    return str;
  }

  void send_message(std::string message_str) {
    std::lock_guard<std::mutex> guard(this->send_mutex);
    throw_ffi_result(crosslocale_backend_send_message(this->raw, (uint8_t*)message_str.data(),
                                                      message_str.length()));
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
      : Napi::AsyncWorker(callback), inner(inner), error(CROSSLOCALE_OK) {}

  ~NodeRecvMessageWorker() {}

  void operator=(const NodeRecvMessageWorker&) = delete;
  NodeRecvMessageWorker(const NodeRecvMessageWorker&) = delete;

  void Execute() override {
    try {
      this->message_str = this->inner->recv_message();
    } catch (const FfiBackendException& e) {
      this->error = e;
    }
  }

  std::vector<napi_value> GetResult(Napi::Env env) override {
    if (this->error.is_ok()) {
      return {env.Null(), Napi::String::New(env, this->message_str)};
    } else {
      Napi::Error obj = error.ToNodeError(env);
      return {obj.Value()};
    }
  }

private:
  std::shared_ptr<FfiBackend> inner;
  std::string message_str;
  FfiBackendException error;
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
    if (!(info.Length() == 1 && info[0].IsString())) {
      NAPI_THROW(Napi::TypeError::New(env, "send_message(text: string): void"), Napi::Value());
    }

    Napi::String message = info[0].As<Napi::String>();
    try {
      this->inner->send_message(message.Utf8Value());
    } catch (const FfiBackendException& e) {
      throw e.ToNodeError(env);
    }
    return Napi::Value();
  }

  Napi::Value recv_message(const Napi::CallbackInfo& info) {
    Napi::Env env = info.Env();
    if (!(info.Length() == 1 && info[0].IsFunction())) {
      NAPI_THROW(Napi::TypeError::New(env, "recv_message(callback: Function): void"),
                 Napi::Value());
    }

    Napi::Function callback = info[0].As<Napi::Function>();
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
