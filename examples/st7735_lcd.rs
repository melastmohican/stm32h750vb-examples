#![no_std]
#![no_main]

use panic_probe as _;
use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use embedded_graphics::image::{Image, ImageRaw, ImageRawLE};
use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_hal_compat::eh1_0::delay::DelayNs;
use embedded_hal_compat::eh1_0::digital::OutputPin;
use embedded_hal_compat::ForwardCompat;
use stm32h7xx_hal::{self as hal};
use hal::hal::spi;
use hal::prelude::*;
use hal::stm32;
use st7735_lcd::{Orientation, ST7735};
use stm32h7xx_hal::spi::NoMiso;
use tinybmp::Bmp;

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
    let mosi = gpioe.pe14.into_alternate();

    let mut rst = gpioe.pe15.into_push_pull_output().forward();
    let mut dc = gpioe.pe13.into_push_pull_output().forward();
    let mut cs = gpioe.pe11.into_push_pull_output().forward();
    let mut led = Led::new(gpioe.pe10.into_push_pull_output().forward());
   
    // Initialise the SPI peripheral.
    let spi = dp.SPI4.spi(
        (sck, NoMiso, mosi),
        spi::MODE_0,
        3.MHz(),
        ccdr.peripheral.SPI4,
        &ccdr.clocks,
    ).forward();
    
    //let mut delay = cp.SYST.delay(ccdr.clocks);
    let mut delay = cortex_m::delay::Delay::new(cp.SYST, ccdr.clocks.sysclk().raw()).forward();
    led.update(&mut delay);

    let spi_dev = ExclusiveDevice::new_no_delay(spi, cs).unwrap();

    let mut display = ST7735::<_, _, _>::new(spi_dev, dc, rst, false, true, 160,80);
    display.init(&mut delay).unwrap();
    display.set_offset(1, 26);
    display.set_orientation(&Orientation::LandscapeSwapped).unwrap();
    display.clear(Rgb565::BLACK).expect("Unable to clear");
    
    
    // draw ferris
    let image_raw: ImageRawLE<Rgb565> = ImageRaw::new(include_bytes!("ferris.raw"), 86);
    let image: Image<_> = Image::new(&image_raw, Point::new(80, 8));
    image.draw(&mut display).unwrap();
    // draw rust logo
    let logo = Bmp::from_slice(include_bytes!("rust.bmp")).unwrap();
    let logo = Image::new(&logo, Point::new(0, 0));
    logo.draw(&mut display).unwrap();
    
    hprintln!("lcd test finished.");
    loop {
        cortex_m::asm::wfi(); // sleep infinitely
    }
}

