//! CDC-ACM serial port example using polling in a busy loop
//!
//! Adapted for WeAct MiniSTM32H750VB.
//! Ported to defmt/RTT for high-speed logging.
//!
//! Usage:
//! 1. Connect the USB-C port to your computer.
//! 2. Open a serial terminal.
//! 3. Type characters — they will be echoed back in uppercase.

#![no_main]
#![no_std]

use core::mem::MaybeUninit;
use panic_probe as _;

use cortex_m_rt::entry;
use defmt_rtt as _;
use stm32h7xx_hal::rcc::rec::UsbClkSel;
use stm32h7xx_hal::usb_hs::{UsbBus, USB2}; // Use USB2 for PA11/PA12 on H743/H750
use stm32h7xx_hal::{pac, prelude::*};

use usb_device::prelude::*;
use usbd_serial::SerialPort;

static mut EP_MEMORY: MaybeUninit<[u32; 1024]> = MaybeUninit::uninit();

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();

    // Power
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();

    // RCC
    let rcc = dp.RCC.constrain();
    let mut ccdr = rcc.sys_ck(400.MHz()).freeze(pwrcfg, &dp.SYSCFG);

    // 48MHz CLOCK for USB from internal HSI48
    let _ = ccdr.clocks.hsi48_ck().expect("HSI48 must run");
    ccdr.peripheral.kernel_usb_clk_mux(UsbClkSel::Hsi48);

    // Enable the USB voltage regulator (Internal 3.3V)
    unsafe {
        let pwr = &*pac::PWR::ptr();
        pwr.cr3.modify(|_, w| w.usb33den().set_bit());
        while pwr.cr3.read().usb33rdy().bit_is_clear() {}
    }

    defmt::info!("USB Serial Example Started (Polling)");

    // IO Pins for OTG2_HS (PA11/PA12)
    // For rm0433 chips (H743/H750), PA11/PA12 are USB2 in Alternate 10.
    let gpioa = dp.GPIOA.split(ccdr.peripheral.GPIOA);
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

    // Initialise EP_MEMORY to zero
    unsafe {
        let buf: &mut [MaybeUninit<u32>; 1024] =
            &mut *(core::ptr::addr_of_mut!(EP_MEMORY) as *mut _);
        for value in buf.iter_mut() {
            value.as_mut_ptr().write(0);
        }
    }

    // Now we may assume that EP_MEMORY is initialised
    #[allow(static_mut_refs)]
    let usb_bus = UsbBus::new(usb, unsafe { EP_MEMORY.assume_init_mut() });

    let mut serial = SerialPort::new(&usb_bus);

    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .strings(&[usb_device::device::StringDescriptors::default()
            .manufacturer("WeAct")
            .product("MiniSTM32H750VB Serial")
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
                    defmt::info!("Read {} bytes: {:?}", count, s);
                } else {
                    defmt::info!("Read {} bytes (hex): {:X}", count, &buf[0..count]);
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
