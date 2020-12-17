pub mod address;
pub mod interrupt;

use address::GenericAddress;
use bit_field::BitField;
pub use interrupt::{
    Apic,
    InterruptModel,
    InterruptSourceOverride,
    IoApic,
    LocalInterruptLine,
    NmiLine,
    NmiProcessor,
    NmiSource,
    Polarity,
    TriggerMode,
};

use crate::{fadt::Fadt, madt::Madt, AcpiError, AcpiHandler, AcpiTables, PowerProfile};
use alloc::vec::Vec;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessorState {
    /// A processor in this state is unusable, and you must not attempt to bring it up.
    Disabled,

    /// A processor waiting for a SIPI (Startup Inter-processor Interrupt) is currently not active,
    /// but may be brought up.
    WaitingForSipi,

    /// A Running processor is currently brought up and running code.
    Running,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Processor {
    pub processor_uid: u8,
    pub local_apic_id: u8,

    /// The state of this processor. Always check that the processor is not `Disabled` before
    /// attempting to bring it up!
    pub state: ProcessorState,

    /// Whether this processor is the Bootstrap Processor (BSP), or an Application Processor (AP).
    /// When the bootloader is entered, the BSP is the only processor running code. To run code on
    /// more than one processor, you need to "bring up" the APs.
    pub is_ap: bool,
}

pub struct ProcessorInfo {
    pub boot_processor: Processor,
    /// Application processors should be brought up in the order they're defined in this list.
    pub application_processors: Vec<Processor>,
}

/// Information about the ACPI Power Management Timer (ACPI PM Timer).
pub struct PmTimer {
    /// A generic address to the register block of ACPI PM Timer.
    pub base: GenericAddress,
    /// This field is true if the hardware supports 32-bit timer, and false if the hardware
    /// supports 24-bit timer.
    pub supports_32bit: bool,
}
impl PmTimer {
    /// Creates a new instance of `PmTimer`.
    pub fn new(fadt: &Fadt) -> Result<Option<PmTimer>, AcpiError> {
        let base = fadt.pm_timer_block()?;
        let flags = fadt.flags();

        match base {
            Some(base) => Ok(Some(PmTimer { base, supports_32bit: flags.get_bit(8) })),
            None => Ok(None),
        }
    }
}

/// `PlatformInfo` allows the collection of some basic information about the platform from some of the fixed-size
/// tables in a nice way. It requires access to the `FADT` and `MADT`. It is the easiest way to get information
/// about the processors and interrupt controllers on a platform.
pub struct PlatformInfo {
    pub power_profile: PowerProfile,
    pub interrupt_model: InterruptModel,
    /// On `x86_64` platforms that support the APIC, the processor topology must also be inferred from the
    /// interrupt model. That information is stored here, if present.
    pub processor_info: Option<ProcessorInfo>,
    pub pm_timer: Option<PmTimer>,
    /*
     * TODO: we could provide a nice view of the hardware register blocks in the FADT here.
     */
}

impl PlatformInfo {
    pub fn new<H>(tables: &AcpiTables<H>) -> Result<PlatformInfo, AcpiError>
    where
        H: AcpiHandler,
    {
        let fadt = unsafe {
            tables
                .get_sdt::<Fadt>(crate::sdt::Signature::FADT)?
                .ok_or(AcpiError::TableMissing(crate::sdt::Signature::FADT))?
        };
        let power_profile = fadt.power_profile();

        let madt = unsafe { tables.get_sdt::<Madt>(crate::sdt::Signature::MADT)? };
        let (interrupt_model, processor_info) = match madt {
            Some(madt) => madt.parse_interrupt_model()?,
            None => (InterruptModel::Unknown, None),
        };
        let pm_timer = PmTimer::new(&fadt)?;

        Ok(PlatformInfo { power_profile, interrupt_model, processor_info, pm_timer })
    }
}
