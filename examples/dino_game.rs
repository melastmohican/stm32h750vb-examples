//! Chrome Dino Game for WeAct MiniSTM32H750VB (mipidsi version)
//!
//! Controls:
//! - K1 User Button (PC13) to Jump.
//! - External Button (PA0 to GND) to Jump.
//!   Display: ST7735 LCD on SPI4 (PE11-PE15).
//!
//! Using native embedded-hal 1.0 with mipidsi 0.8 via forward compat.

#![no_main]
#![no_std]

use panic_probe as _;

#[rtic::app(device = stm32h7xx_hal::pac, peripherals = true, dispatchers = [EXTI1])]
mod app {
    use defmt_rtt as _;
    use display_interface_spi::SPIInterface;
    use embedded_graphics::{
        mono_font::{ascii::FONT_6X10, MonoTextStyle},
        pixelcolor::Rgb565,
        prelude::*,
        primitives::{Line, PrimitiveStyle, Rectangle},
        text::Text,
    };
    use embedded_hal_bus::spi::ExclusiveDevice;
    use embedded_hal_compat::{markers::ForwardOutputPin, ForwardCompat};
    use mipidsi::{
        models::ST7735s,
        options::{ColorInversion, ColorOrder, Orientation, Rotation},
        Builder, Display,
    };
    use stm32h7xx_hal::gpio::{
        gpioa::PA0, gpioc::PC13, gpioe::PE3, Edge, ExtiPin, Input, Output, PushPull,
    };
    use stm32h7xx_hal::prelude::*;
    use stm32h7xx_hal::spi::{self, NoMiso};

    // --- Sprites (Bitmasks) ---
    const DINO_WIDTH: i32 = 16;
    const DINO_HEIGHT: i32 = 16;
    const DINO_IDLE: [u16; 16] = [
        0b0000011111000000,
        0b0000010111100000,
        0b0000011111110000,
        0b0000011111111000,
        0b0000011111100000,
        0b0000011111111100,
        0b0001111111100000,
        0b0111111111111100,
        0b1111111111111000,
        0b1111111111110000,
        0b1111111111000000,
        0b0111111110000000,
        0b0001111101000000,
        0b0000110110000000,
        0b0000110110000000,
        0b0000110111000000,
    ];

    const CACTUS_WIDTH: i32 = 8;
    const CACTUS_HEIGHT: i32 = 12;
    const CACTUS: [u8; 12] = [
        0b00011000, 0b00011000, 0b01011010, 0b01011010, 0b01111110, 0b01111110, 0b00011000,
        0b00011000, 0b00011000, 0b00011000, 0b00011000, 0b00011000,
    ];

    const SCREEN_WIDTH: i32 = 160;
    const GROUND_Y: i32 = 65;
    const SCALE: i32 = 100;
    const GRAVITY: i32 = 6; // 0.06 px/frame^2
    const JUMP_FORCE: i32 = -240; // -2.4 px/frame
    const INITIAL_SPEED: i32 = 60; // 0.6 px/frame

    pub struct GameState {
        dino_y: i32,
        prev_dino_y: i32,
        dino_vel_y: i32,
        is_jumping: bool,
        jump_requested: bool,
        score: u32,
        cactus_x: i32,
        prev_cactus_x: i32,
        speed: i32,
        game_over: bool,
        frame_count: u32,
    }

    impl GameState {
        fn new() -> Self {
            let start_y = (GROUND_Y - DINO_HEIGHT) * SCALE;
            Self {
                dino_y: start_y,
                prev_dino_y: start_y,
                dino_vel_y: 0,
                is_jumping: false,
                jump_requested: false,
                score: 0,
                cactus_x: SCREEN_WIDTH * SCALE,
                prev_cactus_x: SCREEN_WIDTH * SCALE,
                speed: INITIAL_SPEED,
                game_over: false,
                frame_count: 0,
            }
        }

        fn update(&mut self) {
            if self.game_over {
                return;
            }

            // Process latched jump request
            if self.jump_requested && !self.is_jumping {
                self.is_jumping = true;
                self.dino_vel_y = JUMP_FORCE;
            }
            self.jump_requested = false;

            self.prev_dino_y = self.dino_y;
            if self.is_jumping {
                self.dino_vel_y += GRAVITY;
                self.dino_y += self.dino_vel_y;

                if self.dino_y >= (GROUND_Y - DINO_HEIGHT) * SCALE {
                    self.dino_y = (GROUND_Y - DINO_HEIGHT) * SCALE;
                    self.dino_vel_y = 0;
                    self.is_jumping = false;
                }
            }

            self.prev_cactus_x = self.cactus_x;
            self.cactus_x -= self.speed;
            if self.cactus_x < (-CACTUS_WIDTH) * SCALE {
                self.cactus_x = (SCREEN_WIDTH + (self.frame_count % 50) as i32) * SCALE;
                self.score += 10;
                if self.score.is_multiple_of(100) && self.speed < 800 {
                    self.speed += 20;
                }
            }

            let d_x = 20;
            let d_y = self.dino_y / SCALE;
            let c_x = self.cactus_x / SCALE;
            let c_y = GROUND_Y - CACTUS_HEIGHT;

            if d_x < c_x + CACTUS_WIDTH
                && d_x + DINO_WIDTH > c_x
                && d_y < c_y + CACTUS_HEIGHT
                && d_y + DINO_HEIGHT > c_y
            {
                self.game_over = true;
            }

            self.frame_count += 1;
        }
    }

    // Type definition for RTIC resources using wrappers
    type Spi4 = stm32h7xx_hal::pac::SPI4;
    type SpiPort = stm32h7xx_hal::spi::Spi<Spi4, stm32h7xx_hal::spi::Enabled>;
    type ForwardSpi = embedded_hal_compat::Forward<SpiPort>;
    type ForwardPin = embedded_hal_compat::Forward<
        stm32h7xx_hal::gpio::Pin<'E', 13, Output<PushPull>>,
        ForwardOutputPin,
    >;
    type ForwardPinRst = embedded_hal_compat::Forward<
        stm32h7xx_hal::gpio::Pin<'E', 15, Output<PushPull>>,
        ForwardOutputPin,
    >;
    type ForwardPinCs = embedded_hal_compat::Forward<
        stm32h7xx_hal::gpio::Pin<'E', 11, Output<PushPull>>,
        ForwardOutputPin,
    >;

    type SpiDevice = ExclusiveDevice<ForwardSpi, ForwardPinCs, embedded_hal_bus::spi::NoDelay>;
    type Interface = SPIInterface<SpiDevice, ForwardPin>;
    type Lcd = Display<Interface, ST7735s, ForwardPinRst>;

    #[shared]
    struct Shared {
        game: GameState,
    }

    #[local]
    struct Local {
        lcd: Lcd,
        button: PC13<Input>,
        ext_button: PA0<Input>,
        led: PE3<Output<PushPull>>,
    }

    #[init]
    fn init(mut ctx: init::Context) -> (Shared, Local, init::Monotonics) {
        let pwr = ctx.device.PWR.constrain();
        let pwrcfg = pwr.freeze();
        let rcc = ctx.device.RCC.constrain();
        let ccdr = rcc.sys_ck(400.MHz()).freeze(pwrcfg, &ctx.device.SYSCFG);

        let gpioa = ctx.device.GPIOA.split(ccdr.peripheral.GPIOA);
        let gpioc = ctx.device.GPIOC.split(ccdr.peripheral.GPIOC);
        let gpioe = ctx.device.GPIOE.split(ccdr.peripheral.GPIOE);

        let sck = gpioe.pe12.into_alternate();
        let mosi = gpioe.pe14.into_alternate();
        let rst = gpioe.pe15.into_push_pull_output().forward();
        let dc = gpioe.pe13.into_push_pull_output().forward();
        let cs = gpioe.pe11.into_push_pull_output().forward();
        let mut bl = gpioe.pe10.into_push_pull_output();
        bl.set_low();

        let mut led = gpioe.pe3.into_push_pull_output();
        led.set_high();

        let spi = ctx
            .device
            .SPI4
            .spi(
                (sck, NoMiso, mosi),
                spi::MODE_0,
                20.MHz(),
                ccdr.peripheral.SPI4,
                &ccdr.clocks,
            )
            .forward();

        let spi_dev = ExclusiveDevice::new_no_delay(spi, cs).unwrap();
        let di = SPIInterface::new(spi_dev, dc);
        let mut delay =
            cortex_m::delay::Delay::new(ctx.core.SYST, ccdr.clocks.sysclk().raw()).forward();

        let mut lcd = Builder::new(ST7735s, di)
            .reset_pin(rst)
            .color_order(ColorOrder::Bgr)
            .invert_colors(ColorInversion::Inverted)
            .display_size(80, 160)
            .display_offset(26, 1)
            .orientation(Orientation::new().rotate(Rotation::Deg270))
            .init(&mut delay)
            .unwrap();

        lcd.clear(Rgb565::WHITE).unwrap();

        // Internal Button (PC13)
        let mut button = gpioc.pc13.into_floating_input();
        button.make_interrupt_source(&mut ctx.device.SYSCFG);
        button.trigger_on_edge(&mut ctx.device.EXTI, Edge::Falling);
        button.enable_interrupt(&mut ctx.device.EXTI);

        // External Button (PA0)
        let mut ext_button = gpioa.pa0.into_pull_up_input();
        ext_button.make_interrupt_source(&mut ctx.device.SYSCFG);
        ext_button.trigger_on_edge(&mut ctx.device.EXTI, Edge::Falling);
        ext_button.enable_interrupt(&mut ctx.device.EXTI);

        (
            Shared {
                game: GameState::new(),
            },
            Local {
                lcd,
                button,
                ext_button,
                led,
            },
            init::Monotonics(),
        )
    }

    #[idle(shared = [game], local = [lcd, led])]
    fn idle(mut ctx: idle::Context) -> ! {
        let mut last_score = 0;
        let mut score_buf = [0u8; 12];

        loop {
            let mut game_over = false;
            let mut score = 0;
            let mut dino_y = 0;
            let mut prev_dino_y = 0;
            let mut cactus_x = 0;
            let mut prev_cactus_x = 0;

            ctx.shared.game.lock(|g| {
                g.update();
                game_over = g.game_over;
                score = g.score;
                dino_y = g.dino_y;
                prev_dino_y = g.prev_dino_y;
                cactus_x = g.cactus_x;
                prev_cactus_x = g.prev_cactus_x;
            });

            if game_over {
                let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::RED);
                Text::new("GAME OVER", Point::new(50, 40), text_style)
                    .draw(ctx.local.lcd)
                    .unwrap();
                ctx.local.led.set_low();

                cortex_m::asm::delay(80_000_000);
                ctx.shared.game.lock(|g| *g = GameState::new());
                ctx.local.lcd.clear(Rgb565::WHITE).unwrap();
                ctx.local.led.set_high();
                last_score = 0;
                continue;
            }

            // Scaled back to screen coordinates
            let draw_dino_y = dino_y / SCALE;
            let draw_prev_dino_y = prev_dino_y / SCALE;
            let draw_cactus_x = cactus_x / SCALE;
            let draw_prev_cactus_x = prev_cactus_x / SCALE;

            // OPTIMIZED: Only clear areas that changed
            if draw_dino_y != draw_prev_dino_y {
                Rectangle::new(
                    Point::new(20, draw_prev_dino_y),
                    Size::new(DINO_WIDTH as u32, DINO_HEIGHT as u32),
                )
                .into_styled(PrimitiveStyle::with_fill(Rgb565::WHITE))
                .draw(ctx.local.lcd)
                .unwrap();
            }
            if draw_cactus_x != draw_prev_cactus_x {
                Rectangle::new(
                    Point::new(draw_prev_cactus_x, GROUND_Y - CACTUS_HEIGHT),
                    Size::new(CACTUS_WIDTH as u32, CACTUS_HEIGHT as u32),
                )
                .into_styled(PrimitiveStyle::with_fill(Rgb565::WHITE))
                .draw(ctx.local.lcd)
                .unwrap();
            }

            Line::new(Point::new(0, GROUND_Y), Point::new(160, GROUND_Y))
                .into_styled(PrimitiveStyle::with_stroke(Rgb565::BLACK, 1))
                .draw(ctx.local.lcd)
                .unwrap();

            // Draw Dino
            draw_bitmask_16(
                ctx.local.lcd,
                20,
                draw_dino_y as i16,
                &DINO_IDLE,
                Rgb565::BLACK,
            );

            // Draw Cactus
            draw_bitmask_8(
                ctx.local.lcd,
                draw_cactus_x as i16,
                (GROUND_Y - CACTUS_HEIGHT) as i16,
                &CACTUS,
                Rgb565::GREEN,
            );

            // Score
            if score != last_score || score == 0 {
                let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::BLACK);
                let score_str = u32_to_str(score, &mut score_buf);
                Text::new(score_str, Point::new(120, 10), text_style)
                    .draw(ctx.local.lcd)
                    .unwrap();
                last_score = score;
            }

            cortex_m::asm::delay(6_400_000); // 60 FPS target at 400MHz
        }
    }

    fn draw_bitmask_16<D: DrawTarget<Color = Rgb565>>(
        display: &mut D,
        x: i16,
        y: i16,
        mask: &[u16; 16],
        color: Rgb565,
    ) where
        D::Error: core::fmt::Debug,
    {
        let colors = (0..256).map(|i| {
            let row = i / 16;
            let col = i % 16;
            if (mask[row] >> (15 - col)) & 1 == 1 {
                color
            } else {
                Rgb565::WHITE
            }
        });
        display
            .fill_contiguous(
                &Rectangle::new(Point::new(x as i32, y as i32), Size::new(16, 16)),
                colors,
            )
            .unwrap();
    }

    fn draw_bitmask_8<D: DrawTarget<Color = Rgb565>>(
        display: &mut D,
        x: i16,
        y: i16,
        mask: &[u8; 12],
        color: Rgb565,
    ) where
        D::Error: core::fmt::Debug,
    {
        let colors = (0..96).map(|i| {
            let row = i / 8;
            let col = i % 8;
            if (mask[row] >> (7 - col)) & 1 == 1 {
                color
            } else {
                Rgb565::WHITE
            }
        });
        display
            .fill_contiguous(
                &Rectangle::new(Point::new(x as i32, y as i32), Size::new(8, 12)),
                colors,
            )
            .unwrap();
    }

    #[task(binds = EXTI15_10, shared = [game], local = [button])]
    fn jump_internal(mut ctx: jump_internal::Context) {
        ctx.local.button.clear_interrupt_pending_bit();
        ctx.shared.game.lock(|g| g.jump_requested = true);
    }

    #[task(binds = EXTI0, shared = [game], local = [ext_button])]
    fn jump_external(mut ctx: jump_external::Context) {
        ctx.local.ext_button.clear_interrupt_pending_bit();
        ctx.shared.game.lock(|g| g.jump_requested = true);
    }

    fn u32_to_str(mut n: u32, buf: &mut [u8]) -> &str {
        if n == 0 {
            return "0";
        }
        let mut i = buf.len();
        while n > 0 && i > 0 {
            i -= 1;
            buf[i] = (n % 10) as u8 + b'0';
            n /= 10;
        }
        core::str::from_utf8(&buf[i..]).unwrap_or("")
    }
}
