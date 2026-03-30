//! STM32H750VB Compatibility Library
//!
//! This library provides minimal wrappers to bridge the stm32h7xx-hal (v0.2)
//! to the modern embedded-hal (v1.0) traits required by latest sensor crates.

#![no_std]

pub mod compat {
    use stm32h7xx_hal::hal;

    // --- I2C Wrapper ---

    #[derive(Debug)]
    pub struct I2cError<E>(pub E);

    impl<E: core::fmt::Debug> embedded_hal::i2c::Error for I2cError<E> {
        fn kind(&self) -> embedded_hal::i2c::ErrorKind {
            embedded_hal::i2c::ErrorKind::Other
        }
    }

    pub struct I2cEh1<I2C>(pub I2C);

    impl<I2C, E> embedded_hal::i2c::ErrorType for I2cEh1<I2C>
    where
        I2C: hal::blocking::i2c::Write<Error = E>
            + hal::blocking::i2c::Read<Error = E>
            + hal::blocking::i2c::WriteRead<Error = E>,
        E: core::fmt::Debug,
    {
        type Error = I2cError<E>;
    }

    impl<I2C, E> embedded_hal::i2c::I2c for I2cEh1<I2C>
    where
        I2C: hal::blocking::i2c::Write<Error = E>
            + hal::blocking::i2c::Read<Error = E>
            + hal::blocking::i2c::WriteRead<Error = E>,
        E: core::fmt::Debug,
    {
        fn read(&mut self, _address: u8, read: &mut [u8]) -> Result<(), Self::Error> {
            self.0.read(_address, read).map_err(I2cError)
        }

        fn write(&mut self, _address: u8, write: &[u8]) -> Result<(), Self::Error> {
            self.0.write(_address, write).map_err(I2cError)
        }

        fn write_read(
            &mut self,
            _address: u8,
            write: &[u8],
            read: &mut [u8],
        ) -> Result<(), Self::Error> {
            self.0.write_read(_address, write, read).map_err(I2cError)
        }

        fn transaction(
            &mut self,
            address: u8,
            operations: &mut [embedded_hal::i2c::Operation<'_>],
        ) -> Result<(), Self::Error> {
            for op in operations {
                match op {
                    embedded_hal::i2c::Operation::Read(read) => {
                        self.0.read(address, read).map_err(I2cError)?
                    }
                    embedded_hal::i2c::Operation::Write(write) => {
                        self.0.write(address, write).map_err(I2cError)?
                    }
                }
            }
            Ok(())
        }
    }

    // --- Delay Wrapper ---

    pub struct DelayEh1<D>(pub D);

    impl<D> embedded_hal::delay::DelayNs for DelayEh1<D>
    where
        D: hal::blocking::delay::DelayMs<u32> + hal::blocking::delay::DelayUs<u32>,
    {
        fn delay_ns(&mut self, ns: u32) {
            self.0.delay_us(ns.div_ceil(1000));
        }

        fn delay_us(&mut self, us: u32) {
            self.0.delay_us(us);
        }

        fn delay_ms(&mut self, ms: u32) {
            self.0.delay_ms(ms);
        }
    }
}
