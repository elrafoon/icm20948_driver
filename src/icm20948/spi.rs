// Copyright (c) 2022, Zachary D. Olkin.
// This code is provided under the MIT license.

use crate::icm20948::AccLPF;
use crate::icm20948::AccSensitivity;
use crate::icm20948::AccStates::{self, *};
use crate::icm20948::GyroLPF;
use crate::icm20948::GyroSensitivity;
use crate::icm20948::GyroStates::{self, *};
use crate::icm20948::IcmError;
use crate::icm20948::MagStates::{self, *};
use crate::icm20948::RegistersBank0;
use crate::icm20948::RegistersBank2;
use crate::icm20948::{
    ACCEL_SEN_0, ACCEL_SEN_1, ACCEL_SEN_2, ACCEL_SEN_3, GYRO_SEN_0, GYRO_SEN_1, GYRO_SEN_2,
    GYRO_SEN_3, INT_ENABLED, INT_NOT_ENABLED, READ_REG, WRITE_REG,
};
use crate::icm20948::{REG_BANK_0, REG_BANK_2};
use defmt::{Format, Formatter};

use embedded_hal::{self as hal, blocking::spi};

/// The ICM IMU struct is the base of the driver. Instantiate this struct in your application code then use
/// it to interact with the IMU.
pub struct IcmImu<BUS, PIN> {
    bus: BUS,
    acc_en: AccStates,
    gyro_en: GyroStates,
    mag_en: MagStates,

    databuf: [u8; 5],

    accel_sen: u16,
    gyro_sen: f32,

    int_enabled: bool,

    cs: PIN,
}

impl<BUS, E, PIN> IcmImu<BUS, PIN>
where
    BUS: spi::Transfer<u8, Error = E>,
    PIN: hal::digital::v2::OutputPin,
{
    /// Create and initialize a new IMU driver.
    ///
    /// The SPI bus is given as `bus`
    ///
    /// The chip select pin is specified through `cs`
    pub fn new(mut bus: BUS, mut cs: PIN) -> Result<Self, IcmError<E>> {
        let mut buf = [RegistersBank0::PwrMgmt1.get_addr(WRITE_REG), 0x01];

        cs.set_low().ok();
        bus.transfer(&mut buf)?;
        cs.set_high().ok();

        Ok(IcmImu {
            bus,
            acc_en: AccOn,
            gyro_en: GyroOn,
            mag_en: MagOff,
            cs,
            accel_sen: ACCEL_SEN_0,
            gyro_sen: GYRO_SEN_0,
            int_enabled: INT_NOT_ENABLED,
            databuf: [0; 5],
        })
    }

    /// Who Am I? Reads the wai register and reports the value.
    ///
    /// Useful for testing that the IMU is properly connected. See the data sheet for the expected return value.
    pub fn wai(&mut self) -> Result<u8, IcmError<E>> {
        self.cs.set_low().ok();

        self.databuf[0] = RegistersBank0::Wai.get_addr(READ_REG);
        self.databuf[1] = 0;
        self.bus.transfer(&mut self.databuf[0..2])?;

        self.cs.set_high().ok();

        Ok(self.databuf[1])
    }

    /// Enable the raw data ready interrupt.
    pub fn enable_int(&mut self) -> Result<(), IcmError<E>> {
        self.cs.set_low().ok();

        self.databuf[0] = RegistersBank0::IntEnable1.get_addr(WRITE_REG);
        self.databuf[1] = 0x01;
        self.bus.transfer(&mut self.databuf[0..2])?;

        self.cs.set_high().ok();

        self.int_enabled = INT_ENABLED;
        Ok(())
    }

    /// Disable the raw data ready interrupt.
    pub fn disable_int(&mut self) -> Result<(), IcmError<E>> {
        self.cs.set_low().ok();

        self.databuf[0] = RegistersBank0::IntEnable1.get_addr(WRITE_REG);
        self.databuf[1] = 0x00;
        self.bus.transfer(&mut self.databuf[0..2])?;

        self.cs.set_high().ok();

        self.int_enabled = INT_NOT_ENABLED;
        Ok(())
    }

    /// Checks if the interrupt is enabled. Returns true if enabled, false if otherwise.
    pub fn int_on(&self) -> bool {
        self.int_enabled == INT_ENABLED
    }

    /// Enables the accelerometer.
    pub fn enable_acc(&mut self) -> Result<(), IcmError<E>> {
        self.cs.set_low().ok();

        self.databuf[0] = RegistersBank0::PwrMgmt2.get_addr(READ_REG);
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.databuf[1] = self.databuf[0] & 0x07;
        self.databuf[0] = RegistersBank0::PwrMgmt2.get_addr(WRITE_REG);
        self.bus.transfer(&mut self.databuf[0..2])?;

        self.cs.set_high().ok();

        self.acc_en = AccOn;
        defmt::trace!("Accelerometer: On");

        Ok(())
    }

    /// Disables the accelerometer.
    pub fn disable_acc(&mut self) -> Result<(), IcmError<E>> {
        self.cs.set_low().ok();

        self.databuf[0] = RegistersBank0::PwrMgmt2.get_addr(READ_REG);
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.databuf[1] = self.databuf[0] | 0x38;
        self.databuf[0] = RegistersBank0::PwrMgmt2.get_addr(WRITE_REG);
        self.bus.transfer(&mut self.databuf[0..2])?;

        self.cs.set_high().ok();

        self.acc_en = AccOff;
        defmt::trace!("Accelerometer: Off");

        Ok(())
    }

    /// Enables the gyro.
    pub fn enable_gyro(&mut self) -> Result<(), IcmError<E>> {
        self.cs.set_low().ok();

        self.databuf[0] = RegistersBank0::PwrMgmt2.get_addr(READ_REG);
        self.databuf[1] = 0x00;
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.databuf[1] = self.databuf[0] & 0x38;
        self.databuf[0] = RegistersBank0::PwrMgmt2.get_addr(WRITE_REG);
        self.bus.transfer(&mut self.databuf[0..2])?;

        self.cs.set_high().ok();

        self.gyro_en = GyroOn;
        defmt::trace!("Gyro: On");

        Ok(())
    }

    /// Disables the gyro.
    pub fn disable_gyro(&mut self) -> Result<(), IcmError<E>> {
        self.cs.set_low().ok();

        self.databuf[0] = RegistersBank0::PwrMgmt2.get_addr(READ_REG);
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.databuf[1] = self.databuf[0] | 0x07;
        self.databuf[0] = RegistersBank0::PwrMgmt2.get_addr(WRITE_REG);
        self.bus.transfer(&mut self.databuf[0..2])?;

        self.cs.set_high().ok();

        self.gyro_en = GyroOff;
        defmt::trace!("Gyro: Off");

        Ok(())
    }

    /// Checks if the gyro is enabled. Returns true if enabled, false if otherwise.
    pub fn gyro_on(&self) -> bool {
        self.gyro_en == GyroOn
    }

    /// Checks if the accelerometer is enabled. Returns true if enabled, false if otherwise.
    pub fn acc_on(&self) -> bool {
        self.acc_en == AccOn
    }

    /// Checks if the magnetometer is enabled. Returns true if enabled, false if otherwise.
    /// NOTE: Currently the magnetometer cannot be used.
    pub fn mag_on(&self) -> bool {
        self.mag_en == MagOn
    }

    /// Gets the accelerometer reading in the X direction.
    pub fn read_acc_x(&mut self) -> Result<f32, IcmError<E>> {
        if self.acc_en != AccOn {
            self.enable_acc()?;
        }

        self.databuf[0] = RegistersBank0::AccelXOutH.get_addr(READ_REG);
        self.databuf[1] = RegistersBank0::AccelXOutL.get_addr(READ_REG);

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..3])?;
        self.cs.set_high().ok();

        Ok(
            (((self.databuf[1] as i16) << 8) | (self.databuf[2] as i16)) as f32
                / self.accel_sen as f32,
        )
    }

    /// Gets the accelerometer reading in the Y direction.
    pub fn read_acc_y(&mut self) -> Result<f32, IcmError<E>> {
        if self.acc_en != AccOn {
            self.enable_acc()?;
        }

        self.databuf[0] = RegistersBank0::AccelYOutH.get_addr(READ_REG);
        self.databuf[1] = RegistersBank0::AccelYOutL.get_addr(READ_REG);

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..3])?;
        self.cs.set_high().ok();

        Ok(
            (((self.databuf[1] as i16) << 8) | (self.databuf[2] as i16)) as f32
                / self.accel_sen as f32,
        )
    }

    /// Gets the accelerometer reading in the Z direction.
    pub fn read_acc_z(&mut self) -> Result<f32, IcmError<E>> {
        if self.acc_en != AccOn {
            self.enable_acc()?;
        }

        self.databuf[0] = RegistersBank0::AccelZOutH.get_addr(READ_REG);
        self.databuf[1] = RegistersBank0::AccelZOutL.get_addr(READ_REG);

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..3])?;
        self.cs.set_high().ok();

        Ok(
            (((self.databuf[1] as i16) << 8) | (self.databuf[2] as i16)) as f32
                / self.accel_sen as f32,
        )
    }

    /// Reads all three accelerometer values and returns them as an array.
    pub fn read_acc(&mut self) -> Result<[f32; 3], IcmError<E>> {
        let mut res = [0.0; 3];
        res[0] = self.read_acc_x()?;
        res[1] = self.read_acc_y()?;
        res[2] = self.read_acc_z()?;

        Ok(res)
    }

    /// Gets the gyro reading in the X direction.
    pub fn read_gyro_x(&mut self) -> Result<f32, IcmError<E>> {
        if self.gyro_en != GyroOn {
            self.enable_acc()?;
        }

        self.databuf[0] = RegistersBank0::GyroXOutH.get_addr(READ_REG);
        self.databuf[1] = RegistersBank0::GyroXOutL.get_addr(READ_REG);

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..3])?;
        self.cs.set_high().ok();

        Ok((((self.databuf[1] as i16) << 8) | (self.databuf[2] as i16)) as f32 / self.gyro_sen)
    }

    /// Gets the gyro reading in the Y direction.
    pub fn read_gyro_y(&mut self) -> Result<f32, IcmError<E>> {
        if self.gyro_en != GyroOn {
            self.enable_acc()?;
        }

        self.databuf[0] = RegistersBank0::GyroYOutH.get_addr(READ_REG);
        self.databuf[1] = RegistersBank0::GyroYOutL.get_addr(READ_REG);

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..3])?;
        self.cs.set_high().ok();

        Ok((((self.databuf[1] as i16) << 8) | (self.databuf[2] as i16)) as f32 / self.gyro_sen)
    }

    /// Gets the gyro reading in the Z direction.
    pub fn read_gyro_z(&mut self) -> Result<f32, IcmError<E>> {
        if self.gyro_en != GyroOn {
            self.enable_acc()?;
        }

        self.databuf[0] = RegistersBank0::GyroZOutH.get_addr(READ_REG);
        self.databuf[1] = RegistersBank0::GyroZOutL.get_addr(READ_REG);

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..3])?;
        self.cs.set_high().ok();

        Ok((((self.databuf[1] as i16) << 8) | (self.databuf[2] as i16)) as f32 / self.gyro_sen)
    }

    /// Reads all three gyro values and returns them as an array.
    pub fn read_gyro(&mut self) -> Result<[f32; 3], IcmError<E>> {
        let mut res = [0.0; 3];
        res[0] = self.read_gyro_x()?;
        res[1] = self.read_gyro_y()?;
        res[2] = self.read_gyro_z()?;

        Ok(res)
    }

    /// Sets the sensitivity of the accelerometer.
    ///
    /// `acc_sen` specifies the desired sensitivity. See the data sheet for more details.
    pub fn set_acc_sen(&mut self, acc_sen: AccSensitivity) -> Result<(), IcmError<E>> {
        self.change_bank(REG_BANK_2)?;

        self.databuf[2] = RegistersBank2::AccelConfig.get_addr(READ_REG);

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[2..4])?;
        self.cs.set_high().ok();

        match acc_sen {
            AccSensitivity::Sen2g => self.databuf[1] = self.databuf[3] & 0xF9,
            AccSensitivity::Sen4g => self.databuf[1] = (self.databuf[3] & 0xFB) | 0x02,
            AccSensitivity::Sen8g => self.databuf[1] = (self.databuf[3] & 0xFD) | 0x04,
            AccSensitivity::Sen16g => self.databuf[1] = self.databuf[3] | 0x06,
        };

        self.databuf[0] = RegistersBank2::AccelConfig.get_addr(WRITE_REG);

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.cs.set_high().ok();

        self.change_bank(REG_BANK_0)?;

        match acc_sen {
            AccSensitivity::Sen2g => self.accel_sen = ACCEL_SEN_0,
            AccSensitivity::Sen4g => self.accel_sen = ACCEL_SEN_1,
            AccSensitivity::Sen8g => self.accel_sen = ACCEL_SEN_2,
            AccSensitivity::Sen16g => self.accel_sen = ACCEL_SEN_3,
        }
        Ok(())
    }

    /// Sets the sensitivity of the gyro.
    ///
    /// `gyro_sen` specifies the desired sensitivity. See the data sheet for more details.
    pub fn set_gyro_sen(&mut self, gyro_sen: GyroSensitivity) -> Result<(), IcmError<E>> {
        self.change_bank(REG_BANK_2)?;

        self.databuf[2] = RegistersBank2::GyroConfig1.get_addr(READ_REG);

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[2..4])?;
        self.cs.set_high().ok();

        match gyro_sen {
            GyroSensitivity::Sen250dps => self.databuf[1] = self.databuf[3] & 0xF9,
            GyroSensitivity::Sen500dps => self.databuf[1] = (self.databuf[3] & 0xFB) | 0x02,
            GyroSensitivity::Sen1000dps => self.databuf[1] = (self.databuf[3] & 0xFD) | 0x04,
            GyroSensitivity::Sen2000dps => self.databuf[1] = self.databuf[3] | 0x06,
        };

        self.databuf[0] = RegistersBank2::GyroConfig1.get_addr(WRITE_REG);

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.cs.set_high().ok();

        self.change_bank(REG_BANK_0)?;

        match gyro_sen {
            GyroSensitivity::Sen250dps => self.gyro_sen = GYRO_SEN_0,
            GyroSensitivity::Sen500dps => self.gyro_sen = GYRO_SEN_1,
            GyroSensitivity::Sen1000dps => self.gyro_sen = GYRO_SEN_2,
            GyroSensitivity::Sen2000dps => self.gyro_sen = GYRO_SEN_3,
        }

        Ok(())
    }

    /// Configures the Low Pass Filter (LPF) for the accelerometer.
    ///
    /// `bw` is the 3DB bandwidth of the LPF. See the data sheet for more details.
    pub fn config_acc_lpf(&mut self, bw: AccLPF) -> Result<(), IcmError<E>> {
        self.change_bank(REG_BANK_2)?;

        self.databuf[0] = RegistersBank2::AccelConfig.get_addr(READ_REG);

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.cs.set_high().ok();

        let mask: u8 = 0x38;

        match bw {
            AccLPF::BW6 => self.databuf[1] = (self.databuf[1] & !mask) | ((6 << 3) & mask),
            AccLPF::BW11 => self.databuf[1] = (self.databuf[1] & !mask) | ((5 << 3) & mask),
            AccLPF::BW24 => self.databuf[1] = (self.databuf[1] & !mask) | ((4 << 3) & mask),
            AccLPF::BW50 => self.databuf[1] = (self.databuf[1] & !mask) | ((3 << 3) & mask),
            AccLPF::BW111 => self.databuf[1] = (self.databuf[1] & !mask) | ((2 << 3) & mask),
            AccLPF::BW246 => self.databuf[1] = (self.databuf[1] & !mask) | ((0 << 3) & mask),
            AccLPF::Bw473 => self.databuf[1] = (self.databuf[1] & !mask) | ((7 << 3) & mask),
        }

        self.databuf[1] = self.databuf[1] | 0x01;

        self.databuf[0] = RegistersBank2::AccelConfig.get_addr(WRITE_REG);

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.cs.set_high().ok();

        self.change_bank(REG_BANK_0)?;
        Ok(())
    }

    /// Configures the Low Pass Filter (LPF) for the gyro.
    ///
    /// `bw` is the 3DB bandwidth of the LPF. See the data sheet for more details.
    pub fn config_gyro_lpf(&mut self, bw: GyroLPF) -> Result<(), IcmError<E>> {
        self.change_bank(REG_BANK_2)?;

        self.databuf[0] = RegistersBank2::GyroConfig1.get_addr(READ_REG);

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.cs.set_high().ok();

        let mask: u8 = 0x38;

        match bw {
            GyroLPF::BW6 => self.databuf[1] = (self.databuf[1] & !mask) | ((6 << 3) & mask),
            GyroLPF::BW12 => self.databuf[1] = (self.databuf[1] & !mask) | ((5 << 3) & mask),
            GyroLPF::BW24 => self.databuf[1] = (self.databuf[1] & !mask) | ((4 << 3) & mask),
            GyroLPF::BW51 => self.databuf[1] = (self.databuf[1] & !mask) | ((3 << 3) & mask),
            GyroLPF::BW119 => self.databuf[1] = (self.databuf[1] & !mask) | ((2 << 3) & mask),
            GyroLPF::BW152 => self.databuf[1] = (self.databuf[1] & !mask) | ((1 << 3) & mask),
            GyroLPF::BW197 => self.databuf[1] = (self.databuf[1] & !mask) | ((0 << 3) & mask),
            GyroLPF::BW361 => self.databuf[1] = (self.databuf[1] & !mask) | ((7 << 3) & mask),
        }

        self.databuf[1] = self.databuf[1] | 0x01;

        self.databuf[0] = RegistersBank2::GyroConfig1.get_addr(WRITE_REG);

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.cs.set_high().ok();

        self.change_bank(REG_BANK_0)?;
        Ok(())
    }

    /// Disables the low pass filter for the accelerometer.
    pub fn disable_acc_lpf(&mut self) -> Result<(), IcmError<E>> {
        self.change_bank(REG_BANK_2)?;

        self.databuf[0] = RegistersBank2::AccelConfig.get_addr(READ_REG);
        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.cs.set_high().ok();

        self.databuf[0] = RegistersBank2::AccelConfig.get_addr(WRITE_REG);
        self.databuf[1] = self.databuf[1] & 0xFE;

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.cs.set_high().ok();

        self.change_bank(REG_BANK_0)?;

        Ok(())
    }

    /// Disables the low pass filter for the gyro.
    pub fn disable_gyro_lpf(&mut self) -> Result<(), IcmError<E>> {
        self.change_bank(REG_BANK_2)?;

        self.databuf[0] = RegistersBank2::GyroConfig1.get_addr(READ_REG);
        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.cs.set_high().ok();

        self.databuf[0] = RegistersBank2::GyroConfig1.get_addr(WRITE_REG);
        self.databuf[1] = self.databuf[1] & 0xFE;

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.cs.set_high().ok();

        self.change_bank(REG_BANK_0)?;

        Ok(())
    }

    /// Configure the data rate for the accelerometer.
    ///
    /// Rate must be less than 1.125kHz since that is the maximum of the accelerometer. Rate is specified in Hz.
    /// If rate is > 1.125kHz then an InvalidInput will be returned.
    /// Since the divisor is specified as an integer, the exact rate may not be met if it would require a floating point divisor.
    ///
    /// Note that while the gyro is enabled the gyro ODR (output data rate) will determine the interrupt frequency.
    /// If the accelerometer frequency is lower than the gyro frequency then it will not be updated at every interrupt.
    pub fn config_acc_rate(&mut self, rate: u16) -> Result<(), IcmError<E>> {
        if rate < 1_125 {
            let div = 1_125 / (rate) - 1;

            self.change_bank(REG_BANK_2)?;

            self.databuf[0] = RegistersBank2::OdrAlignEn.get_addr(WRITE_REG);
            self.databuf[1] = 0x01;

            self.cs.set_low().ok();
            self.bus.transfer(&mut self.databuf[0..2])?;
            self.cs.set_high().ok();

            self.databuf[0] = RegistersBank2::AccelSmplrtDiv1.get_addr(WRITE_REG);
            self.databuf[1] = div.to_be_bytes()[0];
            self.databuf[2] = RegistersBank2::AccelSmplrtDiv2.get_addr(WRITE_REG);
            self.databuf[3] = div.to_be_bytes()[1];

            self.cs.set_low().ok();
            self.bus.transfer(&mut self.databuf[0..2])?;
            self.cs.set_high().ok();

            self.cs.set_low().ok();
            self.bus.transfer(&mut self.databuf[2..4])?;
            self.cs.set_high().ok();

            self.change_bank(REG_BANK_0)?;

            Ok(())
        } else {
            Err(IcmError::InvalidInput)
        }
    }

    /// Configure the data rate for the gyro.
    ///
    /// Rate must be less than 1.125kHz since that is the maximum of the gyro. Rate is specified in Hz.
    /// If rate is > 1.1kHz then an InvalidInput will be returned.
    /// Since the divisor is specified as an integer, the exact rate may not be met if it would require a floating point divisor.
    ///
    /// Note that while the gyro is enabled the gyro ODR (output data rate) will determine the interrupt frequency.
    pub fn config_gyro_rate(&mut self, rate: u16) -> Result<(), IcmError<E>> {
        if rate > 4 {
            let div: u8 = (1_125 / (rate) - 1) as u8;

            self.change_bank(REG_BANK_2)?;

            self.databuf[0] = RegistersBank2::OdrAlignEn.get_addr(WRITE_REG);
            self.databuf[1] = 0x01;

            self.cs.set_low().ok();
            self.bus.transfer(&mut self.databuf[0..2])?;
            self.cs.set_high().ok();

            self.databuf[0] = RegistersBank2::GyroSmplrtDiv.get_addr(WRITE_REG);
            self.databuf[1] = div;

            self.cs.set_low().ok();
            self.bus.transfer(&mut self.databuf[0..2])?;
            self.cs.set_high().ok();

            self.change_bank(REG_BANK_0)?;

            Ok(())
        } else {
            Err(IcmError::InvalidInput)
        }
    }

    /// Configure the accelerometer data rate by directly modifying the sample rate divider.
    ///
    /// The user should calculate what the resulting data rate will be before using this function.
    ///
    /// div specifies the divider and must be less than 4096.
    pub fn config_acc_rate_div(&mut self, mut div: u16) -> Result<(), IcmError<E>> {
        if div < 4096 {
            div = div - 1;

            self.change_bank(REG_BANK_2)?;

            self.databuf[0] = RegistersBank2::OdrAlignEn.get_addr(WRITE_REG);
            self.databuf[1] = 0x01;

            self.cs.set_low().ok();
            self.bus.transfer(&mut self.databuf[0..2])?;
            self.cs.set_high().ok();

            self.databuf[0] = RegistersBank2::AccelSmplrtDiv1.get_addr(WRITE_REG);
            self.databuf[1] = div.to_be_bytes()[0];
            self.databuf[2] = RegistersBank2::AccelSmplrtDiv2.get_addr(WRITE_REG);
            self.databuf[3] = div.to_be_bytes()[1];

            self.cs.set_low().ok();
            self.bus.transfer(&mut self.databuf[0..2])?;
            self.cs.set_high().ok();

            self.cs.set_low().ok();
            self.bus.transfer(&mut self.databuf[2..4])?;
            self.cs.set_high().ok();

            self.change_bank(REG_BANK_0)?;

            Ok(())
        } else {
            Err(IcmError::InvalidInput)
        }
    }

    /// Configure the gyroscope data rate by directly modifying the sample rate divider.
    ///
    /// The user should calculate what the resulting data rate will be before using this function.
    ///
    /// div specifies the divider to use.
    pub fn config_gyro_rate_div(&mut self, mut div: u8) -> Result<(), IcmError<E>> {
        div = div - 1;

        self.change_bank(REG_BANK_2)?;

        self.databuf[0] = RegistersBank2::OdrAlignEn.get_addr(WRITE_REG);
        self.databuf[1] = 0x01;

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.cs.set_high().ok();

        self.databuf[0] = RegistersBank2::GyroSmplrtDiv.get_addr(WRITE_REG);
        self.databuf[1] = div;

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.cs.set_high().ok();

        self.change_bank(REG_BANK_0)?;

        Ok(())
    }

    /// Resets the IMU the wakes it up from sleep mode.
    /// After the reset a 20ms sleep is suggested.
    ///
    /// TODO: Unstable and NOT suggested for use.
    pub fn reset(&mut self) -> Result<(), IcmError<E>> {
        self.databuf[0] = RegistersBank0::PwrMgmt1.get_addr(WRITE_REG);
        self.databuf[1] = 0x80;

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.cs.set_high().ok();

        // TODO: Replace with a delay
        let mut j = 0;
        for i in 1..2000000 {
            j = j + i/10000;
        }

        self.databuf[0] = RegistersBank0::PwrMgmt1.get_addr(WRITE_REG);
        self.databuf[1] = 0x01;

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.cs.set_high().ok();

        Ok(())
    }

    fn change_bank(&mut self, bank: u8) -> Result<(), IcmError<E>> {
        self.databuf[0] = RegistersBank0::RegBankSel.get_addr(WRITE_REG);
        self.databuf[1] = bank;

        self.cs.set_low().ok();
        self.bus.transfer(&mut self.databuf[0..2])?;
        self.cs.set_high().ok();

        Ok(())
    }
}

impl<BUS, PIN> Format for IcmImu<BUS, PIN> {
    fn format(&self, fmt: Formatter) {
        defmt::write!(fmt, "ICM-20948 IMU")
    }
}
