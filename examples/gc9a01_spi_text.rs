//! # GC9A01 Round LCD Display SPI Text Example for WeAct MiniSTM32H750VB
//!
//! Draw text and shapes on a 240x240 round GC9A01 display over SPI2.
//!
//! This example uses SPI2 to avoid conflict with the built-in LCD (on SPI4).
//!
//! ## Hardware: GC9A01 240x240 Round LCD Display
//!
//! ## Wiring for GC9A01 Display (SPI2)
//!
//! | GC9A01 Pin | STM32H750 Pin | Note |
//! | :--- | :--- | :--- |
//! | **VCC** | 3.3V | |
//! | **GND** | GND | |
//! | **SCL (SCK)** | **PB13** | SPI2 SCK (AF5) |
//! | **SDA (MOSI)** | **PB15** | SPI2 MOSI (AF5) |
//! | **CS** | **PB12** | Chip Select |
//! | **DC** | **PD11** | Data/Command |
//! | **RES (RST)** | **PD10** | Reset |
//! | **BLK (Optional)**| **PC1** | Backlight Control |
//!
//! Run with `cargo run --example gc9a01_spi_text`.

#![no_std]
#![no_main]

use cortex_m_rt::entry;
use defmt_rtt as _;
use display_interface_spi::SPIInterface;
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, ascii::FONT_6X10, ascii::FONT_9X15_BOLD, MonoTextStyleBuilder},
    pixelcolor::{Rgb565, RgbColor},
    prelude::*,
    primitives::{Circle, Line, PrimitiveStyle},
    text::{Baseline, Text},
};
use embedded_hal::digital::OutputPin;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_hal_compat::ForwardCompat;
use mipidsi::{
    models::GC9A01,
    options::{ColorInversion, ColorOrder},
    Builder,
};
use panic_probe as _;
use stm32h750vb_examples::compat::DelayEh1;
use stm32h7xx_hal::{pac, prelude::*};

#[entry]
fn main() -> ! {
    let cp = pac::CorePeripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    // --- Configure PWR and RCC ---
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();
    let rcc = dp.RCC.constrain();
    let ccdr = rcc
        .sys_ck(400.MHz())
        .pll1_q_ck(48.MHz()) // Enable PLL1_Q for SPI123 kernel clock
        .freeze(pwrcfg, &dp.SYSCFG);

    // --- Initialize Delay and Wrap for EH 1.0 ---
    let delay = cp.SYST.delay(ccdr.clocks);
    let mut delay_eh1 = DelayEh1(delay);

    defmt::info!("Initializing GC9A01 round LCD display on SPI2...");

    // --- GPIO Configuration ---
    let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);
    let gpioc = dp.GPIOC.split(ccdr.peripheral.GPIOC);
    let gpiod = dp.GPIOD.split(ccdr.peripheral.GPIOD);

    // Configure SPI2 Pins
    let sck = gpiob.pb13.into_alternate::<5>();
    let mosi = gpiob.pb15.into_alternate::<5>();
    let miso = stm32h7xx_hal::spi::NoMiso;

    // Control pins
    let cs = gpiob.pb12.into_push_pull_output().forward();
    let dc = gpiod.pd11.into_push_pull_output().forward();
    let rst = gpiod.pd10.into_push_pull_output().forward();
    let mut bl = gpioc.pc1.into_push_pull_output().forward();

    // Turn on backlight
    let _ = bl.set_high();

    // --- SPI2 Initialization ---
    let spi = dp
        .SPI2
        .spi(
            (sck, miso, mosi),
            stm32h7xx_hal::spi::MODE_0,
            20.MHz(),
            ccdr.peripheral.SPI2,
            &ccdr.clocks,
        )
        .forward();

    // Create exclusive SPI device with CS pin
    let spi_device = ExclusiveDevice::new_no_delay(spi, cs).unwrap();

    // Create display interface
    let di = SPIInterface::new(spi_device, dc);

    defmt::info!("Initializing display via mipidsi...");

    // Create and initialize display using mipidsi
    let mut display = Builder::new(GC9A01, di)
        .reset_pin(rst)
        .invert_colors(ColorInversion::Inverted)
        .color_order(ColorOrder::Bgr)
        .display_size(240, 240)
        .init(&mut delay_eh1)
        .unwrap();

    defmt::info!("Display initialized!");

    // Clear screen to black
    display.clear(Rgb565::BLACK).unwrap();

    defmt::info!("Drawing text and shapes...");

    // Create text styles
    let title_style = MonoTextStyleBuilder::new()
        .font(&FONT_10X20)
        .text_color(Rgb565::WHITE)
        .build();

    let subtitle_style = MonoTextStyleBuilder::new()
        .font(&FONT_9X15_BOLD)
        .text_color(Rgb565::YELLOW)
        .build();

    let small_text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(Rgb565::CYAN)
        .build();

    // Draw title text centered (accounting for round display shape)
    Text::with_baseline("GC9A01", Point::new(90, 40), title_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();

    // Draw subtitle (13 chars * 9)
    Text::with_baseline(
        "Round Display",
        Point::new(61, 65),
        subtitle_style,
        Baseline::Top,
    )
    .draw(&mut display)
    .unwrap();

    // Draw small text (11 chars * 6)
    Text::with_baseline(
        "240x240 RGB",
        Point::new(87, 85),
        small_text_style,
        Baseline::Top,
    )
    .draw(&mut display)
    .unwrap();

    // Draw a large circle outline
    Circle::new(Point::new(15, 15), 210)
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::BLUE, 2))
        .draw(&mut display)
        .unwrap();

    // Draw smaller concentric circle
    Circle::new(Point::new(90, 110), 60)
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::GREEN, 2))
        .draw(&mut display)
        .unwrap();

    // Draw filled circles
    Circle::new(Point::new(110, 130), 20)
        .into_styled(PrimitiveStyle::with_fill(Rgb565::RED))
        .draw(&mut display)
        .unwrap();

    // Draw radiating lines
    let center = Point::new(120, 120);

    Line::new(center, Point::new(120, 20))
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::WHITE, 1))
        .draw(&mut display)
        .unwrap();

    Line::new(center, Point::new(220, 120))
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::WHITE, 1))
        .draw(&mut display)
        .unwrap();

    Line::new(center, Point::new(120, 220))
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::WHITE, 1))
        .draw(&mut display)
        .unwrap();

    Line::new(center, Point::new(20, 120))
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::WHITE, 1))
        .draw(&mut display)
        .unwrap();

    Text::with_baseline(
        "STM32H750VB",
        Point::new(87, 195),
        small_text_style,
        Baseline::Top,
    )
    .draw(&mut display)
    .unwrap();

    defmt::info!("Display complete!");

    loop {
        cortex_m::asm::wfi();
    }
}
