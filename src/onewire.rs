// onewire.rs

use embedded_hal::digital::{InputPin, OutputPin};
use esp_idf_hal::{
    delay::{Ets, FreeRtos},
    gpio::{self, Pull},
};
use one_wire_bus::{Address, OneWire, OneWireError};

use crate::*;

// When performing a measurement it can happen that no device was found on the one-wire-bus
// in addition to the bus errors. Therefore we extend the error cases for proper error handling.
#[derive(Debug)]
pub enum MeasurementError<E> {
    OneWireError(OneWireError<E>),
    NoDeviceFound,
}

pub async fn measure_temperature<P, E>(
    one_wire_bus: &mut OneWire<P>,
    addr: &Address,
    max_retry: u32,
) -> Result<f32, MeasurementError<E>>
where
    P: OutputPin<Error=E> + InputPin<Error=E>,
    E: std::fmt::Debug,
{
    let sensor = ds18b20::Ds18b20::new::<E>(addr.to_owned())?;
    sensor.set_config(i8::MIN, i8::MAX, ds18b20::Resolution::Bits12, one_wire_bus, &mut Ets)?;
    sleep(Duration::from_millis(50)).await; // extra sleep
    sensor.start_temp_measurement(one_wire_bus, &mut Ets)?;
    ds18b20::Resolution::Bits12.delay_for_measurement_time(&mut FreeRtos);
    sleep(Duration::from_millis(10)).await; // extra sleep

    // Quite often we have to retry, CrcMismatch is observed occasionally
    let mut retries = 0;
    let mut meas = -999.0;
    loop {
        match sensor.read_data(one_wire_bus, &mut Ets) {
            Ok(data) => {
                meas = data.temperature;
                info!("Sensor {addr:?} retry#{retries}: {meas} °C");
                break;
            }
            Err(e) => {
                retries += 1;
                error!("Sensor {addr:?} read error: {e:?}");
                if retries > max_retry {
                    break;
                }
            }
        }
        sleep(Duration::from_millis(100)).await; // extra sleep
    }
    sleep(Duration::from_millis(100)).await;

    if meas < -100.0 {
        Err(MeasurementError::NoDeviceFound)
    } else {
        Ok(meas)
    }
}

pub fn scan_1wire<P, E>(one_wire_bus: &mut OneWire<P>) -> Result<Address, MeasurementError<E>>
where
    P: OutputPin<Error=E> + InputPin<Error=E>,
{
    let state = None;
    if let Some((addr, _state)) = one_wire_bus.device_search(state, false, &mut Ets)? {
        Ok(addr)
    } else {
        Err(MeasurementError::NoDeviceFound)
    }
}

impl<E> From<OneWireError<E>> for MeasurementError<E> {
    fn from(value: OneWireError<E>) -> Self {
        MeasurementError::OneWireError(value)
    }
}

pub async fn poll_sensor(state: Arc<Pin<Box<MyState>>>) -> anyhow::Result<()> {
    if !state.config.sensor_enable {
        info!("Sensor is disabled.");
        // we cannot return, otherwise tokio::select in main() will exit
        loop {
            sleep(Duration::from_secs(3600)).await;
        }
    }

    sleep(Duration::from_secs(60)).await;
    let mut onewire_pin = state.onewire_pin.write().await.take().unwrap().onewire;
    let onewire_addr = &state.onewire_addr;

    loop {
        sleep(Duration::from_secs(60)).await;
        info!("Polling 1-wire sensors");

        {
            let mut pin_drv = gpio::PinDriver::input_output_od(&mut onewire_pin).unwrap();
            pin_drv.set_pull(Pull::Up).unwrap();
            let mut w = OneWire::new(pin_drv).unwrap();

            match Box::pin(measure_temperature(&mut w, onewire_addr, 5)).await {
                Ok(meas) => {
                    info!("Onewire response {onewire_addr:?}:\n{meas:#?}");
                    *state.meas.write().await = meas;
                    *state.meas_updated.write().await = true;
                }
                Err(e) => {
                    error!("Temp read error: {e:#?}");
                }
            }
        }
    }
}
// EOF
