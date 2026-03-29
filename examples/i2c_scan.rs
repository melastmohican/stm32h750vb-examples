//! ## Conditional Multi-Bus I2C Scanner for WeAct MiniSTM32H750VB
//!
//! This example scans I2C1, I2C2, and I2C4, and conditionally scans I2C3.
//!
//! ### The PA8 "Master" Logic:
//! 1. Starts **PA8** as **MCO1** (16MHz Clock) to wake up any camera on I2C1.
//! 2. Scans **I2C1** (PB8/PB9).
//! 3. If a camera/device is found on I2C1, it **keeps** the clock on PA8 and skips I2C3.
//! 4. If nothing is found on I2C1, it **reconfigures** PA8 as I2C3_SCL and scans I2C3.

#![no_main]
#![no_std]

use panic_probe as _;

use core::fmt::Debug;
use cortex_m_rt::entry;
use defmt_rtt as _;
use stm32h7xx_hal::{pac, prelude::*};

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().expect("cannot take core peripherals");
    let dp = pac::Peripherals::take().expect("cannot take peripherals");

    // Configure PWR and RCC
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();
    let rcc = dp.RCC.constrain();

    // Setup clocks: 400MHz System Clock
    let ccdr = rcc.sys_ck(400.MHz()).freeze(pwrcfg, &dp.SYSCFG);

    // Generate 16MHz natively from the robust internal 64MHz HSI (divided by 4)
    // mco1pre: values >= 8 divide the clock. 8 = /2, 9 = /3, 10 = /4.
    // This is REQUIRED to "wake up" the OV2640 camera.
    unsafe {
        (*pac::RCC::ptr())
            .cfgr
            .modify(|_, w| w.mco1pre().bits(10).mco1().hsi());
    }

    defmt::info!("Multi-Bus I2C Scanner (Conditional I2C3)");

    // Initialize Delay for bus settling
    let mut delay = cortex_m::delay::Delay::new(cp.SYST, ccdr.clocks.sysclk().raw());

    // Split GPIO ports once
    let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);
    let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);
    let _gpioc = dp.GPIOC.split(ccdr.peripheral.GPIOC); // Actually, I'll need gpioc for pc9 later
    let gpiod = dp.GPIOD.split(ccdr.peripheral.GPIOD);

    // We need to keep gpioc for potentially scanning I2C3 if I2C1 is empty
    let gpioc = _gpioc;

    // Provide the 16MHz clock to the camera on PA8 (MCO1) to start
    let pa8 = gpioa.pa8.into_alternate::<0>();

    // --- Bus 1 Scan (PB8 / PB9) ---
    defmt::info!("Scanning I2C1 (PB8/PB9) - Camera Port...");
    let scl1 = gpiob
        .pb8
        .into_alternate::<4>()
        .set_open_drain()
        .internal_pull_up(true);
    let sda1 = gpiob
        .pb9
        .into_alternate::<4>()
        .set_open_drain()
        .internal_pull_up(true);
    delay.delay_ms(20u32); // Wait for camera to boot with XCLK
    let mut i2c1 = dp
        .I2C1
        .i2c((scl1, sda1), 100.kHz(), ccdr.peripheral.I2C1, &ccdr.clocks);
    let devices1 = scan_bus(&mut i2c1, "I2C1");

    // --- Conditional I2C3 Scan (PA8 / PC9) ---
    if devices1 == 0 {
        defmt::info!("No Camera on I2C1. Switching PA8 to I2C3...");

        let scl3 = pa8
            .into_alternate::<4>()
            .set_open_drain()
            .internal_pull_up(true);
        let sda3 = gpioc
            .pc9
            .into_alternate::<4>()
            .set_open_drain()
            .internal_pull_up(true);
        delay.delay_ms(10u32);

        let mut i2c3 = dp
            .I2C3
            .i2c((scl3, sda3), 100.kHz(), ccdr.peripheral.I2C3, &ccdr.clocks);
        scan_bus(&mut i2c3, "I2C3");
    } else {
        defmt::info!("Camera Detected on I2C1. Keeping PA8 as XCLK (Skipping I2C3).");
    }

    // --- Bus 2 Scan (PB10 / PB11) ---
    defmt::info!("Scanning I2C2 (PB10/PB11)...");
    let scl2 = gpiob
        .pb10
        .into_alternate::<4>()
        .set_open_drain()
        .internal_pull_up(true);
    let sda2 = gpiob
        .pb11
        .into_alternate::<4>()
        .set_open_drain()
        .internal_pull_up(true);
    delay.delay_ms(10u32);
    let mut i2c2 = dp
        .I2C2
        .i2c((scl2, sda2), 100.kHz(), ccdr.peripheral.I2C2, &ccdr.clocks);
    scan_bus(&mut i2c2, "I2C2");

    // --- Bus 4 Scan (PD12 / PD13) ---
    defmt::info!("Scanning I2C4 (PD12/PD13)...");
    let scl4 = gpiod
        .pd12
        .into_alternate::<4>()
        .set_open_drain()
        .internal_pull_up(true);
    let sda4 = gpiod
        .pd13
        .into_alternate::<4>()
        .set_open_drain()
        .internal_pull_up(true);
    delay.delay_ms(10u32);
    let mut i2c4 = dp
        .I2C4
        .i2c((scl4, sda4), 100.kHz(), ccdr.peripheral.I2C4, &ccdr.clocks);
    scan_bus(&mut i2c4, "I2C4");

    defmt::info!("All scans complete.");

    loop {
        cortex_m::asm::wfi();
    }
}

/// Generic scanner function for any I2C implementation satisfying the embedded-hal 0.2 Read trait.
/// Returns the number of devices found.
fn scan_bus<I2C, E>(i2c: &mut I2C, bus_label: &str) -> usize
where
    I2C: embedded_hal_02::blocking::i2c::Read<Error = E>,
    E: Debug,
{
    let mut devices_found = 0;
    for addr in 1..=127 {
        let mut read_buf = [0u8; 1];
        match i2c.read(addr, &mut read_buf) {
            Ok(_) => {
                defmt::info!("  [{}] Device found at: 0x{:X}", bus_label, addr);
                devices_found += 1;
            }
            Err(_) => {
                // Ignore NACKs (no device)
            }
        }
    }
    if devices_found == 0 {
        defmt::info!("  [{}] No devices found.", bus_label);
    }
    devices_found
}
