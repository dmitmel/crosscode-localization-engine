import ctypes

lib = ctypes.CDLL("./target/debug/libcrosslocale.so")


class crosslocale_backend_t(ctypes.Structure):
    pass


crosslocale_backend_t_p = ctypes.POINTER(crosslocale_backend_t)

message_callback_t = ctypes.CFUNCTYPE(None, ctypes.c_void_p, ctypes.c_uint32)

lib.crosslocale_init_logging.argtypes = []
lib.crosslocale_init_logging.restype = None

lib.crosslocale_backend_new.argtypes = []
lib.crosslocale_backend_new.restype = crosslocale_backend_t_p

lib.crosslocale_backend_set_message_callback.argtypes = [
    crosslocale_backend_t_p,
    message_callback_t,
    ctypes.c_void_p,
]
lib.crosslocale_backend_set_message_callback.restype = None

lib.crosslocale_backend_send_message.argtypes = [
    crosslocale_backend_t_p,
    ctypes.c_uint32,
]
lib.crosslocale_backend_send_message.restype = None

lib.crosslocale_backend_free.argtypes = [crosslocale_backend_t_p]
lib.crosslocale_backend_free.restype = None


def message_callback(user_data, msg):
    print("recv", msg)
    pass


backend = lib.crosslocale_backend_new()
lib.crosslocale_backend_set_message_callback(
    backend, message_callback_t(message_callback), None
)

for msg in range(16):
    print("send", msg)
    lib.crosslocale_backend_send_message(backend, msg)

lib.crosslocale_backend_free(backend)
