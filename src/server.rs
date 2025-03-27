use esp32_nimble::{
    utilities::{mutex::Mutex, BleUuid},
    BLEAdvertisementData, BLECharacteristic, BLEDevice, BLEServer, BLEService, NimbleProperties,
};
use log::{debug, info};
use std::sync::Arc;

// Stuff that's missing from original implementation:
//   * pAdvertising->setMinPreferred(0x06) interval is not set

pub(crate) trait Server<T> {
    fn new(config: T) -> Self;
    fn send(&self, goals: &str);
}

pub(crate) struct BleConfig {
    pub(crate) service_uuid: BleUuid,
    pub(crate) goals_uuid: BleUuid,
    pub(crate) ir_threshold_uuid: BleUuid,
}

pub(crate) struct KickerBle<'a> {
    // device: &'a mut BLEDevice,
    _server: &'a mut BLEServer,
    _config: BleConfig,
    goals_characteristic: Arc<Mutex<BLECharacteristic>>,
    ir_threshold_characteristic: Arc<Mutex<BLECharacteristic>>,
    _service: Arc<Mutex<BLEService>>,
}

impl Server<BleConfig> for KickerBle<'_> {
    fn new(config: BleConfig) -> Self {
        let ble_device = BLEDevice::take();
        let ble_advertising = ble_device.get_advertising();
        let _server = ble_device.get_server();
        let service = _server.create_service(config.service_uuid);
        // let mut client_connected = false;

        // create server
        _server
            .on_connect(|server, desc| {
                info!("Client connected: '{:?}'", desc);

                server
                    .update_conn_params(desc.conn_handle(), 24, 48, 0, 60)
                    .unwrap();

                if server.connected_count()
                    < (esp_idf_svc::sys::CONFIG_BT_NIMBLE_MAX_CONNECTIONS as _)
                {
                    ::log::info!("Multi-connect support: start advertising");
                    ble_advertising.lock().start().unwrap();
                }
            })
            .on_disconnect(|_desc, reason| {
                ::log::info!("Client disconnected ({:?})", reason);
            });

        // create characteristics
        let goals_characteristic = service.lock().create_characteristic(
            config.goals_uuid,
            NimbleProperties::READ | NimbleProperties::NOTIFY,
        );
        let ir_thres_characteristic = service
            .lock()
            .create_characteristic(config.ir_threshold_uuid, NimbleProperties::WRITE);

        goals_characteristic.lock().set_value(b"Number of Goals");
        // let goals_descriptor = goals_characteristic
        //     .lock()
        //     .create_descriptor(BleUuid::from_uuid16(0x2902), DescriptorProperties::READ);
        // goals_descriptor.lock().set_value(b"Number of Goals");
        debug!("Characteristic 'Number of goals' is set.");

        // mode characteristic. Production = 0, Debug = 1
        // let mode_characteristic = service.lock().create_characteristic(
        //     config.mode_characteristic_uuid,
        //     NimbleProperties::WRITE | NimbleProperties::READ,
        // );
        // mode_characteristic.lock().on_write(|args| {
        //     if let Ok(val) = std::str::from_utf8(args.recv_data()) {
        //         info!("Got new value for mode_characteristic: '{val}'");
        //         if val == "1" || val.to_lowercase() == "true" || val.to_lowercase() == "debug" {
        //             // set server into debug mode
        //             // DEBUG_MODE.lock().borrow().set(true);
        //             // Lazy::force(&DEBUG_MODE).lock().set(true);
        //             // DEBUG_MODE.lock().set(true);
        //         } else {
        //             // Lazy::force(&DEBUG_MODE).lock().set(false);
        //             // DEBUG_MODE.lock().set(true);
        //         }
        //         // debug!("New value for DEBUG_MODE: {:?}", DEBUG_MODE.lock().get());
        //     };
        // });
        // let mode_descriptor = mode_characteristic
        //     .lock()
        //     .create_descriptor(BleUuid::from_uuid16(0x2902), DescriptorProperties::READ);
        // mode_descriptor.lock().set_value(b"Operational mode");

        let _ = BLEDevice::set_device_name("Goal Counter");
        ble_advertising
            .lock()
            .set_data(
                BLEAdvertisementData::new()
                    .name("Goal server")
                    .add_service_uuid(config.service_uuid),
            )
            .unwrap();
        ble_advertising.lock().start().unwrap();

        _server.ble_gatts_show_local();

        Self {
            _server,
            _config: config,
            goals_characteristic,
            _service: service,
        }
    }

    fn send(&self, goals: &str) {
        self.goals_characteristic
            .lock()
            .set_value(goals.as_bytes())
            .notify();
    }
}
