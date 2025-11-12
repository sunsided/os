use kernel_qemu::qemu_trace;
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};

pub struct UefiLogger {
    max_level: LevelFilter,
    boot_services_available: bool,
}

impl UefiLogger {
    #[must_use]
    pub const fn new(max_level: LevelFilter) -> Self {
        Self {
            max_level,
            boot_services_available: true,
        }
    }

    /// Call this once during early init.
    #[allow(
        static_mut_refs,
        clippy::missing_errors_doc,
        clippy::missing_panics_doc
    )]
    pub fn init(self) -> Result<&'static mut Self, SetLoggerError> {
        // SAFETY: log::set_logger expects &'static Log. Use a leaked boxed (or a static) in kernels.
        // For no-alloc, we'll use a static.
        static mut LOGGER: Option<UefiLogger> = None;

        // move self into static
        unsafe {
            LOGGER = Some(self);
            // set_logger requires &'static dyn Log
            log::set_logger(LOGGER.as_ref().unwrap() as &'static dyn Log)?;
        }
        log::set_max_level(LevelFilter::Trace);
        unsafe { Ok(LOGGER.as_mut().expect("initialized")) }
    }

    pub const fn exit_boot_services(&mut self) {
        self.boot_services_available = false;
    }
}

impl Log for UefiLogger {
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

        // Mirror to UEFI console as long as possible.
        if self.boot_services_available {
            uefi::println!(
                "[{}] {}: {}",
                record.level(),
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {
        // no-op for qemu debug port
    }
}
