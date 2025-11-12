use crate::qemu_trace;
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};

pub struct QemuLogger {
    max_level: LevelFilter,
}

impl QemuLogger {
    #[must_use]
    pub const fn new(max_level: LevelFilter) -> Self {
        Self { max_level }
    }

    /// Call this once during early init.
    #[allow(
        static_mut_refs,
        clippy::missing_errors_doc,
        clippy::missing_panics_doc
    )]
    pub fn init(self) -> Result<(), SetLoggerError> {
        // SAFETY: log::set_logger expects &'static Log. Use a leaked boxed (or a static) in kernels.
        // For no-alloc, we'll use a static.
        static mut LOGGER: Option<QemuLogger> = None;

        // move self into static
        unsafe {
            LOGGER = Some(self);
            // set_logger requires &'static dyn Log
            log::set_logger(LOGGER.as_ref().unwrap() as &'static dyn Log)?;
        }
        log::set_max_level(LevelFilter::Trace);
        Ok(())
    }
}

impl Log for QemuLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.max_level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        // Format: "[LEVEL] target: message\n"
        // Keep allocations out — format directly into qemu_trace!
        // qemu_trace! is assumed to accept format! style args.
        qemu_trace!(
            "[{}] {}: {}\n",
            record.level(),
            record.target(),
            record.args()
        );
    }

    fn flush(&self) {
        // no-op for qemu debug port
    }
}
