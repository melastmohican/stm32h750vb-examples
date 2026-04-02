# stm32h750vb-examples

This project explores the **WeAct Studio MiniSTM32H7xx** development board (featuring the STM32H750VBT6) using Rust and the `stm32h7xx-hal`. It provides a collection of working examples ranging from simple GPIO manipulation to high-speed SPI display drivers.

## Hardware

The [WeAct Studio MiniSTM32H7xx](https://github.com/WeActStudio/MiniSTM32H7xx) is a powerful, compact development board based on the high-performance STM32H750VBT6 microcontroller.

- **MCU**: STM32H750VBT6 (ARM Cortex-M7 @ 480MHz)
- **Memory**: 128KB Internal Flash, 1MB RAM
- **Peripherals**: High-speed SPI, QSPI Flash, USB-C, 2 LEDs, 1 User Button.
- **Form Factor**: Small "Black Pill" style with dual-row headers.

<img src="https://github.com/WeActStudio/MiniSTM32H7xx/raw/master/Images/STM32H750VB_2.jpg" width="50%" alt="WeAct STM32H750 Board">

## Documentation

- [WeAct MiniSTM32H7xx Official Repository](https://github.com/WeActStudio/MiniSTM32H7xx)
- [stm32h7xx-hal Documentation](https://docs.rs/stm32h7xx-hal/latest/stm32h7xx_hal/)
- [STM32H750VB Reference Manual (RM0433)](https://www.st.com/resource/en/reference_manual/rm0433-stm32h742-stm32h743753-and-stm32h750-value-line-advanced-armbased-32bit-mcus-stmicroelectronics.pdf)

## Examples

The examples demonstrate various features of the board and external peripherals.

---

### Core Examples

#### blinky (main)

The entry-point firmware (`src/main.rs`) toggles the green User LED on the board at a fixed 1Hz rate.

```bash
cargo run
```

**Hardware:**

- WeAct MiniSTM32H7xx Board
- LED (Green): Managed via `PE3`

#### button_poll

A basic polling-based button example. It reads the state of the K1 User Button and sets the LED state accordingly.

```bash
cargo run --example button_poll
```

**Hardware:**

- Button (K1): PC13
- LED: PE3

#### button_int (Interrupts)

Demonstrates the use of **EXTI** (External Interrupts) to toggle the LED on a button press. It includes a 5-second "settle" delay for the floating PC13 pin.

```bash
cargo run --example button_int
```

---

### Low-Level Peripheral Examples

#### blinky_random

Uses the MCU's internal hardware **Random Number Generator (RNG)** to toggle the onboard LED at irregular intervals (50ms - 500ms).

```bash
cargo run --example blinky_random
```

**Hardware:**

- Peripheral: RNG (True Random Source)
- LED: PE3
- MCU: STM32H750VBT6

#### i2c_scan (Multi-Bus)

Scans multiple I2C buses for connected devices. It uses a **conditional logic** on **PA8**:
1. Starts **PA8** as **MCO1** (16MHz Clock) to wake up the camera on I2C1.

2. Scans **I2C1** (PB8/PB9).

3. If no device is found on I2C1, it reconfigures PA8 as **I2C3_SCL** and scans **I2C3** (PA8/PC9).

4. Also scans **I2C2** (PB10/PB11) and **I2C4** (PD12/PD13).

```bash
cargo run --example i2c_scan
```

#### bme280_i2c

Standalone BME280 sensor example. Reads temperature, humidity, and pressure and logs it to the terminal.

```bash
cargo run --example bme280_i2c
```

#### ssd1306

Standalone SSD1306 OLED example. Displays a 1-bit black and white image (`rustbw.bmp`).

```bash
cargo run --example ssd1306
```

#### ssd1306_text

Standalone SSD1306 OLED example. Demonstrates drawing text, lines, rectangles, and circles.

```bash
cargo run --example ssd1306_text
```

#### bme280_ssd1306_i2c (Combined)

A combined environmental sensor and display example. It reads data from a **BME280** and renders it to an **SSD1306** OLED.

```bash
cargo run --example bme280_ssd1306_i2c
```

**Hardware:**

- Bus: I2C2 (Shared via `embedded-hal-bus`)
- Pins: PB10 (SCL), PB11 (SDA)
- Sensor: BME280 (0x77/0x76)
- Display: SSD1306 (0x3C/0x3D)

---

### Analog Peripheral Examples

#### mcutemp

Combines the internal temperature sensor (**ADC3**) with the onboard **ST7735 LCD** (via SPI4 and `mipidsi`). It renders the live temperature value in white text on a black background, updating every second.

```bash
cargo run --example mcutemp
```

**Hardware:**

- Peripheral: ADC3 (Internal) + SPI4 (LCD)
- MCU: STM32H750VBT6
- Display: ST7735 (160x80)

---

### SPI Display Examples

These examples demonstrate driving TFT displays using the **SPI4** peripheral (built-in LCD) or the **SPI2** peripheral (external GC9A01 display). Due to the limited 128KB internal FLASH, these examples are optimized for size.

#### ov2640_lcd

The system is now streaming stable, color-accurate video at ~15 FPS. Video feed was verified via user media and serial logs confirming DMA NDTR alignment.

![ov2640_lcd](ov2640_lcd.png)

```bash
cargo run --example ov2640_lcd
```

#### mipidsi

Demonstrates advanced ST7735 driver usage via the `mipidsi` crate, including custom rotation and color inversion.

```bash
cargo run --example mipidsi
```

**Hardware:**

- Display: ST7735 (160x80)
- Driver: `mipidsi` crate

#### gc9a01_spi

Renders images on a 240x240 **round** GC9A01 display using the modern `mipidsi` driver.

```bash
cargo run --example gc9a01_spi
```

#### gc9a01_spi_text

Demonstrates text rendering, shapes (circles), and clock-like radiating lines on the round GC9A01 display.

```bash
cargo run --example gc9a01_spi_text
```

#### zermatt_snow

Renders a high-quality 320x240 image of Zermatt with dynamic falling snow physics. This example demonstrates:
- **External Asset Loading**: Loads a 225KB BMP from a MicroSD card to bypass the 128KB internal FLASH limit.
- **AXISRAM Usage**: Buffers the large image in the 512KB AXISRAM (D1 Domain) for fast access.
- **Hardware RNG**: Uses the internal RNG for authentic snowflake drift and generation.

```bash
cargo run --example zermatt_snow
```

**Hardware:**

- **Display**: Adafruit 2.2" TFT SPI Display (Product 1480).
- **Storage**: Onboard MicroSD slot (requires `ZERMATT.BMP` on a FAT32 card).
- **Bus**: SPI2 (Display) + SDMMC1 (Storage).

#### dino_game (Chrome Dino)

A playable "Chrome Dino" clone optimized for the STM32H750. It features:
- **60 FPS** smooth gameplay.
- **Fixed-point physics** for precise movement.
- **Optimized SPI rendering** using `fill_contiguous`.

```bash
cargo run --example dino_game
```

**Controls:**

- **K1 (PC13)**: Jump
- **PA0**: Jump (External button to GND)

---

### Storage Examples

#### sdmmc

Demonstrates initialization of the onboard MicroSD card reader using the SDMMC1 peripheral. It performs a raw block verification followed by a FAT filesystem test (listing the directory and writing a file).

```bash
cargo run --example sdmmc
```

**Hardware:**

- Peripheral: SDMMC1 (4-bit mode)
- Driver: `embedded-sdmmc` crate
- Storage: MicroSD Card

---

### USB Serial Examples

These examples implement USB CDC-ACM (Serial) communication using the onboard **USB-C** connector. They use the **USB2 (OTG2_HS)** peripheral mapped to **PA11/PA12**.

#### usb_serial

A basic polling-based USB echo server.

```bash
cargo run --example usb_serial
```

#### usb_rtic_serial

An interrupt-driven USB echo server built with the **RTIC** framework.

```bash
cargo run --example usb_rtic_serial
```

#### rtic_button

Demonstrates handling the User Button (K1) via the **RTIC** framework tasks.

```bash
cargo run --example rtic_button
```

#### usb_serial_lcd (Terminal)

A combined example that takes incoming USB serial data and renders it as a scrolling terminal on the **ST7735 LCD**.

```bash
cargo run --example usb_serial_lcd
```

**Testing Instructions (macOS):**

1. Connect the board via USB-C.
2. Find the device path: `ls /dev/cu.usbmodem*`
3. Connect using `screen` or `tio`:
   ```bash
   # Using screen
   screen /dev/cu.usbmodemSN123451 115200

   # Using tio (recommended)
   tio /dev/cu.usbmodemSN123451
   ```

4. Type characters to see them echoed back (in uppercase).
5. For `usb_serial_lcd`, typed characters will appear on the physical display.

6. **How to exit**:
   - For `screen`: Press `Ctrl+A` then `K`.
   - For `tio`: Press `Ctrl+T` then `Q`.

**Hardware:**
- Peripheral: USB2 (OTG2_HS)
- Pins: PA11 (DM), PA12 (DP)
- LCD (Optional): SPI4 (PE11-PE15)

## Implementation Architecture

### 1. embedded-hal v1.0 Compatibility

Since `stm32h7xx-hal` v0.16.x currently supports the older **embedded-hal v0.2.x** traits, this project includes a minimal compatibility layer in `src/lib.rs`.

- **Pattern**: Wraps HAL types in a `compat` struct implemented for EH v1.0.
- **Benefit**: Allows the use of modern, high-quality sensor crates (like `bme280` v0.5 and `ssd1306` v0.9) that require the new trait standard without waiting for upstream HAL updates.

### 2. Memory Coherency (MPU & SRAM4)

On Cortex-M7 (STM32H7), DMA and the CPU Cache compete for data consistency.

- **Pattern**: Configure the MPU to mark the D2/D3 SRAM regions (where DMA resides) as **Non-Cacheable** and **Shareable**.

- **Benefit**: Eliminates the need for manual `invalidate_dcache` calls which can cause `Imprecise BusFaults` if addresses are not 32-byte aligned.

### 2. DCMI & DMA Configuration

- **Peripheral**: DCMI (Digital Camera Interface) in 8-bit parallel mode.
- **DMA**: DMA1 Stream 0 with DMAMUX ID 75.
- **Packing**: DCMI packs received bytes into 32-bit words. Since pixels are 16-bit, bit-packing alignment issues (the "colorful sand" effect) must be handled.

### 3. Signal Synchronization (VSYNC Polling)

- **Problem**: Fixed-loop delays (e.g., `delay_ms(16)`) drift relative to the camera's frame rate, causing tearing.
- **Solution**: Explicitly poll the DCMI `frame_ris` bit. Clear it via `icr` only after the frame is fully processed.

## Design Decisions

### 1. SPI Display Driver Standardization

- **Decision**: Standardize all SPI display examples on the **`mipidsi`** crate.
- **Rationale**: The `mipidsi` crate is more modern, actively maintained, and provides better support for various rotations and color formats compared to the older `st7735-lcd` crate.
- **Result**: The `st7735_lcd` example was removed as it became redundant once all logic was migrated to use `mipidsi`.

## Hardware Gotchas & Troubleshooting

### Power Rail Instability (PA7/PF1/PF0)

- **Issue**: Toggling Camera PWDN or RESET pins too early or too frequently can cause voltage sags that crash the LCD controller (Black Screen).
- **Solution**: Rely on board pull-ups/pull-downs. Leave these pins floating or in their reset state to prevent electrical ripples.

### Backlight Polarity

- **Issue**: Some WeAct modules use **Active-LOW** backlight circuits.
- **Solution**: If the screen is black but code is running, verify if the backlight pin (PE10) needs to be driven `LOW` to turn on.

### Pixel Format ("Colorful Sand")

- **Symptom**: Recognizable shapes but with stochastic noise/grain.
- **Cause**: Byte-swapping at 16-bit boundaries.
- **Fix**: Perform a manual `chunk.swap(0, 1)` on the raw `u8` framebuffer before drawing.

### Psychedelic Colors (YUV vs RGB)

- **Symptom**: Solid colors clear correctly, but the camera feed looks like thermal imaging or a negative.
- **Cause**: OV2640 Register `0xDA` (Format Control).
- **Fix**: Ensure `0xDA` is set to `0x08` for RGB565. Values like `0x01` or `0x02` force YUV/RAW output.

### SPI1/2/3 Kernel Clock (PLL1_Q)

- **Issue**: Attempting to initialize SPI1, SPI2, or SPI3 without enabling `PLL1_Q` causes an immediate panic.
- **Solution**: These peripherals (unlike SPI4/5) require an explicit kernel clock source. Ensure `.pll1_q_ck()` is called in your RCC configuration:

```rust
let ccdr = rcc.sys_ck(400.MHz()).pll1_q_ck(48.MHz()).freeze(pwrcfg, &dp.SYSCFG);
```

---

### Shared Wiring (ST7735 Display)

Both SPI display examples use the following pin mapping for **SPI4**:

| ST7735 LCD | STM32H750 Pin | Note                 |
| :--------- | :------------ | :------------------- |
| VCC        | 3.3V          |                      |
| GND        | GND           |                      |
| SCK        | PE12          | SPI4 SCK             |
| SDA (MOSI) | PE14          | SPI4 MOSI            |
| CS         | PE11          | Chip Select          |
| DC         | PE13          | Data/Command         |
| RES (RST)  | PE15          | Reset                |
| LED        | PE10          | Backlight (optional) |

### External GC9A01 Wiring (SPI2)

The `gc9a01` examples use **SPI2** to avoid conflicts with the built-in LCD:

| GC9A01 LCD | STM32H750 Pin | Note                 |
| :--------- | :------------ | :------------------- |
| VCC        | 3.3V          |                      |
| GND        | GND           |                      |
| SCL (SCK)  | **PB13**      | SPI2 SCK (AF5)       |
| SDA (MOSI) | **PB15**      | SPI2 MOSI (AF5)      |
| CS         | **PB12**      | Chip Select          |
| DC         | **PD11**      | Data/Command         |
| RESET      | **PD10**      | Reset                |

### Hardware RNG Support

### Camera Wiring (OV2640 DVP)

The `ov2640_lcd` example uses the following DCMI/I2C/Clock mapping:

| OV2640 Camera | STM32H750 Pin | Description                 |
| :------------ | :------------ | :-------------------------- |
| XVCLK         | PA8           | Camera Master Clock (MCO1)  |
| SIO_C (SCL)   | PB8           | SCCB Clock (I2C1 + Int PU)  |
| SIO_D (SDA)   | PB9           | SCCB Data (I2C1 + Int PU)   |
| VSYNC         | PB7           | Vertical Sync               |
| HSYNC         | PA4           | Horizontal Sync             |
| PCLK          | PA6           | Pixel Timing Clock          |
| D7            | PE6           | DCMI Data 7                 |
| D6            | PE5           | DCMI Data 6                 |
| D5            | PD3           | DCMI Data 5                 |
| D4            | PE4           | DCMI Data 4                 |
| D3            | PE1           | DCMI Data 3                 |
| D2            | PE0           | DCMI Data 2                 |
| D1            | PC7           | DCMI Data 1                 |
| D0            | PC6           | DCMI Data 0                 |
| RESET / PWDN  | PA7           | Shared Power Control (Opt.) |

### Storage Wiring (MicroSD)

The `sdmmc` example uses the onboard MicroSD slot connected to **SDMMC1**:

| Signal | STM32H750 Pin | Description    |
| :----- | :------------ | :------------- |
| D0     | PC8           | Data 0 (AF12)  |
| D1     | PC9           | Data 1 (AF12)  |
| D2     | PC10          | Data 2 (AF12)  |
| D3     | PC11          | Data 3 (AF12)  |
| CMD    | PD2           | Command (AF12) |
| CK     | PC12          | Clock (AF12)   |

### USB Wiring (OTG2_HS)

The USB-C connector is wired directly to the following pins:

| Signal | STM32H750 Pin | Alternate Function |
| :----- | :------------ | :----------------- |
| USB_DM | PA11          | AF10               |
| USB_DP | PA12          | AF10               |

> [!IMPORTANT]
> The internal 3.3V USB regulator (`usb33den`) and HSI48 clock must be enabled for these pins to function as a USB device.

## Build Configuration

Special build profiles are used to ensure the binaries fit in the 128KB internal flash:

- **Size Optimization**: `opt-level = "s"` is used in the `dev` profile for the crate and all dependencies.
- **Linker**: Uses `rust-lld` as defined in `.cargo/config.toml`.

## References

- [WeAct Studio MiniSTM32H7xx Repository](https://github.com/WeActStudio/MiniSTM32H7xx)
- [WeAct Studio STM32H750 Schematic (PDF)](https://github.com/WeActStudio/MiniSTM32H7xx/blob/master/Hardware/STM32H7xx%20SchDoc%20V12.pdf)
- [Arduino WeActMiniH7xx Variant](https://github.com/stm32duino/Arduino_Core_STM32/blob/main/variants/STM32H7xx/H742V(G-I)(H-T)_H743V(G-I)(H-T)_H750VBT_H753VI(H-T)/variant_WeActMiniH7xx.h)
- [Zephyr Mini STM32H7B0 Documentation](https://docs.zephyrproject.org/latest/boards/weact/mini_stm32h7b0/doc/index.html)
- [Zephyr OV2640 Module Documentation](https://docs.zephyrproject.org/latest/boards/shields/weact_ov2640_cam_module/doc/index.html)
- [NuttX WeAct STM32H743 Documentation](https://nuttx.apache.org/docs/latest/platforms/arm/stm32h7/boards/weact-stm32h743/index.html)
- [OV2640 Datasheet](https://www.uctronics.com/download/cam_module/OV2640DS.pdf)
- [stm32h7xx-hal examples](https://github.com/stm32-rs/stm32h7xx-hal/tree/master/examples)
- [Dinosaur Game](https://en.wikipedia.org/wiki/Dinosaur_Game)