//! ## SDMMC Example for WeAct MiniSTM32H750VB (Integrated Adapter)
//!
//! This example demonstrates how to use the SDMMC1 peripheral to interface with a MicroSD card.
//! It uses the HAL's integrated `sdmmc_block_device()` adapter to connect to the
//! `embedded-sdmmc` crate.
//!
//! ### Wiring (WeAct MiniSTM32H750VB onboard MicroSD):
//! | Signal | STM32 Pin | Function |
//! | :--- | :--- | :--- |
//! | **D0** | PC8 | Data 0 (AF12) |
//! | **D1** | PC9 | Data 1 (AF12) |
//! | **D2** | PC10 | Data 2 (AF12) |
//! | **D3** | PC11 | Data 3 (AF12) |
//! | **CMD** | PD2 | Command (AF12) |
//! | **CK** | PC12 | Clock (AF12) |

#![no_main]
#![no_std]

use panic_probe as _;

use cortex_m_rt::entry;
use defmt_rtt as _;
use stm32h7xx_hal::{
    pac,
    prelude::*,
    sdmmc::{SdCard, Sdmmc},
};

use embedded_sdmmc::{Controller, Mode, VolumeIdx};

/// Dummy time source for embedded-sdmmc
struct DummyTimeSource;
impl embedded_sdmmc::TimeSource for DummyTimeSource {
    fn get_timestamp(&self) -> embedded_sdmmc::Timestamp {
        embedded_sdmmc::Timestamp {
            year_since_1970: 54, // 2024
            zero_indexed_month: 2,
            zero_indexed_day: 28,
            hours: 12,
            minutes: 0,
            seconds: 0,
        }
    }
}

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    // Configure PWR and RCC
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();
    let rcc = dp.RCC.constrain();
    let ccdr = rcc
        .sys_ck(400.MHz())
        .pll1_q_ck(100.MHz()) // SDMMC clock source
        .freeze(pwrcfg, &dp.SYSCFG);

    defmt::info!("Integrated SDMMC Example Started");

    let gpioc = dp.GPIOC.split(ccdr.peripheral.GPIOC);
    let gpiod = dp.GPIOD.split(ccdr.peripheral.GPIOD);

    // Get the delay provider
    let mut delay = cp.SYST.delay(ccdr.clocks);

    // Configure SDMMC1 pins (AF12) with VeryHigh speed and internal pull-ups
    // Matches the official HAL best practice for SDMMC pins.
    let clk = gpioc
        .pc12
        .into_alternate::<12>()
        .speed(stm32h7xx_hal::gpio::Speed::VeryHigh);
    let cmd = gpiod
        .pd2
        .into_alternate::<12>()
        .internal_pull_up(true)
        .speed(stm32h7xx_hal::gpio::Speed::VeryHigh);
    let d0 = gpioc
        .pc8
        .into_alternate::<12>()
        .internal_pull_up(true)
        .speed(stm32h7xx_hal::gpio::Speed::VeryHigh);
    let d1 = gpioc
        .pc9
        .into_alternate::<12>()
        .internal_pull_up(true)
        .speed(stm32h7xx_hal::gpio::Speed::VeryHigh);
    let d2 = gpioc
        .pc10
        .into_alternate::<12>()
        .internal_pull_up(true)
        .speed(stm32h7xx_hal::gpio::Speed::VeryHigh);
    let d3 = gpioc
        .pc11
        .into_alternate::<12>()
        .internal_pull_up(true)
        .speed(stm32h7xx_hal::gpio::Speed::VeryHigh);

    defmt::info!("Initializing SDMMC1 peripheral...");

    let mut sdmmc: Sdmmc<pac::SDMMC1, SdCard> = dp.SDMMC1.sdmmc(
        (clk, cmd, d0, d1, d2, d3),
        ccdr.peripheral.SDMMC1,
        &ccdr.clocks,
    );

    // Initial bus frequency for card identification
    let bus_frequency = 400.kHz();
    loop {
        match sdmmc.init(bus_frequency) {
            Ok(_) => break,
            Err(e) => {
                defmt::info!("Init error: {:?}", defmt::Debug2Format(&e));
                delay.delay_ms(1000u32);
            }
        }
        defmt::info!("Waiting for card...");
    }

    defmt::info!(
        "Card initialized! Capacity: {} bytes",
        sdmmc.card().unwrap().size()
    );

    // --- Raw Block Test ---
    defmt::info!("Running Raw Block Test...");
    let test_block = 1000;
    let write_data = [0xA5; 512];
    sdmmc
        .write_block(test_block, &write_data)
        .expect("Block write failed");
    let mut read_data = [0u8; 512];
    sdmmc
        .read_block(test_block, &mut read_data)
        .expect("Block read failed");
    assert_eq!(read_data, write_data);
    defmt::info!("Raw Block Test Passed!");

    // --- Switch to Integrated Filesystem Adapter ---
    defmt::info!("Initializing Controller with integrated adapter...");

    // Convert to block device adapter (consumes raw sdmmc)
    // In HAL 0.16.0, this satisfies embedded-sdmmc 0.5.x traits.
    let mut controller = Controller::new(sdmmc.sdmmc_block_device(), DummyTimeSource);

    defmt::info!("Mounting Volume 0...");
    let mut volume = controller
        .get_volume(VolumeIdx(0))
        .expect("Failed to get volume");
    let root_dir = controller
        .open_root_dir(&volume)
        .expect("Failed to open root dir");

    defmt::info!("Listing root directory:");
    controller
        .iterate_dir(&volume, &root_dir, |entry| {
            defmt::info!(
                "  {}: {} ({} bytes)",
                defmt::Debug2Format(&entry.name),
                if entry.attributes.is_directory() {
                    "DIR"
                } else {
                    "FILE"
                },
                entry.size
            );
        })
        .expect("Failed to iterate dir");

    defmt::info!("Creating /HELLO.TXT...");
    let mut file = controller
        .open_file_in_dir(
            &mut volume,
            &root_dir,
            "HELLO.TXT",
            Mode::ReadWriteCreateOrTruncate,
        )
        .expect("Failed to open file");

    let data = b"Hello from WeAct H750VB Rust SDMMC Example using integrated HAL adapter!\n";
    controller
        .write(&mut volume, &mut file, data)
        .expect("File write failed");
    controller
        .close_file(&volume, file)
        .expect("Failed to close file");

    defmt::info!("Integrated SDMMC Test Complete Success!");

    loop {
        cortex_m::asm::nop();
    }
}
