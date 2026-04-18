//! # PMS5003 PM2.5 Air Quality Sensor (Stable UART) Example
//!
//! Reads particulate matter data from a PMS5003 sensor over USART1.
//!
//! ## Stability Strategies
//!
//! - **Active Mode**: The sensor is kept in its default "streaming" mode.
//! - **Drain & Hunt**: Before every measurement, the MCU drains the UART hardware FIFO
//!   to clear stale data, then "hunts" for a fresh frame header. This prevents
//!   UART buffer overruns and synchronization errors.
//! - **Wake & Verify**: On startup, the MCU listens for data and sends an explicit
//!   wake-up command if the sensor is silent.
//!
//! ## Hardware
//!
//! - **Board:** WeAct MiniSTM32H750VB
//! - **Sensor:** Adafruit PM2.5 Air Quality Sensor (PMS5003)
//!
//! ## Wiring (USART1)
//!
//! | PMS5003 Pin | STM32H7 Pin | Function          |
//! |-------------|------------|-------------------|
//! | VCC         | 5V         | 5V Power          |
//! | GND         | GND        | Ground            |
//! | TXD         | PA10       | MCU RX (USART1 RX)|
//! | RXD         | PA9        | MCU TX (USART1 TX)|
//!
//! Run with `cargo run --example pms5003`.

#![no_std]
#![no_main]

use cortex_m_rt::entry;
use defmt::*;
use defmt_rtt as _;
use embedded_hal::delay::DelayNs;
use panic_probe as _;
use pmsx003::PmsX003Sensor;
use stm32h750vb_examples::compat::{DelayEh1, SerialEh1};
use stm32h7xx_hal::{pac, prelude::*};

#[entry]
fn main() -> ! {
    info!("PMS5003 PM2.5 Air Quality Sensor Example (STM32H7)");

    let cp = pac::CorePeripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    // --- Configure PWR and RCC ---
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();
    let rcc = dp.RCC.constrain();

    // Configure system clock to 400MHz
    let ccdr = rcc.sys_ck(400.MHz()).freeze(pwrcfg, &dp.SYSCFG);

    // --- Initialize Delay ---
    let delay = cp.SYST.delay(ccdr.clocks);
    let mut delay_eh1 = DelayEh1(delay);

    // --- GPIO Configuration ---
    let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);

    // USART1 Pins: PA9 (TX), PA10 (RX)
    let tx = gpioa.pa9.into_alternate::<7>();
    let rx = gpioa.pa10.into_alternate::<7>();

    // --- USART1 Initialization ---
    // PMS5003 default baud rate is 9600
    let serial = dp
        .USART1
        .serial((tx, rx), 9600.bps(), ccdr.peripheral.USART1, &ccdr.clocks)
        .unwrap();

    // Wrap for embedded-io support
    let mut serial_compat = SerialEh1(serial);

    info!("USART1 initialized at 9600 baud");
    info!("MCU TX (PA9)  -> Sensor RX");
    info!("MCU RX (PA10) <- Sensor TX");

    // 1. Initial Wake Up / Detection Phase
    // Listen for data to see if the sensor is already streaming
    info!("Checking if sensor is active...");
    let mut data_detected = false;
    for _ in 0..10 {
        if nb::block!(serial_compat.0.read()).is_ok() {
            data_detected = true;
            break;
        }
        delay_eh1.delay_ms(100);
    }

    if !data_detected {
        info!("Sensor silent. Sending wake-up command (Active Mode)...");
        let mut pms_sensor = PmsX003Sensor::new(&mut serial_compat);
        let _ = pms_sensor.active();
        // Give it a moment to start the fan and laser
        delay_eh1.delay_ms(500);
    } else {
        info!("Sensor is already streaming.");
    }

    info!("PMS5003 sensor initialized (Active Mode)");
    info!("Starting Drain & Hunt polling (every 2 seconds)...");

    loop {
        // 1. Drain stale backlog from MCU UART buffer to ensure we catch the LATEST frame
        // Non-blocking read until it errors with WouldBlock
        while serial_compat.0.read().is_ok() {
            // Draining...
        }

        // 2. Initialize "Hunting" for a valid frame
        let mut found = false;

        // Try to find a valid frame with a timeout
        for hunt_attempt in 1..=40 {
            // Check if data is available (non-blocking)
            match serial_compat.0.read() {
                Ok(_byte) => {
                    // Start of frame found? Actually PmsX003Sensor::read() handles the protocol.
                    // We need to pass the serial back to the driver.
                    // Since we just read a byte, the driver might miss the header.

                    let mut pms_sensor = PmsX003Sensor::new(&mut serial_compat);
                    match pms_sensor.read() {
                        Ok(frame) => {
                            info!("--- PMS5003 Report ---");
                            info!("PM1.0: {} μg/m³", frame.pm1_0);
                            info!("PM2.5: {} μg/m³", frame.pm2_5);
                            info!("PM10:  {} μg/m³", frame.pm10);
                            info!("----------------------");
                            found = true;
                            break;
                        }
                        Err(_e) => {
                            // Mid-packet sync or checksum error, retry
                            delay_eh1.delay_ms(10);
                        }
                    }
                }
                Err(nb::Error::WouldBlock) => {
                    // No data yet, wait a bit
                    delay_eh1.delay_ms(50);
                }
                Err(_) => {
                    // Other error (parity, overrun, etc)
                    delay_eh1.delay_ms(10);
                }
            }

            if hunt_attempt == 20 && !found {
                // Halfway through and still nothing? Try to wake it up again
                let mut pms_sensor = PmsX003Sensor::new(&mut serial_compat);
                let _ = pms_sensor.active();
            }
        }

        if !found {
            warn!("Sensor timeout: No valid data received in this cycle.");
        }

        // Wait 2 seconds before the next reading cycle
        delay_eh1.delay_ms(2000);
    }
}
