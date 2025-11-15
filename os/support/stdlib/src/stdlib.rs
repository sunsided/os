use crate::syscall::debug_byte;

#[inline(always)]
const fn hex_digit(n: u8) -> u8 {
    if n < 10 { b'0' + n } else { b'A' + (n - 10) }
}

pub fn print_hex(mut x: u64) {
    for _ in 0..16 {
        x = x.rotate_left(4);
        let nib = (x as u8) & 0x0F;
        debug_byte(hex_digit(nib));
    }
}

#[deprecated(note = "Use print_hex instead")]
#[allow(deprecated)]
pub fn print_hex_int80(mut x: u64) {
    for _ in 0..16 {
        x = x.rotate_left(4);
        let nib = (x as u8) & 0x0F;
        crate::syscall::int80::debug_byte_int80(hex_digit(nib));
    }
}
