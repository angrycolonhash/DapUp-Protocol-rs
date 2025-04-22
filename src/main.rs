mod utils;

use esp32_nimble::{enums::*, utilities::BleUuid, *};
use esp_idf_hal::task::block_on;
use esp_idf_hal::{delay::FreeRtos, sys::TaskFunction_t};
use esp_idf_svc::sys::xTaskCreatePinnedToCore;
use esp_idf_sys::{self as _, ble_svc_gap_device_name};
use std::{
    ffi::{c_void, CString},
    ptr,
};
use utils::{serial::*, thread};

const SERVICE_UUID: BleUuid = BleUuid::from_uuid16(0xFF44);
const SERIAL_UUID: BleUuid = BleUuid::from_uuid128([
    0x53, 0x65, 0x72, 0x69, 0x61, 0x6C, 0x4E, 0x75, // "SerialNu"
    0x6D, 0x62, 0x65, 0x72, 0x00, 0x00, 0x00, 0x00, // "mber" + padding to 16 bytes
]);

fn main() -> Result<(), anyhow::Error> {
    // -----------------------------
    // Required stuff, do not remove
    // -----------------------------
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::set_max_level(log::LevelFilter::Debug);
    // -----------------------------

    ble_server();
    Ok(())
}

fn ble_server() -> anyhow::Result<()> {
    let device_info = DeviceInfo::new();
    let ble_device = BLEDevice::take();
    let ble_advertising = ble_device.get_advertising();

    let server = ble_device.get_server();
    server.on_connect(|server, desc| {
        log::info!("Client connected: {:?}", desc);

        server.update_conn_params(desc.conn_handle(), 24, 48, 0, 100).unwrap();

        if server.connected_count() < (esp_idf_svc::sys::CONFIG_BT_NIMBLE_MAX_CONNECTIONS as _) {
            log::info!("Multi-connect support: start advertising");
            ble_advertising.lock().start().unwrap();
        }
    });

    server.on_disconnect(|_desc, reason| {
        log::info!("Client disconnected from server: [{:?}]", reason);
    });

    let service = server.create_service(SERVICE_UUID);

    // let serial_number = service.lock().create_characteristic(SERIAL_UUID, NimbleProperties::READ);
    // serial_number.lock().set_value();

    let mut ad_data = BLEAdvertisementData::new();
    ad_data.name(&device_info.device_name);
    ad_data.add_service_uuid(SERVICE_UUID);
    let serial_value = device_info.serial_num.parse::<u32>().unwrap_or(0);
    let bytes = serial_value.to_le_bytes();
    ad_data.service_data(SERIAL_UUID, &bytes);

    ble_advertising.lock().set_data(&mut ad_data)?;
    ble_advertising.lock().start()?;

    server.ble_gatts_show_local();

    loop {
        FreeRtos::delay_ms(1000);
    }

    Ok(())
}