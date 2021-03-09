#ifndef _CROSSLOCALE_H
#define _CROSSLOCALE_H

/* Generated with cbindgen:0.18.0 */

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct crosslocale_backend_t crosslocale_backend_t;

typedef uint32_t crosslocale_message_t;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

void crosslocale_init_logging(void);

struct crosslocale_backend_t *crosslocale_backend_new(void);

void crosslocale_backend_free(struct crosslocale_backend_t *myself);

void crosslocale_backend_set_message_callback(struct crosslocale_backend_t *myself,
                                              void (*callback)(void *user_data, crosslocale_message_t message),
                                              void *user_data);

crosslocale_message_t *crosslocale_backend_recv_message(struct crosslocale_backend_t *_myself);

void crosslocale_backend_send_message(struct crosslocale_backend_t *myself, uint32_t message);

#ifdef __cplusplus
} // extern "C"
#endif // __cplusplus

#endif /* _CROSSLOCALE_H */
