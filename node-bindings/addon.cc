#include <napi.h>

#include <crosslocale.h>

// TODO: Check error codes, throw errors as JS exceptions.

void init_logging(const Napi::CallbackInfo &info) { crosslocale_init_logging(); }

class Backend : public Napi::ObjectWrap<Backend> {
public:
  static Napi::Object Init(Napi::Env env, Napi::Object exports) {
    Napi::Function func = DefineClass(env, "Backend",
                                      {
                                          InstanceMethod("send_message", &Backend::send_message),
                                          InstanceMethod("recv_message", &Backend::recv_message),
                                      });

    Napi::FunctionReference *constructor = new Napi::FunctionReference();
    *constructor = Napi::Persistent(func);
    env.SetInstanceData(constructor);

    exports.Set("Backend", func);
    return exports;
  }

  Backend(const Napi::CallbackInfo &info) : Napi::ObjectWrap<Backend>(info), raw(nullptr) {
    Napi::Env env = info.Env();
    if (!(info.Length() == 0)) {
      Napi::TypeError::New(env, "constructor()").ThrowAsJavaScriptException();
      return;
    }

    crosslocale_backend_new(&this->raw);
  }

  ~Backend() {
    if (this->raw != nullptr) {
      crosslocale_backend_free(this->raw);
      this->raw = nullptr;
    }
  }

private:
  Napi::Value send_message(const Napi::CallbackInfo &info) {
    Napi::Env env = info.Env();
    if (!(info.Length() == 1 && info[0].IsString())) {
      Napi::TypeError::New(env, "send_message(message: string): void").ThrowAsJavaScriptException();
      return Napi::Value();
    }

    Napi::String message = info[0].As<Napi::String>();
    std::string message_str = message.Utf8Value();
    crosslocale_backend_send_message(this->raw, (const uint8_t *)message_str.data(),
                                     message_str.length());
    return Napi::Value();
  }

  Napi::Value recv_message(const Napi::CallbackInfo &info) {
    Napi::Env env = info.Env();
    if (!(info.Length() == 0)) {
      Napi::TypeError::New(env, "recv_message(): string").ThrowAsJavaScriptException();
      return Napi::Value();
    }

    uint8_t *message_buf = nullptr;
    size_t message_len = 0;
    size_t message_cap = 0;
    crosslocale_backend_recv_message(this->raw, &message_buf, &message_len, &message_cap);

    // Note that this copies the original string.
    Napi::String message = Napi::String::New(env, (char *)message_buf, message_len);

    crosslocale_message_free(message_buf, message_len, message_cap);

    return message;
  }

  crosslocale_backend_t *raw;
};

Napi::Object Init(Napi::Env env, Napi::Object exports) {
  exports.Set(Napi::String::New(env, "VERSION"),
              Napi::String::New(env, (char *)CROSSLOCALE_VERSION_PTR, CROSSLOCALE_VERSION_LEN));
  exports.Set(Napi::String::New(env, "init_logging"), Napi::Function::New(env, init_logging));
  return Backend::Init(env, exports);
}

NODE_API_MODULE(crosscode_localization_engine, Init)
