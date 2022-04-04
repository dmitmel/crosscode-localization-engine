#ifndef _CROSSLOCALE_H
#define _CROSSLOCALE_H

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#if defined(WIN32) || defined(_WIN32) || defined(__WIN32__)
#  define _CROSSLOCALE_DLLIMPORT __declspec(dllimport)
#else
#  define _CROSSLOCALE_DLLIMPORT
#endif

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

#define extern extern _CROSSLOCALE_DLLIMPORT

typedef enum crosslocale_result {
  CROSSLOCALE_OK = 0,
  CROSSLOCALE_ERR_GENERIC_RUST_PANIC = 1,
  CROSSLOCALE_ERR_BACKEND_DISCONNECTED = 2,
  CROSSLOCALE_ERR_NON_UTF8_STRING = 3,
  CROSSLOCALE_ERR_SPAWN_THREAD_FAILED = 4,
} crosslocale_result;

typedef struct crosslocale_backend crosslocale_backend;

extern const uint32_t CROSSLOCALE_FFI_BRIDGE_VERSION;

extern const uint8_t* CROSSLOCALE_VERSION_PTR;

extern const size_t CROSSLOCALE_VERSION_LEN;

extern const uint8_t* CROSSLOCALE_NICE_VERSION_PTR;

extern const size_t CROSSLOCALE_NICE_VERSION_LEN;

extern const uint32_t CROSSLOCALE_PROTOCOL_VERSION;

_CROSSLOCALE_DLLIMPORT const uint8_t* crosslocale_error_describe(enum crosslocale_result myself);

_CROSSLOCALE_DLLIMPORT const uint8_t* crosslocale_error_id_str(enum crosslocale_result myself);

_CROSSLOCALE_DLLIMPORT
enum crosslocale_result crosslocale_backend_new(struct crosslocale_backend** out);

_CROSSLOCALE_DLLIMPORT
enum crosslocale_result crosslocale_backend_free(struct crosslocale_backend* myself);

_CROSSLOCALE_DLLIMPORT
enum crosslocale_result crosslocale_backend_recv_message(
  const struct crosslocale_backend* myself, uint8_t** out_message, size_t* out_message_len);

_CROSSLOCALE_DLLIMPORT
enum crosslocale_result crosslocale_backend_send_message(
  const struct crosslocale_backend* myself, const uint8_t* message, size_t message_len);

_CROSSLOCALE_DLLIMPORT enum crosslocale_result crosslocale_message_free(uint8_t* ptr, size_t len);

_CROSSLOCALE_DLLIMPORT
enum crosslocale_result crosslocale_backend_close(struct crosslocale_backend* myself);

_CROSSLOCALE_DLLIMPORT
enum crosslocale_result crosslocale_backend_is_closed(
  struct crosslocale_backend* myself, bool* out);

#undef extern

#ifdef __cplusplus
} // extern "C"
#endif // __cplusplus

#endif // _CROSSLOCALE_H
