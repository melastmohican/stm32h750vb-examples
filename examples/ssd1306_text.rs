//! SSD1306 OLED Text & Graphics Example for WeAct MiniSTM32H750VB
//!
//! This example demonstrates drawing text and shapes on a 128x64 SSD1306 display over I2C2.
//!
//! # Hardware
//! - **MCU:** WeAct MiniSTM32H750VB
//! - **Display:** SSD1306 OLED (128x64)
//!
//! # Wiring (I2C2)
//! - **VCC -> 3.3V**
//! - **GND -> GND**
//! - **SCL -> PB10**
//! - **SDA -> PB11**
//!
//! Run with `cargo run --example ssd1306_text`.

#![no_std]
#![no_main]

use cortex_m_rt::entry;
use defmt::info;
use defmt_rtt as _;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};
use panic_probe as _;
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};
use stm32h750vb_examples::compat::I2cEh1;
use stm32h7xx_hal::{pac, prelude::*};

#[entry]
fn main() -> ! {
    info!("Initializing peripherals...");
    let dp = pac::Peripherals::take().unwrap();

    // --- Configure PWR and RCC ---
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();
    let rcc = dp.RCC.constrain();
    let ccdr = rcc.sys_ck(400.MHz()).freeze(pwrcfg, &dp.SYSCFG);

    // --- GPIO Configuration ---
    let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);

    // --- I2C2 Initialization (PB10: SCL, PB11: SDA) ---
    info!("Initializing I2C2 (PB10: SCL, PB11: SDA)");
    let scl = gpiob.pb10.into_alternate::<4>().set_open_drain();
    let sda = gpiob.pb11.into_alternate::<4>().set_open_drain();

    let i2c = dp
        .I2C2
        .i2c((scl, sda), 400.kHz(), ccdr.peripheral.I2C2, &ccdr.clocks);

    // Wrap I2C for EH 1.0
    let i2c_eh1 = I2cEh1(i2c);

    // --- Initialize SSD1306 ---
    info!("Initializing SSD1306...");
    let interface = I2CDisplayInterface::new(i2c_eh1);
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    display.init().unwrap();
    info!("Display initialized!");

    // --- Drawing ---
    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    display.clear(BinaryColor::Off).unwrap();

    // Draw title text
    Text::with_baseline("STM32H750VB", Point::new(30, 0), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();

    // Draw a separator line
    Line::new(Point::new(0, 12), Point::new(127, 12))
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(&mut display)
        .unwrap();

    // Draw a rectangle
    Rectangle::new(Point::new(10, 20), Size::new(40, 30))
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
        .draw(&mut display)
        .unwrap();

    // Draw a filled circle
    Circle::new(Point::new(80, 35), 15)
        .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
        .draw(&mut display)
        .unwrap();

    // Draw some text at bottom
    Text::with_baseline(
        "Hello, Rust!",
        Point::new(10, 54),
        text_style,
        Baseline::Top,
    )
    .draw(&mut display)
    .unwrap();

    display.flush().unwrap();
    info!("Display content rendered!");

    loop {
        cortex_m::asm::wfi();
    }
}
