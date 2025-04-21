mod utils;

use esp32_nimble::*;
use esp_idf_hal::{delay::FreeRtos, sys::TaskFunction_t};
use esp_idf_svc::sys::xTaskCreatePinnedToCore;
use std::{ffi::{c_void, CString}, ptr};
use esp_idf_sys as _;
use utils::serial::*;
use esp_idf_hal::task::block_on;

fn main() -> Result<(), anyhow::Error>{
    // -----------------------------
    // Required stuff, do not remove
    // -----------------------------
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    // -----------------------------

    let name = match read_nvs_data(NVSKeyword::DeviceName).unwrap() {
        Some(value)=>value,
        None => "Unknown".to_string(),
    };

    block_on( async {
        let ble_device = BLEDevice::take();
        let mut ble_scan = BLEScan::new();
        let device = match ble_scan
            .start(ble_device, 10000, |device, data| {
                if let Some(device_name) = data.name() {
                    if device_name == name {
                        return Some(*device);
                    }
                }
                None
            })
            .await
            .unwrap() {
                Some(value) => value,
                None => todo!("To do later coz i dont know what to put here :("),
            };
        
        println!(
            //"Advertised Device Name: {:?}, "
            "Address: {:?} dB, RSSI: {:?}",
            device.addr(),
            device.rssi()
        );

    });

    Ok(())
}