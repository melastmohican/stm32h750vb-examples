//! ## Hardware RNG Blinky Example for WeAct MiniSTM32H750VB
//!
//! This example uses the MCU's internal hardware True Random Number Generator (RNG)
//! to generate random blinking intervals for the onboard green LED.
//!
//! ### Wiring (WeAct MiniSTM32H750VB):
//! - LED (Green): PE3
//!
//! ### Implementation Details:
//! 1. **RNG Peripheral**: Configured to use the internal HSI48 oscillator as its kernel clock.
//! 2. **Entropy**: The hardware RNG provides true physical entropy.
//! 3. **Intervals**: Generates a random delay between 50ms and 500ms for an irregular blink pattern.

#![no_main]
#![no_std]

use panic_probe as _;

use cortex_m_rt::entry;
use defmt_rtt as _;
use stm32h7xx_hal::{delay::Delay, pac, prelude::*};

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().expect("cannot take core peripherals");
    let dp = pac::Peripherals::take().expect("cannot take peripherals");

    // Configure PWR and RCC
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();
    let rcc = dp.RCC.constrain();

    // Setup clocks:
    // - 400MHz System Clock
    let ccdr = rcc.sys_ck(400.MHz()).freeze(pwrcfg, &dp.SYSCFG);

    defmt::info!("Hardware RNG Blinky Example Started");

    // Initialize delay
    let mut delay = Delay::new(cp.SYST, ccdr.clocks);

    // Initialize PE3 as the onboard LED (Active LOW on WeAct board)
    let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);
    let mut led = gpioe.pe3.into_push_pull_output();
    led.set_high(); // Turn off LED initially

    // Initialize True Random Number Generator
    defmt::info!("Initializing RNG peripheral...");
    let mut rng = dp.RNG.constrain(ccdr.peripheral.RNG, &ccdr.clocks);

    loop {
        // Generate a random u32 from the hardware source
        let random_res: Result<u32, _> = rng.gen();
        match random_res {
            Ok(random_val) => {
                // Perceptual Randomness Strategy:
                // Mix rapid bursts with long pauses.
                // 70% of the time: fast bursts (20-150ms)
                // 30% of the time: long pauses (500-2000ms)
                let period: u32 = if (random_val % 10) < 7 {
                    (random_val % 131) + 20
                } else {
                    (random_val % 1501) + 500
                };

                defmt::info!(
                    "Random interval: {} ms {}",
                    period,
                    if period < 200 { "(burst)" } else { "(pause)" }
                );

                // Toggle the LED and wait for the random period
                led.toggle();
                delay.delay_ms(period);
            }
            Err(e) => {
                defmt::info!("RNG error: {:?}", defmt::Debug2Format(&e));
                // Simple fixed delay on error to allow retry
                delay.delay_ms(1000u32);
            }
        }
    }
}
