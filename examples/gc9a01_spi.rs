//! # GC9A01 Round LCD Display SPI Example for WeAct MiniSTM32H750VB
//!
//! Draw images on a 240x240 round GC9A01 display over SPI2.
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
//! Run with `cargo run --example gc9a01_spi`.

#![no_std]
#![no_main]

use cortex_m_rt::entry;
use defmt_rtt as _;
use display_interface_spi::SPIInterface;
use embedded_graphics::{geometry::Point, image::Image, pixelcolor::Rgb565, prelude::*};
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
use tinybmp::Bmp;

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

    // Turn on backlight (Assuming Active HIGH for external module, or simple 3.3V)
    let _ = bl.set_high();

    // --- SPI2 Initialization ---
    let spi = dp
        .SPI2
        .spi(
            (sck, miso, mosi),
            stm32h7xx_hal::spi::MODE_0,
            20.MHz(), // Start with 20MHz for stability
            ccdr.peripheral.SPI2,
            &ccdr.clocks,
        )
        .forward(); // Wrap for EH 1.0

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

    defmt::info!("Drawing images...");

    // Draw ferris (86x56)
    let ferris_data = Bmp::from_slice(include_bytes!("ferris.bmp")).unwrap();
    let ferris = Image::new(&ferris_data, Point::new(85, 40));
    ferris.draw(&mut display).unwrap();

    defmt::info!("Ferris drawn!");

    // Draw Rust logo (64x64)
    let logo_data = Bmp::from_slice(include_bytes!("rust.bmp")).unwrap();
    let logo = Image::new(&logo_data, Point::new(88, 121));
    logo.draw(&mut display).unwrap();

    defmt::info!("Rust logo drawn!");
    defmt::info!("--- Loop Started ---");

    loop {
        cortex_m::asm::wfi();
    }
}
