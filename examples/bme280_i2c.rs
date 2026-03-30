//! BME280 Temperature/Humidity/Pressure Sensor Example for WeAct MiniSTM32H750VB
//!
//! This example performs an automatic I2C bus scan and then attempts to initialize
//! the BME280 sensor using detected addresses (0x77 or 0x76).
//!
//! # Hardware
//! - **MCU:** WeAct MiniSTM32H750VB
//! - **Sensor:** BME280 (I2C Address: 0x77 or 0x76)
//!
//! # Wiring (I2C2)
//! - **GND -> GND**
//! - **3.3V -> 3.3V**
//! - **SCL -> PB10**
//! - **SDA -> PB11**
//!
//! Run with `cargo run --example bme280_i2c`.

#![no_std]
#![no_main]

use bme280::i2c::BME280;
use core::cell::RefCell;
use cortex_m_rt::entry;
use defmt::{error, info, warn};
use defmt_rtt as _;
use embedded_hal::delay::DelayNs;
use embedded_hal::i2c::I2c;
use embedded_hal_bus::i2c::RefCellDevice;
use panic_probe as _;
use stm32h750vb_examples::compat::{DelayEh1, I2cEh1};
use stm32h7xx_hal::{pac, prelude::*};

#[entry]
fn main() -> ! {
    info!("Initializing peripherals...");
    let cp = pac::CorePeripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    // --- Configure PWR and RCC ---
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();
    let rcc = dp.RCC.constrain();
    let ccdr = rcc.sys_ck(400.MHz()).freeze(pwrcfg, &dp.SYSCFG);

    // --- Initialize Delay and Wrap for EH 1.0 ---
    let delay = cp.SYST.delay(ccdr.clocks);
    let mut delay_eh1 = DelayEh1(delay);

    // --- GPIO Configuration ---
    let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);

    // --- I2C2 Initialization (PB10: SCL, PB11: SDA) ---
    info!("Initializing I2C2 (PB10: SCL, PB11: SDA)");
    // Added internal pull-ups to help with signal stability if external ones are missing/weak
    let scl = gpiob
        .pb10
        .into_alternate::<4>()
        .set_open_drain()
        .internal_pull_up(true);
    let sda = gpiob
        .pb11
        .into_alternate::<4>()
        .set_open_drain()
        .internal_pull_up(true);

    let i2c = dp.I2C2.i2c(
        (scl, sda),
        100.kHz(),
        ccdr.peripheral.I2C2,
        &ccdr.clocks,
    );

    // Wrap I2C for EH 1.0 and share via RefCell
    let i2c_bus = RefCell::new(I2cEh1(i2c));

    // --- 1. I2C Bus Scanner ---
    info!("--- I2C Bus Scan (I2C2) ---");
    let mut detected_addresses = 0;
    for addr in 1..=127 {
        let mut i2c_dev = RefCellDevice::new(&i2c_bus);
        // We use a dummy read to check for ACK
        let mut byte = [0u8; 1];
        if i2c_dev.read(addr, &mut byte).is_ok() {
            info!("Found device at address: 0x{:02x}", addr);
            detected_addresses += 1;
        }
    }
    if detected_addresses == 0 {
        warn!("No I2C devices detected on I2C2. Check wiring and power!");
    } else {
        info!("Scan complete. Found {} device(s).", detected_addresses);
    }

    // --- 2. BME280 Initialization with Fallback ---
    info!("Initializing BME280...");
    
    // Try 0x77 first (Standard Adafruit/WeAct address)
    let mut bme280 = BME280::new_secondary(RefCellDevice::new(&i2c_bus));
    let mut initialized = false;

    match bme280.init(&mut delay_eh1) {
        Ok(_) => {
            info!("BME280 initialized at 0x77.");
            initialized = true;
        }
        Err(e) => {
            warn!("BME280 not found at 0x77: {:?}", defmt::Debug2Format(&e));
            info!("Trying fallback address 0x76...");
            
            // Try 0x76 (SDO connected to GND)
            let mut bme280_alt = BME280::new_primary(RefCellDevice::new(&i2c_bus));
            match bme280_alt.init(&mut delay_eh1) {
                Ok(_) => {
                    info!("BME280 initialized at 0x76.");
                    bme280 = bme280_alt; // Switch to the successful instance
                    initialized = true;
                }
                Err(e2) => {
                    error!("BME280 not found at 0x76: {:?}", defmt::Debug2Format(&e2));
                }
            }
        }
    }

    if !initialized {
        error!("CRITICAL: BME280 sensor not found on either address. Stopping.");
        loop {
            cortex_m::asm::wfi();
        }
    }

    info!("--- Starting BME280 Measurement Loop ---");

    loop {
        match bme280.measure(&mut delay_eh1) {
            Ok(m) => {
                info!(
                    "Temp: {} C, Hum: {} %, Pres: {} hPa",
                    m.temperature,
                    m.humidity,
                    m.pressure / 100.0
                );
            }
            Err(e) => {
                error!("Failed to read from BME280 sensor: {:?}", defmt::Debug2Format(&e));
            }
        }

        delay_eh1.delay_ms(1000);
    }
}
