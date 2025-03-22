#![no_std]
#![no_main]

use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use display_interface_spi::SPIInterface;
use embedded_graphics::geometry::Point;
use embedded_graphics::image::{Image, ImageRaw, ImageRawLE};
use embedded_graphics::prelude::Drawable;
use embedded_graphics_core::draw_target::DrawTarget;
use embedded_graphics_core::pixelcolor::Rgb565;
use embedded_graphics_core::prelude::RgbColor;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_hal_compat::eh1_0::delay::DelayNs;
use embedded_hal_compat::eh1_0::digital::OutputPin;
use embedded_hal_compat::ForwardCompat;
use mipidsi::models::ST7735s;
use mipidsi::options::{ColorOrder, Orientation, Rotation};
use mipidsi::Builder;
use mipidsi::options::ColorInversion::Inverted;
use stm32h7xx_hal::{self as hal, hal::spi, prelude::*, stm32};
use tinybmp::Bmp;
use panic_probe as _;

struct Led<P: OutputPin> {
    pin: P,
    brightness: u8, // Duty cycle (0-10)
}

impl<P: OutputPin> Led<P> {
    pub fn new(pin: P) -> Self {
        Self {
            pin,
            brightness: 5, // Default 50% duty cycle
        }
    }

    pub fn set_brightness(&mut self, value: u8) {
        if value <= 10 {
            self.brightness = value;
        }
    }

    pub fn update<DELAY>(&mut self, delay: &mut DELAY)
    where
        DELAY: DelayNs,
    {

        for count in 0..10 {
            if count < self.brightness {
                self.pin.set_high().ok();
            } else {
                self.pin.set_low().ok();
            }
            delay.delay_ms(1); // Adjust for timing
        }
    }
}

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = stm32::Peripherals::take().unwrap();

    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();
    let rcc = dp.RCC.constrain();
    let ccdr = rcc
        .sys_ck(96.MHz())
        .pll1_q_ck(48.MHz())
        .freeze(pwrcfg, &dp.SYSCFG);

    let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);

    // SPI4
    let sck = gpioe.pe12.into_alternate();
    let miso = hal::spi::NoMiso;
    let mosi = gpioe.pe14.into_alternate();

    let rst = gpioe.pe15.into_push_pull_output().forward();
    let dc = gpioe.pe13.into_push_pull_output().forward();
    let cs = gpioe.pe11.into_push_pull_output().forward();
    let mut led = Led::new(gpioe.pe10.into_push_pull_output().forward());

    // Initialise the SPI peripheral.
    let spi = dp
        .SPI4
        .spi(
            (sck, miso, mosi),
            spi::MODE_0,
            3.MHz(),
            ccdr.peripheral.SPI4,
            &ccdr.clocks,
        )
        .forward();

    //let mut delay = cp.SYST.delay(ccdr.clocks).forward();
    let mut delay = cortex_m::delay::Delay::new(cp.SYST, ccdr.clocks.sysclk().raw()).forward();

    led.update(&mut delay);

    let spi_device = ExclusiveDevice::new_no_delay(spi, cs).unwrap();
    let di = SPIInterface::new(spi_device, dc);

    let mut display = Builder::new(ST7735s, di)
        .reset_pin(rst)
        .color_order(ColorOrder::Bgr)
        .orientation(Orientation::new().rotate(Rotation::Deg270))
        .invert_colors(Inverted)
        .display_size(160, 80)
        .init(&mut delay)
        .unwrap();

    display.clear(Rgb565::BLACK).unwrap();

    hprintln!("draw ferris");
    // draw ferris
    let image_raw: ImageRawLE<Rgb565> = ImageRaw::new(include_bytes!("ferris.raw"), 86);
    let image: Image<_> = Image::new(&image_raw, Point::new(34, 8));
    image.draw(&mut display).unwrap();
    hprintln!("draw bitmpap");
    let raw_image: Bmp<Rgb565> = Bmp::from_slice(include_bytes!("rust.bmp")).unwrap();
    let image = Image::new(&raw_image, Point::new(0, 0));
    image.draw(&mut display).unwrap();

    loop {
        cortex_m::asm::wfi(); // sleep infinitely
    }
}
