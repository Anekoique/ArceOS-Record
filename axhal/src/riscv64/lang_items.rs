use axlog::error;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    error!("{}", _info);
    super::misc::terminate()
}
