mod utils;

use esp32_nimble::{enums::*, utilities::BleUuid, *};
use esp_idf_hal::{delay::FreeRtos, sys::TaskFunction_t};
use esp_idf_svc::sys::xTaskCreatePinnedToCore;
use std::{ffi::{c_void, CString}, ptr};
use esp_idf_sys::{self as _, ble_svc_gap_device_name};
use utils::{serial::*, thread};
use esp_idf_hal::task::block_on;

const SERVICE_UUID : BleUuid = BleUuid::from_uuid16(0xFF44);
const SERIAL_UUID: BleUuid = BleUuid::from_uuid128([
    0x53, 0x65, 0x72, 0x69, 0x61, 0x6C, 0x4E, 0x75,  // "SerialNu"
    0x6D, 0x62, 0x65, 0x72, 0x00, 0x00, 0x00, 0x00   // "mber" + padding to 16 bytes
]);

fn main() -> Result<(), anyhow::Error>{
    // -----------------------------
    // Required stuff, do not remove
    // -----------------------------
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::set_max_level(log::LevelFilter::Debug);
    // -----------------------------  

    unsafe { thread::thread_start("ble_advertise", advertise_thread) };
    unsafe { thread::thread_start("ble_scan", scan_thread) };

    // let winklink_device_info: DeviceInfo = utils::serial::DeviceInfo::new();

    // let _ = advertise_ble(&winklink_device_info);

    loop {
        // Keep the main thread alive otherwise it will panic
        FreeRtos::delay_ms(500);
    }
    Ok(())
}

unsafe extern "C" fn advertise_thread(arg: *mut c_void) {
    let winklink_device_info: DeviceInfo = utils::serial::DeviceInfo::new();
    loop {
        let _ = advertise_ble(&winklink_device_info);
        FreeRtos::delay_ms(100);
    }
}

unsafe extern "C" fn scan_thread(arg: *mut c_void) {
    let winklink_device_info: DeviceInfo = utils::serial::DeviceInfo::new();
    loop {
        let _ = scan_ble(&winklink_device_info);
        FreeRtos::delay_ms(100);
    }
}

async fn scan_ble(device_info: &DeviceInfo) -> anyhow::Result<()> {
    let mut winklink_found = false;

    let ble_device = BLEDevice::take();
    let mut ble_scan = BLEScan::new();
    ble_scan.active_scan(true).interval(100).window(99);

    ble_scan.start(
        ble_device,
        5000,
        |device, data| {
            log::info!("{:?},{:?}", &device, &data);
            if let Some(service_data) = &data.service_data() {
                if service_data.uuid == SERVICE_UUID {
                    println!("Located another winklink");
                    winklink_found = true;
                } else {
                    log::info!("No winklink device located yet :(");
                }
            }

            None::<()>
        }
    ).await?;
    Ok(())
}

fn advertise_ble(device_info: &DeviceInfo) -> anyhow::Result<()> {
    let mut counter = 0;

    let ble_device = BLEDevice::take();
    let ble_advertiser = ble_device.get_advertising();

    let mut ad_data = BLEAdvertisementData::new();
    ad_data.name(&device_info.device_name);
    ad_data.add_service_uuid(SERVICE_UUID);
    let serial_value = device_info.serial_num.parse::<u32>().unwrap_or(0);
    let bytes = serial_value.to_le_bytes();
    ad_data.service_data(SERIAL_UUID, &bytes);

    ble_advertiser.lock().set_data(&mut ad_data).unwrap();
    ble_advertiser.lock().advertisement_type(ConnMode::Non).disc_mode(DiscMode::Gen).scan_response(false);

    ble_advertiser.lock().start().unwrap();
    println!("Advertising started");
    loop {
        FreeRtos::delay_ms(10);
        counter+=1;
    }
    Ok(())
}