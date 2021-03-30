import ctypes
import json
from pprint import pprint

lib = ctypes.CDLL("./target/debug/libcrosslocale.so")


class crosslocale_backend_t(ctypes.Structure):
    pass


crosslocale_result_t = ctypes.c_uint32

lib.crosslocale_message_free.argtypes = [
    ctypes.POINTER(ctypes.c_uint8),
    ctypes.c_size_t,
    ctypes.c_size_t,
]
lib.crosslocale_message_free.restype = crosslocale_result_t

lib.crosslocale_init_logging.argtypes = []
lib.crosslocale_init_logging.restype = crosslocale_result_t

lib.crosslocale_backend_new.argtypes = [ctypes.POINTER(ctypes.POINTER(crosslocale_backend_t))]
lib.crosslocale_backend_new.restype = crosslocale_result_t

lib.crosslocale_backend_recv_message.argtypes = [
    ctypes.POINTER(crosslocale_backend_t),
    ctypes.POINTER(ctypes.POINTER(ctypes.c_uint8)),
    ctypes.POINTER(ctypes.c_size_t),
    ctypes.POINTER(ctypes.c_size_t),
]
lib.crosslocale_backend_recv_message.restype = crosslocale_result_t

lib.crosslocale_backend_send_message.argtypes = [
    ctypes.POINTER(crosslocale_backend_t),
    ctypes.POINTER(ctypes.c_uint8),
    ctypes.c_size_t,
]
lib.crosslocale_backend_send_message.restype = crosslocale_result_t

lib.crosslocale_backend_free.argtypes = [ctypes.POINTER(crosslocale_backend_t)]
lib.crosslocale_backend_free.restype = crosslocale_result_t


CROSSLOCALE_VERSION_PTR = ctypes.POINTER(ctypes.c_uint8).in_dll(lib, "CROSSLOCALE_VERSION_PTR")
CROSSLOCALE_VERSION_LEN = ctypes.c_size_t.in_dll(lib, "CROSSLOCALE_VERSION_LEN")
CROSSLOCALE_VERSION = ctypes.string_at(
    CROSSLOCALE_VERSION_PTR, CROSSLOCALE_VERSION_LEN.value
).decode("utf8")

lib.crosslocale_init_logging()


backend = ctypes.POINTER(crosslocale_backend_t)()
lib.crosslocale_backend_new(ctypes.byref(backend))

for request_index, request in enumerate(
    [
        {"type": "Backend/info"},
        {"type": "Project/open", "dir": "tmp"},
        {"type": "Project/get_meta", "project_id": 1},
        {"type": "Project/list_tr_files", "project_id": 1},
    ]
):
    message_json = {"type": "req", "id": request_index + 1, "data": request}
    pprint(message_json, sort_dicts=False)
    message = json.dumps(message_json)

    message_buf = (ctypes.c_uint8 * len(message))(*(message.encode("utf8")))
    lib.crosslocale_backend_send_message(backend, message_buf, len(message_buf))

    out_message = ctypes.POINTER(ctypes.c_uint8)()
    out_message_len = ctypes.c_size_t()
    out_message_cap = ctypes.c_size_t()
    try:
        lib.crosslocale_backend_recv_message(
            backend,
            ctypes.byref(out_message),
            ctypes.byref(out_message_len),
            ctypes.byref(out_message_cap),
        )
        message = ctypes.string_at(out_message, out_message_len.value).decode("utf8")
    finally:
        lib.crosslocale_message_free(out_message, out_message_len, out_message_cap)

    message_json = json.loads(message)
    pprint(message_json, sort_dicts=False)

lib.crosslocale_backend_free(backend)
