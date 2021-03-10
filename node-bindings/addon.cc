#include <crosslocale.h>
#include <napi.h>

Napi::String HelloFunc(const Napi::CallbackInfo &info) {
  Napi::Env env = info.Env();
  return Napi::String::New(env, "world");
}

Napi::Object Init(Napi::Env env, Napi::Object exports) {
  exports.Set(Napi::String::New(env, "version"),
              Napi::String::New(env, (char *)CROSSLOCALE_VERSION_PTR,
                                CROSSLOCALE_VERSION_LEN));
  exports.Set(Napi::String::New(env, "hello"),
              Napi::Function::New(env, HelloFunc));
  return exports;
}

NODE_API_MODULE(crosscode_localization_engine, Init)
