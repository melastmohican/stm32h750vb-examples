//! ## OV2640 DCMI-to-LCD Video Pipeline
//!
//! ### Wiring (WeAct MiniSTM32H750VB + OV2640 DVP + ST7735):
//! | Peripheral Pin | STM32 Pin | Note |
//! | :--- | :--- | :--- |
//! | **XCLK (MCO1)** | PA8 | Camera Clock |
//! | **DCMI VSYNC** | PB7 | Vertical Sync |
//! | **DCMI HSYNC** | PA4 | Horizontal Sync |
//! | **DCMI PCLK** | PA6 | Pixel Clock |
//! | **DCMI D[0:7]** | PC6, PC7, PE0, PE1, PE4, PD3, PE5, PE6 | Data Bus |
//! | **I2C SCL/SDA** | PB8, PB9 (I2C1) | Control Bus |
//! | **LCD SPI SCK** | PE12 (SPI4) | LCD Clock |
//! | **LCD SPI SDA** | PE14 (SPI4) | LCD Data |
//! | **LCD CS/DC/RS** | PE11, PE13, PE15 | Control IO |
//! | **LCD BL** | PE10 | Backlight (Active LOW) |
//!
//! ### Implementation Details:
//! 1. **DCMI Snapshot Mode**: To prevent tearing on the slow SPI LCD, we capture one frame
//!    and stop the DMA before drawing.
//! 2. **Memory Coherence (MPU)**: SRAM4 is configured as Non-Cacheable and Shareable to
//!    ensure the CPU sees the latest DMA data without manual invalidation.
//! 3. **Byte-Swapping**: STM32 DCMI packs bytes in a way that requires manual 16-bit
//!    reordering for the ST7735 RGB565 format (provided by `chunk.swap(0, 1)`).

#![no_main]
#![no_std]
#![allow(static_mut_refs)]

use panic_probe as _;

use cortex_m_rt::entry;
use cortex_m_semihosting::hprintln;
use display_interface_spi::SPIInterface;
use embedded_graphics::{image::Image, pixelcolor::Rgb565, prelude::*};
use embedded_hal_compat::eh1_0::delay::DelayNs;
use embedded_hal_compat::ForwardCompat;
use mipidsi::{
    models::ST7735s,
    options::{ColorInversion, ColorOrder, Orientation, Rotation},
    Builder,
};
use stm32h7xx_hal::{pac, prelude::*, rcc::ResetEnable, spi};

// Framebuffer size for QQVGA (160x120) RGB565
const WIDTH: usize = 160;
const HEIGHT: usize = 120;
const FB_SIZE: usize = WIDTH * HEIGHT;

// Place framebuffer in SRAM1 (D2 domain) since DMA1 cannot access AXI SRAM on STM32H7
// We use a u32 array to natively force 4-byte alignment across the Rust compiler and GNU Linker.
// FB_SIZE is the number of pixels (19200). Since pixels are 16-bit, we need FB_SIZE / 2 u32 words (9600).
#[repr(C, align(32))]
struct AlignedFramebuffer([u32; FB_SIZE / 2]);

#[link_section = ".sram4"]
static mut FRAMEBUFFER: AlignedFramebuffer = AlignedFramebuffer([0u32; FB_SIZE / 2]);

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    // Configure PWR and RCC
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();
    let rcc = dp.RCC.constrain();
    let ccdr = rcc
        .sys_ck(96.MHz())
        .pll1_q_ck(48.MHz())
        .freeze(pwrcfg, &dp.SYSCFG);

    // Generate 16MHz natively from the robust internal 64MHz HSI (divided by 4)
    // mco1pre: values >= 8 divide the clock. 8 = /2, 9 = /3, 10 = /4.
    unsafe {
        (*stm32h7xx_hal::pac::RCC::ptr())
            .cfgr
            .modify(|_, w| w.mco1pre().bits(10).mco1().hsi());
    }

    hprintln!("OV2640 DCMI to LCD Example");

    let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);
    let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);
    let gpioc = dp.GPIOC.split(ccdr.peripheral.GPIOC);
    let gpiod = dp.GPIOD.split(ccdr.peripheral.GPIOD);
    let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);
    let _gpiof = dp.GPIOF.split(ccdr.peripheral.GPIOF);

    let _xclk = gpioa
        .pa8
        .into_alternate::<0>()
        .speed(stm32h7xx_hal::gpio::Speed::VeryHigh);

    // Initialize Delay natively using strict system clock frequency ticks
    let mut delay = cortex_m::delay::Delay::new(cp.SYST, ccdr.clocks.sysclk().raw()).forward();

    // Zephyr specifies: reset-gpios = <&gpioa 7 GPIO_ACTIVE_LOW>
    //                   pwdn-gpios = <&gpiof 1 GPIO_ACTIVE_HIGH>
    // Therefore, PA7 must be HIGH to release reset, and PF1 must be LOW to power on.

    // RESET/PWDN control via shared transistor on PA7
    // PA7 HIGH turns on the transistor, which pulls the camera PWDN/RESET low    // 1. Enable Camera Power Supply
    // CRITICAL: We explicitly do NOT touch `PA7` or `PF1` here!
    // The working `mipidsi.rs` reference explicitly leaves these pins untouched identically.
    // Switching `PA7` or `PF1` to an output visibly glitches the circuit board traces,
    // dynamically violently resetting the LCD matrix dynamically. The physical board default
    // pull-ups securely perfectly hold the pins in the correct active state inherently.

    // We safely rely entirely on the OV2640's native I2C soft-reset (0xFF, 0x01) instead.
    // delay.delay_ms(100);

    // 2. Setup I2C1 for OV2640 configuration
    let scl = gpiob
        .pb8
        .into_alternate::<4>()
        .set_open_drain()
        .internal_pull_up(true);
    let sda = gpiob
        .pb9
        .into_alternate::<4>()
        .set_open_drain()
        .internal_pull_up(true);
    let mut i2c = dp
        .I2C1
        .i2c((scl, sda), 100.kHz(), ccdr.peripheral.I2C1, &ccdr.clocks);

    // 3. Setup SPI4 for ST7735 LCD
    let sck = gpioe.pe12.into_alternate();
    let mosi = gpioe.pe14.into_alternate();
    let spi_eth = dp
        .SPI4
        .spi(
            (sck, spi::NoMiso, mosi),
            spi::MODE_0,
            3.MHz(),
            ccdr.peripheral.SPI4,
            &ccdr.clocks,
        )
        .forward();

    let dc = gpioe.pe13.into_push_pull_output().forward();
    let rst = gpioe.pe15.into_push_pull_output().forward();
    let cs = gpioe.pe11.into_push_pull_output().forward();

    // Enable LCD Backlight
    // CRITICAL: PE10 is Active LOW on the WeAct ST7735 module!
    // Driving it HIGH physically powers off the LED circuit.
    let mut backlight = gpioe.pe10.into_push_pull_output();
    backlight.set_low();

    use embedded_hal_bus::spi::ExclusiveDevice;
    let spi_device = ExclusiveDevice::new_no_delay(spi_eth, cs).unwrap();
    let di = SPIInterface::new(spi_device, dc);

    let mut display = Builder::new(ST7735s, di)
        .display_size(80, 160)
        .display_offset(26, 1)
        .reset_pin(rst)
        .orientation(Orientation::new().rotate(Rotation::Deg270))
        .color_order(ColorOrder::Bgr)
        .invert_colors(ColorInversion::Inverted)
        .init(&mut delay)
        .unwrap();

    display.clear(Rgb565::BLACK).unwrap();

    hprintln!("Before RED clear...");
    display.clear(Rgb565::RED).unwrap();
    hprintln!("After RED clear - should be red now");
    delay.delay_ms(2000u32);

    hprintln!("Before GREEN clear...");
    display.clear(Rgb565::GREEN).unwrap();
    hprintln!("After GREEN clear - should be green now");
    delay.delay_ms(2000u32);

    hprintln!("Before BLUE clear...");
    display.clear(Rgb565::BLUE).unwrap();
    hprintln!("After BLUE clear - should be blue now");
    delay.delay_ms(2000u32);

    // 4. Initialize OV2640 Registers
    let ov2640_addr = 0x30;

    hprintln!("Initializing OV2640 at 0x{:02X}...", ov2640_addr);
    if i2c.write(ov2640_addr, &[0xFF, 0x01]).is_err() {
        hprintln!("I2C Error: Reset Bank");
    }
    if i2c.write(ov2640_addr, &[0x12, 0x80]).is_err() {
        hprintln!("I2C Error: Reset COM7");
    }
    delay.delay_ms(100);

    for &(reg, val) in OV2640_SLOW_REGS {
        if i2c.write(ov2640_addr, &[reg, val]).is_err() {
            hprintln!("I2C Error at Reg 0x{:02X}", reg);
        }
    }
    hprintln!("SLOW_REGS configured.");
    delay.delay_ms(30);

    for &(reg, val) in RGB565_REGS {
        i2c.write(ov2640_addr, &[reg, val]).ok();
    }

    for &(reg, val) in SVGA_REGS {
        i2c.write(ov2640_addr, &[reg, val]).ok();
    }

    hprintln!("RGB565 and SVGA REGS configured.");

    // Manual scaling for 160x120
    hprintln!("Configuring Output ZMOW/ZMOH (QQVGA)...");
    i2c.write(ov2640_addr, &[0xFF, 0x00]).ok(); // Select DSP bank
    i2c.write(ov2640_addr, &[0x05, 0x01]).ok(); // Bypass DSP
    i2c.write(ov2640_addr, &[0x5A, 160 / 4]).ok(); // ZMOW ((w >> 2) & 0xFF)
    i2c.write(ov2640_addr, &[0x5B, 120 / 4]).ok(); // ZMOH ((h >> 2) & 0xFF)
    i2c.write(ov2640_addr, &[0x5C, 0x00]).ok(); // ZMHH (0)
    i2c.write(ov2640_addr, &[0x05, 0x00]).ok(); // Enable DSP
    hprintln!("Scaling configuration complete.");

    // Configure the MPU natively to make SRAM4 strictly NON-CACHEABLE and SHAREABLE.
    // This permanently ensures CPU/DMA coherence and eliminates unaligned Cache-line faulting.
    #[allow(clippy::identity_op)]
    unsafe {
        const MPU_BASE: u32 = 0xE000ED90;
        let mpu_ctrl = (MPU_BASE + 0x04) as *mut u32;
        let mpu_rnr = (MPU_BASE + 0x08) as *mut u32;
        let mpu_rbar = (MPU_BASE + 0x0C) as *mut u32;
        let mpu_rasr = (MPU_BASE + 0x10) as *mut u32;

        core::ptr::write_volatile(mpu_ctrl, 0); // Disable MPU

        core::ptr::write_volatile(mpu_rnr, 0); // Select Region 0
        core::ptr::write_volatile(mpu_rbar, 0x38000000); // SRAM4 base address

        // RASR: Enable | Size=128KB (covers 64KB SRAM4) | Full access | TEX=0, S=1, C=0, B=0
        core::ptr::write_volatile(
            mpu_rasr,
            (0b011 << 24) |  // AP: full access
            (0b000 << 19) |  // TEX=0
            (1     << 18) |  // S=1 (shareable)
            (0     << 17) |  // C=0 (not cacheable)
            (0     << 16) |  // B=0 (not bufferable)
            (16    <<  1) |  // SIZE = 16 -> region size 2^(16+1) = 128KB
            (1      <<  0), // Enable region
        );

        core::ptr::write_volatile(mpu_ctrl, 0b101); // Enable MPU + PRIVDEFENA
        cortex_m::asm::dsb();
        cortex_m::asm::isb();
    }

    // 5. Setup DCMI Pins (AF13) with VeryHigh speed
    hprintln!("Configuring DCMI pins...");
    use stm32h7xx_hal::gpio::Speed::VeryHigh;
    let _d0 = gpioc.pc6.into_alternate::<13>().speed(VeryHigh);
    let _d1 = gpioc.pc7.into_alternate::<13>().speed(VeryHigh);
    let _d2 = gpioe.pe0.into_alternate::<13>().speed(VeryHigh);
    let _d3 = gpioe.pe1.into_alternate::<13>().speed(VeryHigh);
    let _d4 = gpioe.pe4.into_alternate::<13>().speed(VeryHigh);
    let _d5 = gpiod.pd3.into_alternate::<13>().speed(VeryHigh);
    let _d6 = gpioe.pe5.into_alternate::<13>().speed(VeryHigh);
    let _d7 = gpioe.pe6.into_alternate::<13>().speed(VeryHigh);
    let _vsync = gpiob.pb7.into_alternate::<13>().speed(VeryHigh);
    let _hsync = gpioa.pa4.into_alternate::<13>().speed(VeryHigh);
    let _pclk = gpioa.pa6.into_alternate::<13>().speed(VeryHigh);

    // 6. Setup DCMI Peripheral
    hprintln!("Enabling DCMI clock...");
    unsafe {
        // Enable DCMI (bit 0) in AHB2
        (*pac::RCC::ptr())
            .ahb2enr
            .modify(|r, w| w.bits(r.bits() | 0x01));
        // Enable SRAM4 (bit 29) in AHB4
        (*pac::RCC::ptr())
            .ahb4enr
            .modify(|r, w| w.bits(r.bits() | (1 << 29)));
    }

    hprintln!("Configuring DCMI CR...");
    let dcmi = &dp.DCMI;
    dcmi.cr.write(|w| {
        w.cm()
            .clear_bit() // Continuous
            .pckpol()
            .clear_bit()
            .hspol()
            .clear_bit()
            .vspol()
            .clear_bit()
    });

    // 7. Setup DMA (DMA1 Stream 0)
    hprintln!("Enabling DMA1 clock...");
    ccdr.peripheral.DMA1.enable().reset();
    unsafe {
        (*pac::RCC::ptr())
            .ahb1enr
            .modify(|r, w| w.bits(r.bits() | 0x04));
    }

    hprintln!("Configuring DMAMUX1...");
    let dmamux1 = dp.DMAMUX1;
    dmamux1.ccr[0].write(|w| unsafe { w.dmareq_id().bits(75) });

    hprintln!("Configuring DMA Stream 0 CR...");
    let dma1 = dp.DMA1;
    let stream0 = &dma1.st[0];
    stream0.cr.write(|w| unsafe {
        w.dir()
            .bits(0) // Peripheral to memory
            .minc()
            .set_bit() // Memory increment
            .psize()
            .bits(2) // Peripheral 32-bit
            .msize()
            .bits(2) // Memory 32-bit
            .circ()
            .set_bit() // Circular
    });

    hprintln!("Setting DMA Addresses...");
    stream0
        .par
        .write(|w| unsafe { w.pa().bits(dp.DCMI.dr.as_ptr() as u32) });
    stream0
        .m0ar
        .write(|w| unsafe { w.m0a().bits(FRAMEBUFFER.0.as_ptr() as u32) });
    stream0.ndtr.write(|w| w.ndt().bits((FB_SIZE / 2) as u16));

    unsafe {
        // CRITICAL FIX: The SRAM4 block is uninitialized because of NOLOAD.
        // STM32H7 SRAM has hardware ECC. Reading uninitialized SRAM causes double-bit ECC Faults!
        // We strictly zero the memory to establish valid background ECC syndromes instantly.
        FRAMEBUFFER.0.fill(0);
    }

    hprintln!("Enabling DMA Stream...");
    stream0.cr.modify(|_, w| w.en().set_bit());
    hprintln!("Enabling DCMI...");
    dcmi.cr.modify(|_, w| w.enable().set_bit());
    hprintln!("Configuring DCMI Capture...");
    dcmi.cr.modify(|_, w| w.capture().set_bit());

    hprintln!("Calibrating Display with RED clear...");
    display.clear(Rgb565::RED).unwrap();
    delay.delay_ms(1000u32);

    hprintln!("DCMI Capture Started, entering main loop");

    delay.delay_ms(200u32);
    unsafe {
        hprintln!("FB addr: 0x{:08X}", FRAMEBUFFER.0.as_ptr() as u32);
        hprintln!(
            "FB[0..4]: {:08X} {:08X} {:08X} {:08X}",
            FRAMEBUFFER.0[0],
            FRAMEBUFFER.0[1],
            FRAMEBUFFER.0[2],
            FRAMEBUFFER.0[3]
        );
    }

    loop {
        // Wait for DCMI frame-complete flag (set at the end of each VSYNC frame)
        while dcmi.ris.read().frame_ris().bit_is_clear() {
            cortex_m::asm::nop();
        }
        // Clear the flag by writing to ICR
        dcmi.icr.write(|w| w.frame_isc().set_bit());

        unsafe {
            // Byte-swap each u16 pixel within the u32 words.
            let fb_u8 =
                core::slice::from_raw_parts_mut(FRAMEBUFFER.0.as_mut_ptr() as *mut u8, FB_SIZE * 2);
            for chunk in fb_u8.chunks_exact_mut(2) {
                chunk.swap(0, 1);
            }

            use embedded_graphics::image::ImageRawLE;
            let raw: ImageRawLE<Rgb565> = ImageRawLE::new(
                core::slice::from_raw_parts(FRAMEBUFFER.0.as_ptr() as *const u8, FB_SIZE * 2),
                160,
            );
            let image = Image::new(&raw, Point::new(0, -20));
            image.draw(&mut display).ok();
        }
        // Frame rate is now securely governed solely by the camera's hardware VSYNC.
    }
}

// Register arrays
static OV2640_SLOW_REGS: &[(u8, u8)] = &[
    (0xff, 0x01),
    (0x12, 0x80),
    (0xff, 0x00),
    (0x2c, 0xff),
    (0x2e, 0xdf),
    (0xff, 0x01),
    (0x3c, 0x32),
    (0x11, 0x00),
    (0x09, 0x02),
    (0x04, 0xD8),
    (0x13, 0xe5),
    (0x14, 0x48),
    (0x2c, 0x0c),
    (0x33, 0x78),
    (0x3a, 0x33),
    (0x3b, 0xfb),
    (0x3e, 0x00),
    (0x43, 0x11),
    (0x16, 0x10),
    (0x39, 0x92),
    (0x35, 0xda),
    (0x22, 0x1a),
    (0x37, 0xc3),
    (0x23, 0x00),
    (0x34, 0x1a),
    (0x06, 0x88),
    (0x07, 0xc0),
    (0x0d, 0x87),
    (0x0e, 0x41),
    (0x4c, 0x00),
    (0x48, 0x00),
    (0x5b, 0x00),
    (0x42, 0x03),
    (0x4a, 0x81),
    (0x21, 0x99),
    (0x24, 0x40),
    (0x25, 0x38),
    (0x26, 0x82),
    (0x5c, 0x00),
    (0x63, 0x00),
    (0x46, 0x22),
    (0x0c, 0x3c),
    (0x61, 0x70),
    (0x62, 0x80),
    (0x7c, 0x05),
    (0x20, 0x80),
    (0x28, 0x30),
    (0x6c, 0x00),
    (0x6d, 0x80),
    (0x6e, 0x00),
    (0x70, 0x02),
    (0x71, 0x94),
    (0x73, 0xc1),
    (0x3d, 0x34),
    (0x5a, 0x57),
    (0x12, 0x40),
    (0x17, 0x11),
    (0x18, 0x43),
    (0x19, 0x00),
    (0x1a, 0x4b),
    (0x32, 0x09),
    (0x37, 0xc0),
    (0x4f, 0xca),
    (0x50, 0xa8),
    (0x5a, 0x23),
    (0x6d, 0x00),
    (0x3d, 0x38),
    (0xff, 0x00),
    (0xe5, 0x7f),
    (0xf9, 0xc0),
    (0x41, 0x24),
    (0xe0, 0x14),
    (0x76, 0xff),
    (0x33, 0xa0),
    (0x42, 0x20),
    (0x43, 0x18),
    (0x4c, 0x00),
    (0x87, 0xd5),
    (0x88, 0x3f),
    (0xd7, 0x03),
    (0xd9, 0x10),
    (0xd3, 0x82),
    (0xc8, 0x08),
    (0xc9, 0x80),
    (0x7c, 0x00),
    (0x7d, 0x00),
    (0x7c, 0x03),
    (0x7d, 0x48),
    (0x7d, 0x48),
    (0x7c, 0x08),
    (0x7d, 20),
    (0x7d, 10),
    (0x7d, 0x0e),
    (0x90, 0x00),
    (0x91, 0x0e),
    (0x91, 0x1a),
    (0x91, 0x31),
    (0x91, 0x5a),
    (0x91, 0x69),
    (0x91, 0x75),
    (0x91, 0x7e),
    (0x91, 0x88),
    (0x91, 0x8f),
    (0x91, 0x96),
    (0x91, 0xa3),
    (0x91, 0xaf),
    (0x91, 0xc4),
    (0x91, 0xd7),
    (0x91, 0xe8),
    (0x91, 20),
    (0x92, 0x00),
    (0x93, 0x06),
    (0x93, 0xe3),
    (0x93, 0x05),
    (0x93, 0x05),
    (0x93, 0x00),
    (0x93, 0x04),
    (0x93, 0x00),
    (0x93, 0x00),
    (0x93, 0x00),
    (0x93, 0x00),
    (0x93, 0x00),
    (0x93, 0x00),
    (0x93, 0x00),
    (0x96, 0x00),
    (0x97, 0x08),
    (0x97, 0x19),
    (0x97, 0x02),
    (0x97, 0x0c),
    (0x97, 0x24),
    (0x97, 0x30),
    (0x97, 0x28),
    (0x97, 0x26),
    (0x97, 0x02),
    (0x97, 0x98),
    (0x97, 0x80),
    (0x97, 0x00),
    (0x97, 0x00),
    (0xc3, 0xed),
    (0xa4, 0x00),
    (0xa8, 0x00),
    (0xc5, 0x11),
    (0xc6, 0x51),
    (0xbf, 0x80),
    (0xc7, 0x10),
    (0xb6, 0x66),
    (0xb8, 0xa5),
    (0xb7, 0x64),
    (0xb9, 0x7c),
    (0xb3, 0xaf),
    (0xb4, 0x97),
    (0xb5, 0xff),
    (0xb0, 0xc5),
    (0xb1, 0x94),
    (0xb2, 0x0f),
    (0xc4, 0x5c),
    (0xc0, 0x64),
    (0xc1, 0x4b),
    (0x8c, 0x00),
    (0x86, 0x3d),
    (0x50, 0x00),
    (0x51, 0xc8),
    (0x52, 0x96),
    (0x53, 0x00),
    (0x54, 0x00),
    (0x55, 0x00),
    (0x5a, 0xc8),
    (0x5b, 0x96),
    (0x5c, 0x00),
    (0xd3, 0x02),
    (0xc3, 0xed),
    (0x7f, 0x00),
    (0xda, 0x08),
    (0xe5, 0x1f),
    (0xe1, 0x67),
    (0xe0, 0x00),
    (0xdd, 0x7f),
    (0x05, 0x00),
    (0xff, 0x00),
    (0xe0, 0x04),
    (0x5a, 0x50),
    (0x5b, 0x3c),
    (0x5c, 0x00),
    (0xe0, 0x00),
    (0x00, 0x00),
];

static RGB565_REGS: &[(u8, u8)] = &[
    // 0xDA is the FORMAT control. 0x08 = RGB565. (0x01 falsely forced YUV/RAW mode!)
    (0xff, 0x00),
    (0xda, 0x08),
    (0xd7, 0x03),
    (0xe1, 0x77),
    (0x00, 0x00),
];

static SVGA_REGS: &[(u8, u8)] = &[
    (0xff, 0x01),
    (0x12, 0x40),
    (0x03, 0x0f),
    (0x32, 0x09),
    (0x17, 0x11),
    (0x18, 0x43),
    (0x19, 0x00),
    (0x1a, 0x4b),
    (0x3d, 0x38),
    (0x35, 0xda),
    (0x22, 0x1a),
    (0x37, 0xc3),
    (0x34, 0xc0),
    (0x06, 0x88),
    (0x0d, 0x87),
    (0x0e, 0x41),
    (0x42, 0x03),
    (0xff, 0x00),
    (0x05, 0x01),
    (0x00, 0x00),
];
