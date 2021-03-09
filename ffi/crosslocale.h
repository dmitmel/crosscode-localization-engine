#ifndef _CROSSLOCALE_H
#define _CROSSLOCALE_H

/* Generated with cbindgen:0.18.0 */

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct crosslocale_backend_t crosslocale_backend_t;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

void crosslocale_init_logging(void);

void crosslocale_message_free(uint8_t *buf, size_t len, size_t cap);

struct crosslocale_backend_t *crosslocale_backend_new(void);

void crosslocale_backend_free(struct crosslocale_backend_t *myself);

void crosslocale_backend_set_message_callback(struct crosslocale_backend_t *myself,
                                              void (*callback)(void *user_data, uint8_t *message, size_t message_len, size_t message_cap),
                                              void *user_data);

void crosslocale_backend_recv_message(struct crosslocale_backend_t *myself,
                                      uint8_t **out_message,
                                      size_t *out_message_len,
                                      size_t *out_message_cap);

void crosslocale_backend_send_message(struct crosslocale_backend_t *myself,
                                      const uint8_t *message,
                                      size_t message_len);

#ifdef __cplusplus
} // extern "C"
#endif // __cplusplus

#endif /* _CROSSLOCALE_H */
