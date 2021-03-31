#ifndef _CROSSLOCALE_H
#define _CROSSLOCALE_H

/* Generated with cbindgen:0.18.0 */

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#if defined(WIN32) || defined(_WIN32) || defined(__WIN32__)
#define _CROSSLOCALE_DLLIMPORT __declspec(dllimport)
#else
#define _CROSSLOCALE_DLLIMPORT
#endif

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

#define extern extern _CROSSLOCALE_DLLIMPORT

typedef struct crosslocale_backend_t crosslocale_backend_t;

typedef uint32_t crosslocale_result_t;

extern const uint32_t CROSSLOCALE_FFI_BRIDGE_VERSION;

extern const uint8_t* CROSSLOCALE_VERSION_PTR;

extern const size_t CROSSLOCALE_VERSION_LEN;

extern const uint32_t CROSSLOCALE_PROTOCOL_VERSION;

extern const crosslocale_result_t CROSSLOCALE_OK;

extern const crosslocale_result_t CROSSLOCALE_ERR_GENERIC_RUST_PANIC;

extern const crosslocale_result_t CROSSLOCALE_ERR_BACKEND_DISCONNECTED;

extern const crosslocale_result_t CROSSLOCALE_ERR_NON_UTF8_STRING;

extern const crosslocale_result_t CROSSLOCALE_ERR_SPAWN_THREAD_FAILED;

_CROSSLOCALE_DLLIMPORT crosslocale_result_t crosslocale_init_logging(void);

_CROSSLOCALE_DLLIMPORT
crosslocale_result_t crosslocale_message_free(uint8_t* buf, size_t len, size_t cap);

_CROSSLOCALE_DLLIMPORT
crosslocale_result_t crosslocale_backend_new(struct crosslocale_backend_t** out);

_CROSSLOCALE_DLLIMPORT
crosslocale_result_t crosslocale_backend_free(struct crosslocale_backend_t* myself);

_CROSSLOCALE_DLLIMPORT
crosslocale_result_t crosslocale_backend_recv_message(const struct crosslocale_backend_t* myself,
                                                      uint8_t** out_message,
                                                      size_t* out_message_len,
                                                      size_t* out_message_cap);

_CROSSLOCALE_DLLIMPORT
crosslocale_result_t crosslocale_backend_send_message(const struct crosslocale_backend_t* myself,
                                                      const uint8_t* message, size_t message_len);

_CROSSLOCALE_DLLIMPORT
crosslocale_result_t crosslocale_backend_close(struct crosslocale_backend_t* myself);

_CROSSLOCALE_DLLIMPORT
crosslocale_result_t crosslocale_backend_is_closed(struct crosslocale_backend_t* myself, bool* out);

#undef extern

#ifdef __cplusplus
} // extern "C"
#endif // __cplusplus

#endif // _CROSSLOCALE_H
