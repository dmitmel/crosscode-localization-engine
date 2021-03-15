// This is the version supported by nwjs 0.35.5 which comes with nodejs 11.6.0.
#define NAPI_VERSION 3
#include <napi.h>

#include <crosslocale.h>

const uint32_t SUPPORTED_FFI_BRIDGE_VERSION = 0;

bool check_ffi_result(crosslocale_result_t res, Napi::Env env) {
  if (res == CROSSLOCALE_OK) {
    return true;
  }

  const char *descr = "unkown error";
  if (res == CROSSLOCALE_ERR_GENERIC_RUST_PANIC) {
    descr = "generic Rust panic";
  } else if (res == CROSSLOCALE_ERR_MESSAGE_SENDER_DISCONNECTED) {
    descr = "the message sender channel has disconnected";
  } else if (res == CROSSLOCALE_ERR_MESSAGE_RECEIVER_DISCONNECTED) {
    descr = "the message receiver channel has disconnected";
  } else if (res == CROSSLOCALE_ERR_NON_UTF8_STRING) {
    descr = "a provided string wasn't properly utf8-encoded";
  } else if (res == CROSSLOCALE_ERR_SPAWN_THREAD_FAILED) {
    descr = "failed to spawn the backend thread";
  }

  std::string full_message = std::string("FFI bridge error: ") + descr;
  NAPI_THROW(Napi::Error::New(env, full_message), false);
}

Napi::Value init_logging(const Napi::CallbackInfo &info) {
  Napi::Env env = info.Env();
  crosslocale_result_t res = crosslocale_init_logging();
  check_ffi_result(res, env);
  return Napi::Value();
}

class Backend : public Napi::ObjectWrap<Backend> {
public:
  static Napi::Object Init(Napi::Env env, Napi::Object exports) {
    Napi::Function ctor = DefineClass(env, "Backend",
                                      {
                                          InstanceMethod("send_message", &Backend::send_message),
                                          InstanceMethod("recv_message", &Backend::recv_message),
                                      });

    exports.Set("Backend", ctor);
    return exports;
  }

  Backend(const Napi::CallbackInfo &info) : Napi::ObjectWrap<Backend>(info), raw(nullptr) {
    Napi::Env env = info.Env();
    if (!(info.Length() == 0)) {
      NAPI_THROW_VOID(Napi::TypeError::New(env, "constructor()"));
    }

    crosslocale_result_t res = crosslocale_backend_new(&this->raw);
    check_ffi_result(res, env);
  }

  ~Backend() {
    if (this->raw != nullptr) {
      crosslocale_backend_free(this->raw);
      this->raw = nullptr;
    }
  }

private:
  crosslocale_backend_t *raw;

  Napi::Value send_message(const Napi::CallbackInfo &info) {
    Napi::Env env = info.Env();
    if (!(info.Length() == 1 && info[0].IsString())) {
      NAPI_THROW(Napi::TypeError::New(env, "send_message(text: string): void"), Napi::Value());
    }

    Napi::String message = info[0].As<Napi::String>();
    std::string message_str = message.Utf8Value();
    crosslocale_result_t res = crosslocale_backend_send_message(
        this->raw, (const uint8_t *)message_str.data(), message_str.length());
    check_ffi_result(res, env);
    return Napi::Value();
  }

  Napi::Value recv_message(const Napi::CallbackInfo &info) {
    Napi::Env env = info.Env();
    if (!(info.Length() == 0)) {
      NAPI_THROW(Napi::TypeError::New(env, "recv_message(): string"), Napi::Value());
    }

    uint8_t *message_buf = nullptr;
    size_t message_len = 0;
    size_t message_cap = 0;
    crosslocale_result_t res =
        crosslocale_backend_recv_message(this->raw, &message_buf, &message_len, &message_cap);
    check_ffi_result(res, env);

    // Note that this copies the original string.
    Napi::String message = Napi::String::New(env, (char *)message_buf, message_len);

    crosslocale_message_free(message_buf, message_len, message_cap);

    return message;
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
              Napi::String::New(env, (char *)CROSSLOCALE_VERSION_PTR, CROSSLOCALE_VERSION_LEN));
  exports.Set(Napi::String::New(env, "PROTOCOL_VERSION"),
              Napi::Number::New(env, CROSSLOCALE_PROTOCOL_VERSION));
  exports.Set(Napi::String::New(env, "init_logging"), Napi::Function::New(env, init_logging));
  return Backend::Init(env, exports);
}

NODE_API_MODULE(crosscode_localization_engine, Init)
