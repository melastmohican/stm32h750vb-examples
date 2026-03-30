//! BME280 + SSD1306 I2C Example for WeAct MiniSTM32H750VB
//!
//! # Hardware Used
//! - WeAct MiniSTM32H750VB
//! - Adafruit BME280 Breakout (I2C Address: 0x77/0x76)
//! - Generic SSD1306 OLED Breakout (I2C Address: 0x3C, 128x64 pixels)
//!
//! # Wiring (I2C2)
//! - SCL2: PB10
//! - SDA2: PB11
//! - 3.3V/GND as required
//!
//! This example uses a shared I2C bus with `embedded-hal-bus` and a custom
//! compatibility layer (`stm32h750vb_examples::compat`) to bridge the
//! HAL's EH v0.2 traits to the sensor's EH v1.0 requirements.

#![no_std]
#![no_main]

use bme280::i2c::BME280;
use cortex_m_rt::entry;
use defmt::{error, info};
use defmt_rtt as _;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Line, PrimitiveStyle},
    text::Text,
};
use embedded_hal::delay::DelayNs;
use heapless::String;
use panic_probe as _;
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};
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

    // --- Initialize Delay and Wrap for EH 1.0 (from crate compat) ---
    let delay = cp.SYST.delay(ccdr.clocks);
    let mut delay_eh1 = DelayEh1(delay);

    // --- GPIO Configuration ---
    let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);

    // --- I2C2 Initialization (PB10: SCL, PB11: SDA) ---
    info!("Initializing I2C2 (PB10: SCL, PB11: SDA)");
    let scl = gpiob.pb10.into_alternate::<4>().set_open_drain();
    let sda = gpiob.pb11.into_alternate::<4>().set_open_drain();

    let i2c = dp
        .I2C2
        .i2c((scl, sda), 100.kHz(), ccdr.peripheral.I2C2, &ccdr.clocks);

    // Wrap I2C for EH 1.0 (from crate compat)
    let i2c_eh1 = I2cEh1(i2c);

    // --- Share I2C bus using embedded-hal-bus ---
    let i2c_bus = core::cell::RefCell::new(i2c_eh1);

    // --- Initialize SSD1306 ---
    info!("Initializing SSD1306...");
    let interface = I2CDisplayInterface::new(embedded_hal_bus::i2c::RefCellDevice::new(&i2c_bus));
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    if display.init().is_err() {
        error!("SSD1306 failed at 0x3C! Trying 0x3D...");
        let interface2 = I2CDisplayInterface::new_alternate_address(
            embedded_hal_bus::i2c::RefCellDevice::new(&i2c_bus),
        );
        display = Ssd1306::new(interface2, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();
        if display.init().is_err() {
            error!("SSD1306 failed at 0x3D! Continuing to BME280 without display...");
        } else {
            info!("SSD1306 ready at 0x3D.");
            let _ = display.clear(BinaryColor::Off);
            let text_style = MonoTextStyleBuilder::new()
                .font(&FONT_6X10)
                .text_color(BinaryColor::On)
                .build();
            let _ = Text::new("Display Ready!", Point::new(0, 10), text_style).draw(&mut display);
            let _ = display.flush();
        }
    } else {
        info!("SSD1306 ready at 0x3C.");
        let _ = display.clear(BinaryColor::Off);
        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();
        let _ = Text::new("Display Ready!", Point::new(0, 10), text_style).draw(&mut display);
        let _ = display.flush();
    }

    // --- Initialize BME280 ---
    info!("Initializing BME280...");
    let mut bme280 = BME280::new_primary(embedded_hal_bus::i2c::RefCellDevice::new(&i2c_bus));
    if bme280.init(&mut delay_eh1).is_err() {
        error!("Could not find a valid BME280 sensor at 0x77!");
        // Try fallback
        bme280 = BME280::new_secondary(embedded_hal_bus::i2c::RefCellDevice::new(&i2c_bus));
        if bme280.init(&mut delay_eh1).is_err() {
            error!("Could not find BME280 at 0x76 either.");
        } else {
            info!("Found BME280 at 0x76.");
        }
    } else {
        info!("BME280 ready at 0x77.");
    }

    info!("--- BME280 + SSD1306 I2C Test Loop ---");

    loop {
        let measurements = match bme280.measure(&mut delay_eh1) {
            Ok(m) => Some(m),
            Err(_) => {
                error!("Failed to read from BME280 sensor!");
                None
            }
        };

        let _ = display.clear(BinaryColor::Off);
        let text_style = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();

        // Header
        let _ = Text::new("BME280 Sensor Data", Point::new(0, 10), text_style).draw(&mut display);
        let _ = Line::new(Point::new(0, 14), Point::new(127, 14))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(&mut display);

        if let Some(m) = measurements {
            let temp = m.temperature;
            let pressure = m.pressure / 100.0; // Convert Pa to hPa
            let humidity = m.humidity;

            info!(
                "Temp: {} C, Hum: {} %, Pres: {} hPa",
                temp, humidity, pressure
            );

            // Display Temp
            let mut temp_str: String<32> = String::new();
            core::fmt::write(&mut temp_str, format_args!("{:.1} C", temp)).unwrap();
            let _ = Text::new(&temp_str, Point::new(0, 30), text_style).draw(&mut display);

            // Display Humidity
            let mut hum_str: String<32> = String::new();
            core::fmt::write(&mut hum_str, format_args!("Humidity: {:.1} %", humidity)).unwrap();
            let _ = Text::new(&hum_str, Point::new(0, 44), text_style).draw(&mut display);

            // Display Pressure
            let mut press_str: String<32> = String::new();
            core::fmt::write(
                &mut press_str,
                format_args!("Pressure: {:.0} hPa", pressure),
            )
            .unwrap();
            let _ = Text::new(&press_str, Point::new(0, 58), text_style).draw(&mut display);
        } else {
            let _ = Text::new("Sensor Error!", Point::new(0, 30), text_style).draw(&mut display);
            let _ = Text::new("Check I2C/Power", Point::new(0, 44), text_style).draw(&mut display);
        }

        let _ = display.flush();
        delay_eh1.delay_ms(2000);
    }
}
