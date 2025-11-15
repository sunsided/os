#[repr(u64)]
pub enum Sysno {
    /// Write a single byte to a kernel-chosen “debug” sink.
    DebugWriteByte = 1,
    /// Just return a made-up number to prove plumbing.
    Bogus = 2,
}
