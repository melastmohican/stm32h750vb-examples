//! Button Polling Example for WeAct MiniSTM32H750VB
//!
//! K1 User Button: PC13 (Active LOW, floating input required)
//! LED: PE3
//!
//! Press K1 to toggle the LED.
//!
//! ## Hardware Note
//! This board's K1 button has weak drive on PC13. The pin must be
//! configured as floating input — the internal pull-up is too strong
//! for the button to overcome. The floating pin needs several seconds
//! to settle after configuration.

#![no_main]
#![no_std]

use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use panic_probe as _;
use stm32h7xx_hal::{pac, prelude::*};

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().expect("cannot take peripherals");
    let cp = pac::CorePeripherals::take().expect("cannot take core peripherals");

    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();
    let rcc = dp.RCC.constrain();
    let ccdr = rcc.sys_ck(100.MHz()).freeze(pwrcfg, &dp.SYSCFG);

    let gpioc = dp.GPIOC.split(ccdr.peripheral.GPIOC);
    let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);

    // LED on PE3
    let mut led = gpioe.pe3.into_push_pull_output();
    led.set_low(); // Start LED OFF

    // K1 button — must use floating input
    let button = gpioc.pc13.into_floating_input();
    let mut delay = cp.SYST.delay(ccdr.clocks);

    // Wait for floating pin to settle
    delay.delay_ms(5000_u16);

    hprintln!("Button Poll Ready. Press K1!");

    let mut was_pressed = button.is_low();

    loop {
        delay.delay_ms(50_u16);
        let pressed = button.is_low();

        if pressed && !was_pressed {
            led.toggle();
        }

        was_pressed = pressed;
    }
}
