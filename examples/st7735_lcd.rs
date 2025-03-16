#![no_std]
#![no_main]

use panic_probe as _;
use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use embedded_graphics::image::{Image, ImageRaw, ImageRawLE};
use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use stm32h7xx_hal::{self as hal};
use hal::hal::spi;
use hal::prelude::*;
use hal::stm32;
use st7735_lcd::Orientation;
use tinybmp::Bmp;

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
    let sck = gpioe.pe12.into_alternate_af5();
    let miso = hal::spi::NoMiso;
    let mosi = gpioe.pe14.into_alternate_af5();

    let rst = gpioe.pe10.into_push_pull_output();
    let dc = gpioe.pe13.into_push_pull_output();
    let cs = gpioe.pe11.into_push_pull_output();

    hprintln!("SPI");
    // Initialise the SPI peripheral.
    let mut spi = dp.SPI4.spi(
        (sck, miso, mosi),
        spi::MODE_0,
        3.MHz(),
        ccdr.peripheral.SPI4,
        &ccdr.clocks,
    );
    
    let mut delay = cp.SYST.delay(ccdr.clocks);

    let mut disp = st7735_lcd::ST7735::new(spi, cs, dc, false, true, 80, 160);
    hprintln!("display init");
    disp.init(&mut delay).unwrap();
    disp.set_orientation(&Orientation::LandscapeSwapped).unwrap();
    hprintln!("display clear");
    disp.clear(Rgb565::BLACK);

    disp.set_offset(0, 25);

    hprintln!("draw ferris");
    // draw ferris
    let image_raw: ImageRawLE<Rgb565> = ImageRaw::new(include_bytes!("ferris.raw"), 86, 64);
    let image: Image<_, Rgb565> = Image::new(&image_raw, Point::new(34, 8));
    image.draw(&mut disp).unwrap();

    //let ferris = Bmp::from_slice(include_bytes!("./ferris.bmp")).unwrap();
    //let ferris = Image::new(&ferris, Point::new(34, 8));
    //ferris.draw(&mut disp).unwrap();
    
    hprintln!("lcd test have done.");
    loop {
        cortex_m::asm::wfi(); // sleep infinitely
    }
}

