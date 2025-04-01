use anyhow::Result;
use esp32_nimble::{utilities::BleUuid, uuid128};
use esp_idf_hal::{
    adc::oneshot::{config::AdcChannelConfig, AdcChannelDriver, AdcDriver},
    cpu::Core,
    prelude::Peripherals,
    task::thread::ThreadSpawnConfiguration,
};
use goal_detector::{DetectedGoal, GoalDetector};
use lazy_static::lazy_static;
use log::{error, info};
use sensor::{SensorArray, ThreshValue};
use server::Server;
use server::{BleConfig, KickerBle};
use std::{
    sync::{Arc, Mutex},
    thread,
};

mod goal_detector;
mod sensor;
mod server;

const SCANNER_CORE: Core = Core::Core0;
const SCANNER_THREAD_NAME: Option<&[u8]> = Some(b"ir_scanner\0");
const SCANNER_THREAD_PRIORITY: u8 = 10;

const SERVER_CORE: Core = Core::Core1;
const SERVER_THREAD_NAME: Option<&[u8]> = Some(b"kickir_server\0");
const SERVER_THREAD_PRIORITY: u8 = 5;

lazy_static! {
    static ref IR_THRESHOLD_HOME: Arc<Mutex<ThreshValue>> = Arc::new(Mutex::new(50));
    static ref IR_THRESHOLD_AWAY: Arc<Mutex<ThreshValue>> = Arc::new(Mutex::new(50));
}

// consts for BLE functionality
const SERVICE_UUID: BleUuid = uuid128!("c03f245f-d01c-4886-850b-408bc53fe63a");
const CHARACTERISTIC_UUID: BleUuid = uuid128!("03524118-dfd4-40d5-8f28-f81e05442bba");
const IR_THRESH_UUID: BleUuid = uuid128!("e468f847-4ee5-4928-8b8f-413cb8086c2c");
// const MODE_CHARACTERISTIC_UUID: BleUuid = uuid128!("a436bad4-7cd6-44da-bf2c-bf000b1d1218");
// consts for ADC / photoelectric gate

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let pins: esp_idf_hal::gpio::Pins = peripherals.pins;

    info!("Starting kickir BLE server...");
    // set up BLE
    let kicker_server = KickerBle::new(BleConfig {
        service_uuid: SERVICE_UUID,
        goals_uuid: CHARACTERISTIC_UUID,
        ir_threshold_uuid: IR_THRESH_UUID,
    });

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

    let (goal_tx, goal_rx) = std::sync::mpsc::channel();

    ThreadSpawnConfiguration {
        name: SCANNER_THREAD_NAME,
        priority: SCANNER_THREAD_PRIORITY,
        pin_to_core: Some(SCANNER_CORE), // same as the watchdog
        ..Default::default()
    }
    .set()?;

    // This thread uses one-shot ADC read-outs for now. ADC1 can be used in continuous mode, which could make
    // the necessary code simpler:
    //   - set continuous mode to read 10k samples/sec into a buffer with size 100
    //   - trigger interrupt when buffer has been filled
    //   - ISR checks buffer for values under a theshold (to detect goals) ... this can be done easily with idiomatic Rust
    //   - send correct notification to BLE server
    // But alas, it is unclear how to implement this:
    // https://stackoverflow.com/questions/76330918/wait-for-adc-interrupts-with-embedded-rust-and-embassy-on-stm32-microcontroller
    let scanner_thread = thread::Builder::new()
        .stack_size(8192)
        .spawn(move || -> Result<()> {
            let adc1_driver = AdcDriver::new(peripherals.adc1)?;
            let adc2_driver = AdcDriver::new(peripherals.adc2)?;
            let adc_gpio34: AdcChannelDriver<
                '_,
                esp_idf_hal::gpio::Gpio34,
                &AdcDriver<'_, esp_idf_hal::adc::ADC1>,
            > = AdcChannelDriver::new(&adc1_driver, pins.gpio34, &AdcChannelConfig::default())?;
            let adc_gpio35 =
                AdcChannelDriver::new(&adc1_driver, pins.gpio35, &AdcChannelConfig::default())?;
            let adc_gpio13 =
                AdcChannelDriver::new(&adc2_driver, pins.gpio13, &AdcChannelConfig::default())?;
            let adc_gpio14 =
                AdcChannelDriver::new(&adc2_driver, pins.gpio14, &AdcChannelConfig::default())?;

            let mut goal_detector = GoalDetector::new(SensorArray {
                adc_gpio34,
                adc_gpio35,
                adc_gpio13,
                adc_gpio14,
            });

            #[cfg(feature = "scan_log")]
            let mut last_scan_log = Instant::now() - Duration::from_secs(3600);
            #[cfg(feature = "scan_log")]
            let mut num_scans: u32 = 0;

            loop {
                #[cfg(feature = "scan_log")]
                {
                    // Kinda weird behavior but we get more scans/sec with an activated watchdog timer than without
                    // This might be some sort of artifact from the Duration comparisons
                    // Activated watchdog:   8k scans/sec
                    // Deactivated watchdog: 6k scans/sec
                    num_scans += 1;
                    if last_scan_log.elapsed() > Duration::from_secs(10) {
                        info!("[IR] Scans per second: {}", num_scans / 10);
                        num_scans = 0;
                        last_scan_log = Instant::now();
                    }
                }

                match goal_detector.scan() {
                    DetectedGoal::None => (),
                    goal => {
                        goal_detector.last_goal_now();
                        info!("[IR] Detected goal: {}", goal);
                        goal_tx.send(goal)?;
                    }
                }
            }
        })?;

    // BLE server thread setup and start
    ThreadSpawnConfiguration {
        name: SERVER_THREAD_NAME,
        priority: SERVER_THREAD_PRIORITY,
        pin_to_core: Some(SERVER_CORE),
        ..Default::default()
    }
    .set()?;

    let _ble_thread = thread::Builder::new()
        .stack_size(4096)
        .spawn(move || -> Result<()> {
            let mut goal_id: u32 = 0;
            loop {
                let goal = goal_rx.recv()?;
                goal_id = goal_id.saturating_add(1);
                info!("[BLE] Sending goal ({}): '{}'", goal_id, goal);
                let send_str = format!("{}: {}", goal_id, goal);
                // kicker_server.send(&goal.to_string());
                kicker_server.send(&send_str);
            }
        })?;

    ThreadSpawnConfiguration::default().set()?;

    if let Err(err) = scanner_thread.join() {
        error!("Error in scanner thread: {:?}", err);
    };

    unreachable!();
}
