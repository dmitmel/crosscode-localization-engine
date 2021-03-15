#ifndef _CROSSLOCALE_H
#define _CROSSLOCALE_H

/* Generated with cbindgen:0.18.0 */

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct crosslocale_backend_t crosslocale_backend_t;

typedef uint32_t crosslocale_result_t;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

extern const uint32_t CROSSLOCALE_FFI_BRIDGE_VERSION;

extern const uint8_t *CROSSLOCALE_VERSION_PTR;

extern const size_t CROSSLOCALE_VERSION_LEN;

extern const uint32_t CROSSLOCALE_PROTOCOL_VERSION;

extern const crosslocale_result_t CROSSLOCALE_OK;

extern const crosslocale_result_t CROSSLOCALE_ERR_GENERIC_RUST_PANIC;

extern const crosslocale_result_t CROSSLOCALE_ERR_BACKEND_DISCONNECTED;

extern const crosslocale_result_t CROSSLOCALE_ERR_NON_UTF8_STRING;

extern const crosslocale_result_t CROSSLOCALE_ERR_SPAWN_THREAD_FAILED;

crosslocale_result_t crosslocale_init_logging(void);

crosslocale_result_t crosslocale_message_free(uint8_t *buf, size_t len, size_t cap);

crosslocale_result_t crosslocale_backend_new(struct crosslocale_backend_t **out);

crosslocale_result_t crosslocale_backend_free(struct crosslocale_backend_t *myself);

crosslocale_result_t crosslocale_backend_recv_message(struct crosslocale_backend_t *myself,
                                                      uint8_t **out_message,
                                                      size_t *out_message_len,
                                                      size_t *out_message_cap);

crosslocale_result_t crosslocale_backend_send_message(struct crosslocale_backend_t *myself,
                                                      const uint8_t *message,
                                                      size_t message_len);

#ifdef __cplusplus
} // extern "C"
#endif // __cplusplus

#endif /* _CROSSLOCALE_H */
