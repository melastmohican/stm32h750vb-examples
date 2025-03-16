#![deny(warnings)]
#![no_main]
#![no_std]

use panic_probe as _;
use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use stm32h7xx_hal::{pac, prelude::*};

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    // Constrain and Freeze power
    hprintln!("Setup PWR...                  ");
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();

    // Constrain and Freeze clock
    hprintln!("Setup RCC...                  ");
    let rcc = dp.RCC.constrain();
    let ccdr = rcc.sys_ck(100.MHz()).freeze(pwrcfg, &dp.SYSCFG);

    hprintln!("stm32h7xx-hal example - Blinky");

    let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);

    // Configure PE1 as output.
    let mut led = gpioe.pe3.into_push_pull_output();

    // Get the delay provider.
    let mut delay = cp.SYST.delay(ccdr.clocks);

    loop {
        led.set_high();
        delay.delay_ms(500_u16);

        led.set_low();
        delay.delay_ms(500_u16);
    }
}
