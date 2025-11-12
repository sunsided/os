/// Hardware **Interrupt Stack Table (IST)** selector.
///
/// x86-64 TSS defines up to seven IST pointers (`IST[0]..IST[6]`).
/// Each IDT gate may specify `.ist(n)` (1–7) to make the CPU load
/// `TSS.IST[n-1]` as the new stack pointer when that interrupt fires.
///
/// Index 0 in the *IDT gate* means “no IST” (use the current stack).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Ist {
    /// No alternate stack (IDT `.ist(0)`).
    None = 0,

    /// Hardware IST#1 → `TSS.IST[0]`.
    Ist1 = 1,
    /// Hardware IST#2 → `TSS.IST[1]`.
    Ist2 = 2,
    /// Hardware IST#3 → `TSS.IST[2]`.
    Ist3 = 3,
    /// Hardware IST#4 → `TSS.IST[3]`.
    Ist4 = 4,
    /// Hardware IST#5 → `TSS.IST[4]`.
    Ist5 = 5,
    /// Hardware IST#6 → `TSS.IST[5]`.
    Ist6 = 6,
    /// Hardware IST#7 → `TSS.IST[6]`.
    Ist7 = 7,
}

impl Ist {
    #[inline]
    pub const fn from_bits(value: u8) -> Self {
        match value {
            1 => Self::Ist1,
            2 => Self::Ist2,
            3 => Self::Ist3,
            4 => Self::Ist4,
            5 => Self::Ist5,
            6 => Self::Ist6,
            7 => Self::Ist7,
            _ => Self::None,
        }
    }

    #[inline]
    pub const fn into_bits(self) -> u8 {
        self as u8
    }

    /// Returns the 0-based array index into `TSS.ist[]`.
    ///
    /// Returns `None` if this variant is [`Ist::None`].
    #[inline]
    pub const fn tss_index(self) -> Option<usize> {
        match self {
            Self::None => None,
            Self::Ist1 => Some(0),
            Self::Ist2 => Some(1),
            Self::Ist3 => Some(2),
            Self::Ist4 => Some(3),
            Self::Ist5 => Some(4),
            Self::Ist6 => Some(5),
            Self::Ist7 => Some(6),
        }
    }

    /// Return the raw numeric value to write into an IDT gate’s `.ist()` field.
    #[inline]
    pub const fn gate_index(self) -> u8 {
        self.into_bits()
    }
}
