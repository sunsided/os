pub mod entry;

use crate::ports::outb;
use stdlib::syscall_abi::Sysno;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SyscallSource {
    Syscall,
    Int80h,
}

#[allow(clippy::no_effect_underscore_binding, clippy::too_many_arguments)]
pub fn syscall(
    sysno: u64,
    arg0: u64,
    _arg1: u64,
    _arg2: u64,
    _arg3: u64,
    _arg4: u64,
    _arg5: u64,
    source: SyscallSource,
) -> u64 {
    match sysno {
        x if x == Sysno::DebugWriteByte as u64 => {
            unsafe {
                let byte = (arg0 & 0xFF) as u8;
                outb(0x402, byte);
            }
            0
        }
        x if x == Sysno::Bogus as u64 => match source {
            SyscallSource::Int80h => 0xd34d_c0d3,
            SyscallSource::Syscall => 0xb007_c4fe,
        },

        _ => u64::MAX,
    }
}
