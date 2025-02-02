use anyhow::Result;
use esp32_nimble::{utilities::BleUuid, uuid128};
use esp_idf_hal::{
    adc::oneshot::{config::AdcChannelConfig, AdcChannelDriver, AdcDriver},
    delay::Delay,
    prelude::Peripherals,
};
use log::{error, info};
use server::Server;
use server::{BleConfig, KickerBle};

mod server;

// consts for BLE functionality
const SERVICE_UUID: BleUuid = uuid128!("c03f245f-d01c-4886-850b-408bc53fe63a");
const CHARACTERISTIC_UUID: BleUuid = uuid128!("03524118-dfd4-40d5-8f28-f81e05442bba");
// const MODE_CHARACTERISTIC_UUID: BleUuid = uuid128!("a436bad4-7cd6-44da-bf2c-bf000b1d1218");
// consts for ADC / photoelectric gate
const THRESHOLD_DETECT_OBJECT: u16 = 50;
// const WAIT_AFTER_DETECTION: Duration = Duration::from_secs(2);

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let delay: Delay = Default::default();

    // set up BLE
    let kicker_server = KickerBle::new(BleConfig {
        service_uuid: SERVICE_UUID,
        characteristic_uuid: CHARACTERISTIC_UUID,
    });

    let peripherals = Peripherals::take()?;

    // https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-reference/peripherals/gpio.html
    // Pin     Mode     Channel         Pin     Mode     Channel
    // Gpio36  Input    ADC1:0          Gpio4   IO       ADC2:0
    // Gpio37  Input    ADC1:1          Gpio0   IO       ADC2:1
    // Gpio38  Input    ADC1:2          Gpio2   IO       ADC2:2
    // Gpio39  Input    ADC1:3          Gpio15  IO       ADC2:3
    // Gpio32  IO       ADC1:4          Gpio13  IO       ADC2:4
    // Gpio33  IO       ADC1:5          Gpio12  IO       ADC2:5
    // Gpio34  Input    ADC1:6          Gpio14  IO       ADC2:6
    // Gpio35  Input    ADC1:7          Gpio27  IO       ADC2:7
    //                                  Gpio25  IO       ADC2:8
    //                                  Gpio26  IO       ADC2:9

    let adc1_driver = AdcDriver::new(peripherals.adc1)?;
    let mut adc_gpio34 = AdcChannelDriver::new(
        &adc1_driver,
        peripherals.pins.gpio34,
        &AdcChannelConfig::default(),
    )?;
    let mut adc_gpio35 = AdcChannelDriver::new(
        &adc1_driver,
        peripherals.pins.gpio35,
        &AdcChannelConfig::default(),
    )?;

    let adc2_driver = AdcDriver::new(peripherals.adc2)?;
    let adc_gpio13 = AdcChannelDriver::new(
        &adc2_driver,
        peripherals.pins.gpio13,
        &AdcChannelConfig::default(),
    )?;
    let adc_gpio14 = AdcChannelDriver::new(
        &adc2_driver,
        peripherals.pins.gpio14,
        &AdcChannelConfig::default(),
    )?;

    loop {
        let x = adc_gpio35.read()?;
        info!("{x}");
        delay.delay_ms(500);
        let goals = x.to_string();
        kicker_server.send(&goals);
    }
}
