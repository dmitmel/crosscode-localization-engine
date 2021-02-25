// TODO: Catch panics, see <https://github.com/hyperium/hyper/blob/4c946af49cc7fbbc6bd4894283a654625c2ea383/src/ffi/macros.rs>.

#[no_mangle]
pub extern "C" fn crosslocale_init_logging() { crate::init_logging(); }

#[no_mangle]
pub extern "C" fn crosslocale_add(a: u32, b: u32) -> u32 { a.wrapping_add(b) }
