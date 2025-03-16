#![no_std]
#![no_main]

use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use display_interface_spi::SPIInterface;
use embedded_graphics::drawable::Drawable;
use embedded_graphics::geometry::Point;
use embedded_graphics::image::{Image, ImageRaw};
use embedded_graphics_core::draw_target::DrawTarget;
use embedded_graphics_core::pixelcolor::Rgb565;
use embedded_graphics_core::prelude::RgbColor;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_hal_compat::ForwardCompat;
use mipidsi::models::ST7735s;
use mipidsi::options::ColorOrder;
use mipidsi::Builder;
use stm32h7xx_hal::{self as hal, hal::spi, prelude::*, stm32};

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

    let rst = gpioe.pe10.into_push_pull_output().forward();
    let dc = gpioe.pe13.into_push_pull_output().forward();
    let cs = gpioe.pe11.into_push_pull_output().forward();

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

    let spi_device = ExclusiveDevice::new_no_delay(spi, cs).unwrap();
    let di = SPIInterface::new(spi_device, dc);

    let mut display = Builder::new(ST7735s, di)
        .reset_pin(rst)
        .color_order(ColorOrder::Rgb)
        //.orientation(Orientation::new().rotate(Rotation::Deg90).flip_vertical())
        .display_size(80, 160)
        .init(&mut delay)
        .unwrap();

    display.clear(Rgb565::BLACK).unwrap();

    hprintln!("draw ferris");
    // draw ferris
    let image_raw = ImageRaw::new(include_bytes!("ferris.raw"), 86, 64);
    let image = Image::new(&image_raw, Point::zero());
    image.draw(&mut display).unwrap();

    /*let ferris = Bmp::from_slice(include_bytes!("./ferris.bmp")).unwrap();
    let ferris = Image::new(&ferris, Point::new(34, 8));
    ferris.draw(&mut display).unwrap();*/

    loop {
        cortex_m::asm::wfi(); // sleep infinitely
    }
}
