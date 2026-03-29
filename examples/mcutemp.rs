//! ## Temperature LCD Display Example for WeAct MiniSTM32H750VB
//!
//! This example demonstrates how to read the internal temperature via ADC3 and
//! display it in real-time on the onboard ST7735 LCD using the `mipidsi` driver.
//!
//! ### Wiring (WeAct MiniSTM32H750VB onboard LCD):
//! | Signal | STM32 Pin | Function |
//! | :--- | :--- | :--- |
//! | **SPI SCK** | PE12 (SPI4) | LCD Clock |
//! | **SPI MOSI** | PE14 (SPI4) | LCD Data |
//! | **LCD CS** | PE11 | Chip Select |
//! | **LCD RS/DC** | PE13 | Data/Command |
//! | **LCD RST** | PE15 | Reset |
//! | **LCD BL** | PE10 | Backlight (Active LOW) |

#![no_main]
#![no_std]

use panic_probe as _;

use core::fmt::Write;
use cortex_m_rt::entry;
use defmt_rtt as _;
use display_interface_spi::SPIInterface;
use embedded_graphics::mono_font::{iso_8859_1::FONT_10X20, MonoTextStyle, MonoTextStyleBuilder};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_graphics::text::Text;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_hal_compat::eh1_0::digital::OutputPin;
use embedded_hal_compat::ForwardCompat;
use mipidsi::models::ST7735s;
use mipidsi::options::ColorInversion::Inverted;
use mipidsi::options::{ColorOrder, Orientation, Rotation};
use mipidsi::Builder;
use stm32h7xx_hal::{
    adc,
    delay::Delay,
    pac,
    prelude::*,
    signature::{TS_CAL_110, TS_CAL_30},
    spi,
};

/// A simple stack-allocated string buffer for formatting
struct StringBuf {
    data: [u8; 32],
    pos: usize,
}

impl StringBuf {
    fn new() -> Self {
        Self {
            data: [0u8; 32],
            pos: 0,
        }
    }
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.data[..self.pos]).unwrap_or("")
    }
}

impl Write for StringBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let len = bytes.len();
        if self.pos + len > self.data.len() {
            return Err(core::fmt::Error);
        }
        self.data[self.pos..self.pos + len].copy_from_slice(bytes);
        self.pos += len;
        Ok(())
    }
}

/// Simple backlight helper
struct Backlight<P: OutputPin> {
    _pin: P,
}

impl<P: OutputPin> Backlight<P> {
    fn new(mut pin: P) -> Self {
        pin.set_low().ok(); // Active LOW -> ON
        Self { _pin: pin }
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

    // Setup multiple PLLs:
    // PLL1 Q -> SPI (48MHz)
    // PLL2 P -> ADC (4MHz)
    let ccdr = rcc
        .sys_ck(400.MHz())
        .pll1_q_ck(48.MHz())
        .pll2_p_ck(4.MHz())
        .freeze(pwrcfg, &dp.SYSCFG);

    defmt::info!("Temperature LCD Example Started");

    // --- ADC3 Initialization ---
    // Create HAL delay once (consumes SYST)
    let mut hal_delay = Delay::new(cp.SYST, ccdr.clocks);

    let mut adc3 = adc::Adc::adc3(
        dp.ADC3,
        4.MHz(),
        &mut hal_delay,
        ccdr.peripheral.ADC3,
        &ccdr.clocks,
    );
    adc3.set_resolution(adc::Resolution::SixteenBit);

    let mut channel = adc::Temperature::new();
    channel.enable(&adc3);
    hal_delay.delay_us(30_u16);
    let mut adc3 = adc3.enable();

    // Now convert HAL delay to EH1.0 compatible Delay for LCD (consumes hal_delay)
    let mut eh1_delay = hal_delay.forward();

    // --- LCD Hardware Setup (SPI4 on Port E) ---
    let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);
    let sck = gpioe.pe12.into_alternate();
    let miso = spi::NoMiso;
    let mosi = gpioe.pe14.into_alternate();

    let rst = gpioe.pe15.into_push_pull_output().forward();
    let dc = gpioe.pe13.into_push_pull_output().forward();
    let cs = gpioe.pe11.into_push_pull_output().forward();
    let _bl = Backlight::new(gpioe.pe10.into_push_pull_output().forward());

    let spi = dp
        .SPI4
        .spi(
            (sck, miso, mosi),
            spi::MODE_0,
            12.MHz(),
            ccdr.peripheral.SPI4,
            &ccdr.clocks,
        )
        .forward();

    let spi_device = ExclusiveDevice::new_no_delay(spi, cs).unwrap();
    let di = SPIInterface::new(spi_device, dc);

    // Initialize display with WeAct board orientation (Deg270)
    let mut display = Builder::new(ST7735s, di)
        .reset_pin(rst)
        .color_order(ColorOrder::Bgr)
        .orientation(Orientation::new().rotate(Rotation::Deg270))
        .invert_colors(Inverted)
        .display_size(80, 160)
        .display_offset(26, 1)
        .init(&mut eh1_delay) // Uses the forwarded delay
        .expect("LCD init failed");

    display.clear(Rgb565::BLACK).unwrap();

    // Draw Static UI
    let style = MonoTextStyle::new(&FONT_10X20, Rgb565::GREEN);
    Text::new("MCU TEMP", Point::new(40, 30), style)
        .draw(&mut display)
        .unwrap();

    defmt::info!("Displaying temperature on LCD...");

    // Get raw clocks for loop delay
    let sys_freq = ccdr.clocks.sysclk().raw();

    loop {
        // Read temperature
        let word: u32 = adc3.read(&mut channel).expect("Temperature read failed.");
        let cal_30 = TS_CAL_30::read() as f32;
        let cal_110 = TS_CAL_110::read() as f32;
        let slope = (110.0 - 30.0) / (cal_110 - cal_30);
        let temperature = slope * (word as f32 - cal_30) + 30.0;

        // Format string
        let mut buf = StringBuf::new();
        write!(buf, "{:.1} \u{00B0}C", temperature).ok();

        // Output to defmt (no float support, using integer part)
        defmt::info!("Temp: {} C", temperature as i32);

        // Update LCD Area
        // Overwrite background by drawing the text with a background color in the style
        let value_style_with_bg = MonoTextStyleBuilder::new()
            .font(&FONT_10X20)
            .text_color(Rgb565::WHITE)
            .background_color(Rgb565::BLACK)
            .build();

        Text::new(buf.as_str(), Point::new(45, 60), value_style_with_bg)
            .draw(&mut display)
            .unwrap();

        // Use assembly delay in the loop (approx 1s)
        cortex_m::asm::delay(sys_freq);
    }
}
