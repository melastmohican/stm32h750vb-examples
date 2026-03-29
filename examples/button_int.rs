//! Standard Interrupt Button Example for WeAct MiniSTM32H750VB
//!
//! K1 User Button: PC13 (Active LOW, floating input required)
//! LED: PE3
//!
//! Press K1 to toggle the LED via EXTI interrupt.
//!
//! ## Hardware Note
//! This board's K1 button has weak drive on PC13. The pin must be
//! configured as floating input — the internal pull-up is too strong
//! for the button to overcome. The floating pin needs several seconds
//! to settle after configuration.

#![no_main]
#![no_std]

use core::cell::RefCell;
use cortex_m::interrupt::Mutex;
use panic_probe as _;

use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use stm32h7xx_hal::gpio::gpioc::PC13;
use stm32h7xx_hal::gpio::gpioe::PE3;
use stm32h7xx_hal::gpio::{Edge, ExtiPin, Input, Output, PushPull};
use stm32h7xx_hal::pac::interrupt;
use stm32h7xx_hal::{pac, prelude::*};

static LED: Mutex<RefCell<Option<PE3<Output<PushPull>>>>> = Mutex::new(RefCell::new(None));
static BUTTON: Mutex<RefCell<Option<PC13<Input>>>> = Mutex::new(RefCell::new(None));

#[entry]
fn main() -> ! {
    let mut dp = pac::Peripherals::take().expect("cannot take peripherals");
    let cp = pac::CorePeripherals::take().expect("cannot take core peripherals");

    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();
    let rcc = dp.RCC.constrain();
    let ccdr = rcc.sys_ck(100.MHz()).freeze(pwrcfg, &dp.SYSCFG);

    let gpioc = dp.GPIOC.split(ccdr.peripheral.GPIOC);
    let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);

    let mut led = gpioe.pe3.into_push_pull_output();
    led.set_low(); // LED OFF

    // K1 button — must use floating input
    let mut button = gpioc.pc13.into_floating_input();
    let mut delay = cp.SYST.delay(ccdr.clocks);

    // Wait for floating pin to settle
    delay.delay_ms(5000_u16);

    // Configure EXTI after pin is stable
    button.make_interrupt_source(&mut dp.SYSCFG);
    button.trigger_on_edge(&mut dp.EXTI, Edge::Falling);
    button.clear_interrupt_pending_bit();
    button.enable_interrupt(&mut dp.EXTI);

    cortex_m::interrupt::free(|cs| {
        LED.borrow(cs).replace(Some(led));
        BUTTON.borrow(cs).replace(Some(button));
    });

    unsafe {
        cortex_m::peripheral::NVIC::unmask(pac::Interrupt::EXTI15_10);
    }

    hprintln!("Button Int Ready. Press K1!");

    loop {
        cortex_m::asm::wfi();
    }
}

#[interrupt]
fn EXTI15_10() {
    cortex_m::interrupt::free(|cs| {
        if let Some(ref mut button) = *BUTTON.borrow(cs).borrow_mut() {
            if button.check_interrupt() {
                button.clear_interrupt_pending_bit();
                if let Some(ref mut led) = *LED.borrow(cs).borrow_mut() {
                    led.toggle();
                }
            }
        }
    });
}
