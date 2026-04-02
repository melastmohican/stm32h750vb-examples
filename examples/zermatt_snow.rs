//! # Zermatt Image Display with Falling Snow Effect (STM32H750VB + SD Card)
//!
//! Display a 320x240 image of Zermatt on an external ILI9341 2.2" TFT LCD display
//! with animated falling snow, powered by the STM32H7's hardware RNG.
//!
//! Due to the 128KB internal FLASH limit, the **ZERMATT.BMP** asset is loaded from
//! a MicroSD card (FAT32) via the onboard SDMMC1 peripheral.
//!
//! ## Hardware: Adafruit 2.2" TFT SPI 240x320 Display (Product 1480)
//!
//! ## Wiring for WeAct MiniSTM32H750VB (SPI2)
//!
//! | LCD Pin | STM32H750 Pin | Note |
//! | :--- | :--- | :--- |
//! | **VCC** | 3.3V | |
//! | **GND** | GND | |
//! | **SCK** | **PB13** | SPI2 SCK (AF5) |
//! | **MOSI** | **PB15** | SPI2 MOSI (AF5) |
//! | **CS** | **PB12** | Chip Select |
//! | **DC** | **PD11** | Data/Command |
//! | **RESET** | **PD10** | Reset |
//!
//! ## Wiring for Onboard MicroSD (SDMMC1)
//!
//! | Signal | STM32 Pin | Function |
//! | :--- | :--- | :--- |
//! | **D0-D3** | PC8-PC11 | Data Bus (AF12) |
//! | **CMD** | PD2 | Command (AF12) |
//! | **CK** | PC12 | Clock (AF12) |
//!
//! Run with `cargo run --example zermatt_snow`.

#![no_std]
#![no_main]

use cortex_m_rt::entry;
use defmt::{error, info, warn, Debug2Format};
use defmt_rtt as _;
use display_interface_spi::SPIInterface;
use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{Point, Size},
    image::{GetPixel, Image},
    pixelcolor::{Rgb565, RgbColor},
    Drawable,
};
use embedded_hal::delay::DelayNs;
use embedded_hal_bus::spi::ExclusiveDevice;
use embedded_hal_compat::ForwardCompat;
use embedded_sdmmc::{Controller, Mode, TimeSource, Timestamp, VolumeIdx};
use mipidsi::{
    models::ILI9341Rgb565,
    options::{ColorOrder, Orientation, Rotation},
    Builder,
};
use panic_probe as _;
use stm32h750vb_examples::compat::DelayEh1;
use stm32h7xx_hal::{
    pac,
    prelude::*,
    sdmmc::{SdCard, Sdmmc},
};
use tinybmp::Bmp;

// Display dimensions in landscape mode
const DISPLAY_WIDTH: usize = 320;
const DISPLAY_HEIGHT: usize = 240;

// Physics engine grid size
const PHY_DISP_RATIO: usize = 2; // Physical cell size in pixels
const PHY_WIDTH: usize = DISPLAY_WIDTH / PHY_DISP_RATIO;
const PHY_HEIGHT: usize = DISPLAY_HEIGHT / PHY_DISP_RATIO;

// Grid storage (1 bit per cell)
const BITS_PER_CELL: usize = 1;
const CELLS_PER_BYTE: usize = 8 / BITS_PER_CELL;
const GRID_TOTAL_CELLS: usize = PHY_WIDTH * PHY_HEIGHT;
const GRID_SIZE_BYTES: usize = GRID_TOTAL_CELLS / CELLS_PER_BYTE;

const FLAKE_SIZE: i32 = 2; // Small 2x2 pixel snowflakes
const SNOW_COLOR: Rgb565 = Rgb565::WHITE;

struct SnowGrid {
    grid: [u8; GRID_SIZE_BYTES],
}

impl SnowGrid {
    const fn new() -> Self {
        Self {
            grid: [0u8; GRID_SIZE_BYTES],
        }
    }

    fn get_cell(&self, row: usize, col: usize) -> bool {
        let cell_index = row * PHY_WIDTH + col;
        let byte_index = cell_index / CELLS_PER_BYTE;
        let bit_index = cell_index % CELLS_PER_BYTE;
        (self.grid[byte_index] >> bit_index) & 1 == 1
    }

    fn set_cell(&mut self, row: usize, col: usize, value: bool) {
        let cell_index = row * PHY_WIDTH + col;
        let byte_index = cell_index / CELLS_PER_BYTE;
        let bit_index = cell_index % CELLS_PER_BYTE;

        if value {
            self.grid[byte_index] |= 1 << bit_index;
        } else {
            self.grid[byte_index] &= !(1 << bit_index);
        }
    }

    fn clear(&mut self) {
        self.grid.fill(0);
    }
}

static mut SNOW_GRID: SnowGrid = SnowGrid::new();

// Allocate 256KB in AXISRAM to hold the BMP data from SD card
#[link_section = ".axisram"]
static mut BMP_BUFFER: [u8; 256 * 1024] = [0u8; 256 * 1024];

struct DummyTimeSource;
impl TimeSource for DummyTimeSource {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 54, // 2024
            zero_indexed_month: 2,
            zero_indexed_day: 28,
            hours: 12,
            minutes: 0,
            seconds: 0,
        }
    }
}

#[entry]
fn main() -> ! {
    let cp = pac::CorePeripherals::take().unwrap();
    let dp = pac::Peripherals::take().unwrap();

    // --- Configure PWR and RCC ---
    let pwr = dp.PWR.constrain();
    let pwrcfg = pwr.freeze();
    let rcc = dp.RCC.constrain();
    let ccdr = rcc
        .sys_ck(400.MHz())
        .pll1_q_ck(100.MHz()) // SDMMC clock source
        .freeze(pwrcfg, &dp.SYSCFG);

    // --- Initialize Delay and Wrap for EH 1.0 ---
    let delay = cp.SYST.delay(ccdr.clocks);
    let mut delay_eh1 = DelayEh1(delay);

    info!("Initializing Zermatt Snow example with SD card asset loading...");

    // --- GPIO Configuration ---
    let gpiob = dp.GPIOB.split(ccdr.peripheral.GPIOB);
    let gpioc = dp.GPIOC.split(ccdr.peripheral.GPIOC);
    let gpiod = dp.GPIOD.split(ccdr.peripheral.GPIOD);

    // 1. Configure SDMMC1 Pins (Onboard slot)
    let clk = gpioc
        .pc12
        .into_alternate::<12>()
        .speed(stm32h7xx_hal::gpio::Speed::VeryHigh);
    let cmd = gpiod
        .pd2
        .into_alternate::<12>()
        .internal_pull_up(true)
        .speed(stm32h7xx_hal::gpio::Speed::VeryHigh);
    let d0 = gpioc
        .pc8
        .into_alternate::<12>()
        .internal_pull_up(true)
        .speed(stm32h7xx_hal::gpio::Speed::VeryHigh);
    let d1 = gpioc
        .pc9
        .into_alternate::<12>()
        .internal_pull_up(true)
        .speed(stm32h7xx_hal::gpio::Speed::VeryHigh);
    let d2 = gpioc
        .pc10
        .into_alternate::<12>()
        .internal_pull_up(true)
        .speed(stm32h7xx_hal::gpio::Speed::VeryHigh);
    let d3 = gpioc
        .pc11
        .into_alternate::<12>()
        .internal_pull_up(true)
        .speed(stm32h7xx_hal::gpio::Speed::VeryHigh);

    // 2. Initialize SDMMC1 Peripheral
    info!("Mounting MicroSD card...");
    let mut sdmmc: Sdmmc<pac::SDMMC1, SdCard> = dp.SDMMC1.sdmmc(
        (clk, cmd, d0, d1, d2, d3),
        ccdr.peripheral.SDMMC1,
        &ccdr.clocks,
    );

    // Wait for card to be ready
    let bus_frequency = 400.kHz();
    while let Err(e) = sdmmc.init(bus_frequency) {
        warn!("Waiting for SD card... ({:?})", Debug2Format(&e));
        delay_eh1.delay_ms(1000);
    }
    info!("SD card initialized! Switching to integrated adapter...");

    // 3. Initialize Filesystem Controller
    let mut controller = Controller::new(sdmmc.sdmmc_block_device(), DummyTimeSource);
    let mut volume = controller
        .get_volume(VolumeIdx(0))
        .expect("Failed to get volume");
    let root_dir = controller
        .open_root_dir(&volume)
        .expect("Failed to open root dir");

    // 4. Load ZERMATT.BMP into AXISRAM
    info!("Loading ZERMATT.BMP from SD card...");
    let mut file = controller
        .open_file_in_dir(&mut volume, &root_dir, "ZERMATT.BMP", Mode::ReadOnly)
        .expect("Failed to open ZERMATT.BMP. Please ensure it is in the root directory.");

    let file_size = file.length();
    info!("File size: {} bytes", file_size);

    if file_size > unsafe { core::ptr::addr_of!(BMP_BUFFER).read().len() as u32 } {
        error!("File is too large for the AXISRAM buffer!");
        panic!("Memory overflow");
    }

    let bmp_data = unsafe {
        let bytes_read = controller
            .read(
                &volume,
                &mut file,
                &mut *core::ptr::addr_of_mut!(BMP_BUFFER),
            )
            .expect("Failed to read image data");
        info!("Read {} bytes into AXISRAM", bytes_read);
        &BMP_BUFFER[0..bytes_read]
    };
    controller.close_file(&volume, file).ok();

    // 5. Configure SPI2 Display Pins
    let spi2_sck = gpiob.pb13.into_alternate::<5>();
    let spi2_mosi = gpiob.pb15.into_alternate::<5>();
    let cs = gpiob.pb12.into_push_pull_output().forward();
    let dc = gpiod.pd11.into_push_pull_output().forward();
    let rst = gpiod.pd10.into_push_pull_output().forward();

    // 6. Initialize SPI2
    let spi = dp
        .SPI2
        .spi(
            (spi2_sck, stm32h7xx_hal::spi::NoMiso, spi2_mosi),
            stm32h7xx_hal::spi::MODE_0,
            40.MHz(),
            ccdr.peripheral.SPI2,
            &ccdr.clocks,
        )
        .forward();

    // 7. Hardware RNG for snowflake generation
    let mut rng = dp.RNG.constrain(ccdr.peripheral.RNG, &ccdr.clocks);

    // 8. Create display interface and initialize mipidsi
    let spi_device = ExclusiveDevice::new_no_delay(spi, cs).unwrap();
    let di = SPIInterface::new(spi_device, dc);

    info!("Initializing display logic...");
    let mut display = Builder::new(ILI9341Rgb565, di)
        .reset_pin(rst)
        .display_size(240, 320)
        .orientation(Orientation::new().rotate(Rotation::Deg90).flip_horizontal())
        .color_order(ColorOrder::Bgr)
        .init(&mut delay_eh1)
        .unwrap();

    display.clear(Rgb565::BLACK).unwrap();

    // 9. Load BMP from RAM buffer
    let bmp = Bmp::<Rgb565>::from_slice(bmp_data).expect("Failed to parse BMP from RAM");
    info!("Drawing Zermatt image...");
    Image::new(&bmp, Point::new(0, 0))
        .draw(&mut display)
        .unwrap();
    info!("Animation starts!");

    // Initialize/clear snow grid
    let snow_grid = unsafe {
        let grid_ptr = core::ptr::addr_of_mut!(SNOW_GRID);
        (*grid_ptr).clear();
        &mut *grid_ptr
    };

    let mut frame_count = 0u32;
    loop {
        // Falling snow logic
        for row in (0..PHY_HEIGHT - 1).rev() {
            for col in 0..PHY_WIDTH {
                if snow_grid.get_cell(row, col) {
                    let rand_val: u32 = rng.gen().unwrap_or(0);
                    let offset = (rand_val % 3) as i32 - 1;
                    let future_col =
                        (col as i32 + offset).max(0).min(PHY_WIDTH as i32 - 1) as usize;

                    if !snow_grid.get_cell(row + 1, future_col) {
                        snow_grid.set_cell(row + 1, future_col, true);
                        render_flake(&mut display, row + 1, future_col);
                        snow_grid.set_cell(row, col, false);
                        render_void(&mut display, &bmp, row, col);
                    }
                }
            }
        }

        // Clear bottom row
        for col in 0..PHY_WIDTH {
            if snow_grid.get_cell(PHY_HEIGHT - 1, col) {
                snow_grid.set_cell(PHY_HEIGHT - 1, col, false);
                render_void(&mut display, &bmp, PHY_HEIGHT - 1, col);
            }
        }

        // New snow at top
        for col in 0..PHY_WIDTH {
            let rand_val: u32 = rng.gen().unwrap_or(0);
            if rand_val.is_multiple_of(25) && !snow_grid.get_cell(0, col) {
                snow_grid.set_cell(0, col, true);
                render_flake(&mut display, 0, col);
            }
        }

        delay_eh1.delay_ms(10);
        frame_count += 1;
        if frame_count.is_multiple_of(50) {
            info!("Frame: {}", frame_count);
        }
    }
}

fn render_flake(display: &mut impl DrawTarget<Color = Rgb565>, row: usize, col: usize) {
    let x = (col * PHY_DISP_RATIO) as i32;
    let y = (row * PHY_DISP_RATIO) as i32;
    use embedded_graphics::primitives::{PrimitiveStyle, Rectangle, StyledDrawable};
    Rectangle::new(
        Point::new(x, y),
        Size::new(FLAKE_SIZE as u32, FLAKE_SIZE as u32),
    )
    .draw_styled(&PrimitiveStyle::with_fill(SNOW_COLOR), display)
    .ok();
}

fn render_void(
    display: &mut impl DrawTarget<Color = Rgb565>,
    bmp: &Bmp<Rgb565>,
    row: usize,
    col: usize,
) {
    let x = (col * PHY_DISP_RATIO) as i32;
    let y = (row * PHY_DISP_RATIO) as i32;
    use embedded_graphics::primitives::Rectangle;
    let area = Rectangle::new(
        Point::new(x, y),
        Size::new(FLAKE_SIZE as u32, FLAKE_SIZE as u32),
    );
    display
        .fill_contiguous(
            &area,
            (0..FLAKE_SIZE).flat_map(|dy| {
                (0..FLAKE_SIZE).map(move |dx| {
                    bmp.pixel(Point::new(x + dx, y + dy))
                        .unwrap_or(Rgb565::BLACK)
                })
            }),
        )
        .ok();
}
