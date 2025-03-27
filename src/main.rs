use anyhow::Result;
use esp32_nimble::{utilities::BleUuid, uuid128};
use esp_idf_hal::{
    adc::oneshot::{config::AdcChannelConfig, AdcChannelDriver, AdcDriver},
    adc::{ADC1, ADC2},
    gpio::{Gpio13, Gpio14, Gpio34, Gpio35},
    prelude::Peripherals,
};
use log::{error, info};
use server::Server;
use server::{BleConfig, KickerBle};
use std::{
    fmt::Display,
    time::{Duration, Instant},
};

mod server;

// consts for BLE functionality
const SERVICE_UUID: BleUuid = uuid128!("c03f245f-d01c-4886-850b-408bc53fe63a");
const CHARACTERISTIC_UUID: BleUuid = uuid128!("03524118-dfd4-40d5-8f28-f81e05442bba");
const IR_THRESH_UUID: BleUuid = uuid128!("e468f847-4ee5-4928-8b8f-413cb8086c2c");
// const MODE_CHARACTERISTIC_UUID: BleUuid = uuid128!("a436bad4-7cd6-44da-bf2c-bf000b1d1218");
// consts for ADC / photoelectric gate
const THRESHOLD_DETECT_OBJECT: u16 = 50;
const WAIT_AFTER_DETECTION: Duration = Duration::from_secs(2);

struct SensorArray<'a> {
    adc_gpio34: AdcChannelDriver<'a, Gpio34, &'a AdcDriver<'a, ADC1>>, // home
    adc_gpio35: AdcChannelDriver<'a, Gpio35, &'a AdcDriver<'a, ADC1>>, // home
    adc_gpio13: AdcChannelDriver<'a, Gpio13, &'a AdcDriver<'a, ADC2>>, // away
    adc_gpio14: AdcChannelDriver<'a, Gpio14, &'a AdcDriver<'a, ADC2>>, // away
}

#[derive(Default)]
enum DetectedGoal {
    #[default]
    None,
    Home,
    Away,
}

impl Display for DetectedGoal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DetectedGoal::Home => write!(f, "Home"),
            DetectedGoal::Away => write!(f, "Away"),
            DetectedGoal::None => write!(f, "None"),
        }
    }
}

struct GoalDetector<'a> {
    last_goal: std::time::Instant,
    sensors: SensorArray<'a>,
}

impl<'a> GoalDetector<'a> {
    fn new(sensors: SensorArray<'a>) -> Self {
        Self {
            last_goal: Instant::now(),
            sensors,
        }
    }

    fn home_triggered(&mut self) -> Result<bool> {
        Ok(self.sensors.adc_gpio34.read()? < THRESHOLD_DETECT_OBJECT
            || self.sensors.adc_gpio35.read()? < THRESHOLD_DETECT_OBJECT)
    }

    fn away_triggered(&mut self) -> Result<bool> {
        Ok(self.sensors.adc_gpio13.read()? < THRESHOLD_DETECT_OBJECT
            || self.sensors.adc_gpio14.read()? < THRESHOLD_DETECT_OBJECT)
    }

    pub fn scan(&mut self) -> DetectedGoal {
        match (
            self.last_goal.elapsed() >= WAIT_AFTER_DETECTION,
            self.home_triggered(),
            self.away_triggered(),
        ) {
            (true, Ok(true), _) => DetectedGoal::Home,
            (true, _, Ok(true)) => DetectedGoal::Away,
            (_, Err(e), _) => {
                error!("Error reading home sensor: {:?}", e);
                DetectedGoal::None
            }
            (_, _, Err(e)) => {
                error!("Error reading away sensor: {:?}", e);
                DetectedGoal::None
            }
            _ => DetectedGoal::None,
        }
    }
}

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    // set up BLE
    let kicker_server = KickerBle::new(BleConfig {
        service_uuid: SERVICE_UUID,
        goals_uuid: CHARACTERISTIC_UUID,
        ir_threshold_uuid: IR_THRESH_UUID,
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
    let adc_gpio34: AdcChannelDriver<
        '_,
        esp_idf_hal::gpio::Gpio34,
        &AdcDriver<'_, esp_idf_hal::adc::ADC1>,
    > = AdcChannelDriver::new(
        &adc1_driver,
        peripherals.pins.gpio34,
        &AdcChannelConfig::default(),
    )?;
    let adc_gpio35 = AdcChannelDriver::new(
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

    let mut goal_detector = GoalDetector::new(SensorArray {
        adc_gpio34,
        adc_gpio35,
        adc_gpio13,
        adc_gpio14,
    });

    loop {
        match goal_detector.scan() {
            DetectedGoal::None => (),
            goal => {
                goal_detector.last_goal = Instant::now();
                kicker_server.send(&goal.to_string());
                info!("Detected goal: {:?}", goal.to_string());
            }
        }
    }
}
