//! USB Serial to ST7735 LCD Bridge (USB Terminal)
//!
//! Adapted for WeAct MiniSTM32H750VB.
//! Ported to defmt/RTT for high-speed logging.
//!
//! ### Implementation Details:
//! 1. **USB2 (OTG2_HS)**: CDC-ACM serial on PA11/PA12 (Alternate 10).
//! 2. **SPI4 LCD**: ST7735 display on Port E pins.
//! 3. **Logic**: Displays received USB text on the LCD. Clears screen when full.

#![no_main]
#![no_std]

use core::mem::MaybeUninit;
use panic_probe as _;

use cortex_m_rt::entry;
use defmt_rtt as _;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::*;
use embedded_graphics::text::Text;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_hal_compat::eh1_0::digital::OutputPin;
use embedded_hal_compat::ForwardCompat;
use st7735_lcd::{Orientation, ST7735};
use stm32h7xx_hal::rcc::rec::UsbClkSel;
use stm32h7xx_hal::spi::NoMiso;
use stm32h7xx_hal::usb_hs::{UsbBus, USB2};
use stm32h7xx_hal::{pac, prelude::*, spi};

use usb_device::prelude::*;
use usbd_serial::SerialPort;

static mut EP_MEMORY: MaybeUninit<[u32; 1024]> = MaybeUninit::uninit();

/// Simple LCD Terminal Helper
struct LcdTerminal<D: DrawTarget<Color = Rgb565>> {
    display: D,
    line_y: i32,
    char_x: i32,
}

impl<D: DrawTarget<Color = Rgb565>> LcdTerminal<D>
where
    D::Error: core::fmt::Debug,
{
    pub fn new(display: D) -> Self {
        Self {
            display,
            line_y: 10,
            char_x: 0,
        }
    }

    pub fn print(&mut self, text: &str) {
        let text_style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);

        for c in text.chars() {
            if c == '\n' || c == '\r' {
                self.newline();
                continue;
            }

            // Draw single char (embedded-graphics doesn't wrap easily, so we do it manually)
            let mut s = [0u8; 4];
            let s_str = c.encode_utf8(&mut s);

            Text::new(s_str, Point::new(self.char_x, self.line_y), text_style)
                .draw(&mut self.display)
                .unwrap();

            self.char_x += 6;
            if self.char_x > 150 {
                self.newline();
            }
        }
    }

    fn newline(&mut self) {
        self.char_x = 0;
        self.line_y += 12;
        if self.line_y > 75 {
            self.display.clear(Rgb565::BLACK).unwrap();
            self.line_y = 10;
        }
    }
}

#[entry]
fn main() -> ! {
    let cp = cortex_m::Peripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    // Power
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();

    // RCC
    let rcc = dp.RCC.constrain();
    let mut ccdr = rcc
        .sys_ck(400.MHz())
        .pll1_q_ck(100.MHz()) // SPI4 clock source
        .freeze(pwrcfg, &dp.SYSCFG);

    // 48MHz CLOCK for USB from internal HSI48
    let _ = ccdr.clocks.hsi48_ck().expect("HSI48 must run");
    ccdr.peripheral.kernel_usb_clk_mux(UsbClkSel::Hsi48);

    // Enable the USB voltage regulator (Internal 3.3V power for PHY)
    unsafe {
        let pwr = &*pac::PWR::ptr();
        pwr.cr3.modify(|_, w| w.usb33den().set_bit());
        while pwr.cr3.read().usb33rdy().bit_is_clear() {}
    }

    defmt::info!("USB Serial to LCD Example Started");

    let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);
    let gpioe = dp.GPIOE.split(ccdr.peripheral.GPIOE);

    // --- LCD Setup ---
    let sck = gpioe.pe12.into_alternate();
    let mosi = gpioe.pe14.into_alternate();
    let rst = gpioe.pe15.into_push_pull_output().forward();
    let dc = gpioe.pe13.into_push_pull_output().forward();
    let cs = gpioe.pe11.into_push_pull_output().forward();
    let mut bl = gpioe.pe10.into_push_pull_output().forward();
    bl.set_low().ok(); // Enable Backlight (Active LOW)

    let spi_peripheral = dp
        .SPI4
        .spi(
            (sck, NoMiso, mosi),
            spi::MODE_0,
            10.MHz(),
            ccdr.peripheral.SPI4,
            &ccdr.clocks,
        )
        .forward();

    let spi_dev = ExclusiveDevice::new_no_delay(spi_peripheral, cs).unwrap();
    let mut delay = cortex_m::delay::Delay::new(cp.SYST, ccdr.clocks.sysclk().raw()).forward();

    let mut display = ST7735::new(spi_dev, dc, rst, false, true, 160, 80);
    display.init(&mut delay).unwrap();
    display.set_offset(1, 26);
    display
        .set_orientation(&Orientation::LandscapeSwapped)
        .unwrap();
    display.clear(Rgb565::BLACK).unwrap();

    let mut terminal = LcdTerminal::new(display);
    terminal.print("USB Term Ready\nWait for input...");

    // --- USB Setup ---
    let pin_dm = gpioa.pa11.into_alternate::<10>();
    let pin_dp = gpioa.pa12.into_alternate::<10>();

    let usb = USB2::new(
        dp.OTG2_HS_GLOBAL,
        dp.OTG2_HS_DEVICE,
        dp.OTG2_HS_PWRCLK,
        pin_dm,
        pin_dp,
        ccdr.peripheral.USB2OTG,
        &ccdr.clocks,
    );

    unsafe {
        let buf: &mut [MaybeUninit<u32>; 1024] =
            &mut *(core::ptr::addr_of_mut!(EP_MEMORY) as *mut _);
        for value in buf.iter_mut() {
            value.as_mut_ptr().write(0);
        }
    }

    #[allow(static_mut_refs)]
    let usb_bus = UsbBus::new(usb, unsafe { EP_MEMORY.assume_init_mut() });
    let mut serial = SerialPort::new(&usb_bus);
    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .strings(&[usb_device::device::StringDescriptors::default()
            .manufacturer("WeAct")
            .product("H750 Terminal")
            .serial_number("SN12345")])
        .unwrap()
        .device_class(usbd_serial::USB_CLASS_CDC)
        .build();

    loop {
        if !usb_dev.poll(&mut [&mut serial]) {
            continue;
        }

        let mut buf = [0u8; 64];
        match serial.read(&mut buf) {
            Ok(count) if count > 0 => {
                if let Ok(s) = core::str::from_utf8(&buf[0..count]) {
                    defmt::info!("USB: {:?}", s);
                    terminal.print(s);
                }

                // Echo back in upper case
                for c in buf[0..count].iter_mut() {
                    if (0x61..=0x7a).contains(c) {
                        *c &= !0x20;
                    }
                }
                let mut write_offset = 0;
                while write_offset < count {
                    match serial.write(&buf[write_offset..count]) {
                        Ok(len) if len > 0 => {
                            write_offset += len;
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}
