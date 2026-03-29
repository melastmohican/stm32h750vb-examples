//! RTIC Button Example for WeAct MiniSTM32H750VB
//!
//! K1 User Button: PC13 (Active LOW, floating input required)
//! LED: PE3
//!
//! Press K1 to toggle the LED via RTIC EXTI interrupt.
//!
//! ## Hardware Note
//! This board's K1 button has weak drive on PC13. The pin must be
//! configured as floating input — the internal pull-up is too strong
//! for the button to overcome. The floating pin needs several seconds
//! to settle after configuration.

#![no_main]
#![no_std]

use panic_probe as _;

#[rtic::app(device = stm32h7xx_hal::pac, peripherals = true)]
mod app {
    use defmt_rtt as _;
    use stm32h7xx_hal::gpio::gpioc::PC13;
    use stm32h7xx_hal::gpio::gpioe::PE3;
    use stm32h7xx_hal::gpio::{Edge, ExtiPin, Input, Output, PushPull};
    use stm32h7xx_hal::prelude::*;

    #[shared]
    struct Shared {}

    #[local]
    struct Local {
        button: PC13<Input>,
        led: PE3<Output<PushPull>>,
    }

    #[init]
    fn init(mut ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let pwr = ctx.device.PWR.constrain();
        let pwrcfg = pwr.freeze();
        let rcc = ctx.device.RCC.constrain();
        let ccdr = rcc.sys_ck(100.MHz()).freeze(pwrcfg, &ctx.device.SYSCFG);

        let gpioc = ctx.device.GPIOC.split(ccdr.peripheral.GPIOC);
        let gpioe = ctx.device.GPIOE.split(ccdr.peripheral.GPIOE);

        let mut led = gpioe.pe3.into_push_pull_output();
        led.set_low(); // LED OFF

        // K1 button — must use floating input
        let mut button = gpioc.pc13.into_floating_input();

        // Wait for floating pin to settle
        cortex_m::asm::delay(500_000_000); // 5s at 100MHz

        // Configure EXTI after pin is stable
        button.make_interrupt_source(&mut ctx.device.SYSCFG);
        button.trigger_on_edge(&mut ctx.device.EXTI, Edge::Falling);
        button.clear_interrupt_pending_bit();
        button.enable_interrupt(&mut ctx.device.EXTI);

        defmt::info!("RTIC Button Ready. Press K1!");

        (Shared {}, Local { button, led }, init::Monotonics())
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            cortex_m::asm::wfi();
        }
    }

    #[task(binds = EXTI15_10, local = [button, led])]
    fn button_click(ctx: button_click::Context) {
        ctx.local.button.clear_interrupt_pending_bit();
        ctx.local.led.toggle();
    }
}
