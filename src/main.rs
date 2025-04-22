mod utils;

use esp32_nimble::{utilities::BleUuid, *};
use esp_idf_hal::{delay::FreeRtos, sys::TaskFunction_t};
use esp_idf_svc::sys::xTaskCreatePinnedToCore;
use std::{ffi::{c_void, CString}, ptr};
use esp_idf_sys::{self as _, ble_svc_gap_device_name};
use utils::{serial::*, thread};
use esp_idf_hal::task::block_on;

struct WinkLink {
    uuid_header: u16,
    found: bool,
}

impl WinkLink {
    fn new() -> Self {
        return WinkLink {
            found: false
        }
    }
}

const SERVICE_UUID : BleUuid = BleUuid::from_uuid16(0xFF44);

fn main() -> Result<(), anyhow::Error>{
    // -----------------------------
    // Required stuff, do not remove
    // -----------------------------
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::set_max_level(log::LevelFilter::Debug);
    // -----------------------------

    let mut winklink_device = WinkLink::new();

    let _ = scan_ble(&mut winklink_device);

    Ok(())
}

async fn scan_ble(winklink_device: &mut WinkLink) -> anyhow::Result<()> {
    let ble_device = BLEDevice::take();
    let mut ble_scan = BLEScan::new();
    ble_scan.active_scan(true).interval(100).window(99);

    ble_scan.start(
        ble_device,
        5000,
        |device, data| {
            log::info!("{:?},{:?}", &device, &data);
            if let Some(service_data) = &data.service_data() {
                if service_data.uuid == BleUuid::from_uuid16(SERVICE_UUID) {
                    println!("Located another winklink");
                    winklink_device.found = true;
                } else {
                    log::info!("No winklink device located yet :(");
                }
            }

            None::<()>
        }
    ).await?;

    log::info!("Scan finished!");
    Ok(())
}

fn advertise_ble(winklink_device: &mut WinkLink) -> anyhow::Result<()> {
    let ble_device = BLEDevice::take();
    let server = ble_device.get_advertising();

    let mut ad_data = BLEAdvertisementData::new();

    Ok(())
}