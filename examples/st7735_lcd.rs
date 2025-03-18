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

    let rst = gpioe.pe15.into_push_pull_output();
    let dc = gpioe.pe13.into_push_pull_output();
    let cs = gpioe.pe11.into_push_pull_output();
    let mut bl = gpioe.pe10.into_push_pull_output();
    bl.set_high();
    
    // Initialise the SPI peripheral.
    let spi = dp.SPI4.spi(
        (sck, miso, mosi),
        spi::MODE_0,
        3.MHz(),
        ccdr.peripheral.SPI4,
        &ccdr.clocks,
    );
    
    let mut delay = cp.SYST.delay(ccdr.clocks);

    let mut display = st7735_lcd::ST7735::new(spi, dc, rst, false, true, 80, 160);
    display.init(&mut delay).unwrap();
    display.set_orientation(&Orientation::LandscapeSwapped).unwrap();
    display.clear(Rgb565::BLACK).expect("Unable to clear");
    display.set_offset(0, 25);
    
    // draw ferris
    let image_raw: ImageRawLE<Rgb565> = ImageRaw::new(include_bytes!("ferris.raw"), 86, 64);
    let image: Image<_, Rgb565> = Image::new(&image_raw, Point::new(34, 8));
    image.draw(&mut display).unwrap();

    //let ferris = Bmp::from_slice(include_bytes!("./ferris.bmp")).unwrap();
    //let ferris = Image::new(&ferris, Point::new(34, 8));
    //ferris.draw(&mut disp).unwrap();
    
    hprintln!("lcd test finished.");
    loop {
        cortex_m::asm::wfi(); // sleep infinitely
    }
}

