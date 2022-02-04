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

typedef enum crosslocale_message_type {
  CROSSLOCALE_MESSAGE_NIL = 0,
  CROSSLOCALE_MESSAGE_BOOL = 1,
  CROSSLOCALE_MESSAGE_I64 = 2,
  CROSSLOCALE_MESSAGE_F64 = 3,
  CROSSLOCALE_MESSAGE_STR = 4,
  CROSSLOCALE_MESSAGE_LIST = 5,
  CROSSLOCALE_MESSAGE_DICT = 6,
  CROSSLOCALE_MESSAGE_INVALID = -1,
} crosslocale_message_type;

typedef enum crosslocale_result {
  CROSSLOCALE_OK = 0,
  CROSSLOCALE_ERR_GENERIC_RUST_PANIC = 1,
  CROSSLOCALE_ERR_BACKEND_DISCONNECTED = 2,
  CROSSLOCALE_ERR_SPAWN_THREAD_FAILED = 4,
} crosslocale_result;

typedef struct crosslocale_backend crosslocale_backend;

typedef struct crosslocale_message_str {
  size_t len;
  uint8_t* ptr;
} crosslocale_message_str;

typedef struct crosslocale_message_list {
  size_t len;
  struct crosslocale_message* ptr;
} crosslocale_message_list;

typedef struct crosslocale_message_dict {
  size_t len;
  struct crosslocale_message_str* keys;
  struct crosslocale_message* values;
} crosslocale_message_dict;

typedef union crosslocale_message_inner {
  bool value_bool;
  int64_t value_i64;
  double value_f64;
  struct crosslocale_message_str value_str;
  struct crosslocale_message_list value_list;
  struct crosslocale_message_dict value_dict;
} crosslocale_message_inner;

typedef struct crosslocale_message {
  enum crosslocale_message_type type;
  union crosslocale_message_inner as;
} crosslocale_message;

extern const uint32_t CROSSLOCALE_FFI_BRIDGE_VERSION;

extern const uint8_t* CROSSLOCALE_VERSION_PTR;

extern const size_t CROSSLOCALE_VERSION_LEN;

extern const uint8_t* CROSSLOCALE_NICE_VERSION_PTR;

extern const size_t CROSSLOCALE_NICE_VERSION_LEN;

extern const uint32_t CROSSLOCALE_PROTOCOL_VERSION;

_CROSSLOCALE_DLLIMPORT enum crosslocale_result crosslocale_init_logging(void);

_CROSSLOCALE_DLLIMPORT const uint8_t* crosslocale_error_description(enum crosslocale_result myself);

_CROSSLOCALE_DLLIMPORT const uint8_t* crosslocale_error_id_str(enum crosslocale_result myself);

_CROSSLOCALE_DLLIMPORT
enum crosslocale_result crosslocale_message_free(struct crosslocale_message* myself);

_CROSSLOCALE_DLLIMPORT
enum crosslocale_result crosslocale_backend_new(struct crosslocale_backend** out);

_CROSSLOCALE_DLLIMPORT
enum crosslocale_result crosslocale_backend_free(struct crosslocale_backend* myself);

_CROSSLOCALE_DLLIMPORT
enum crosslocale_result crosslocale_backend_recv_message(
  const struct crosslocale_backend* myself, struct crosslocale_message* out_message);

_CROSSLOCALE_DLLIMPORT
enum crosslocale_result crosslocale_backend_send_message(
  const struct crosslocale_backend* myself, const struct crosslocale_message* message);

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
