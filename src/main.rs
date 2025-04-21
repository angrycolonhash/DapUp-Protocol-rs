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

    // Create 2 threads, one to advertise and one to scan

    // Once another device has been located, it will stop the advertise and scan threads.
    // The MCU will setup a server and a client using another 2 threads
    // Both devices will connect to each other and transfer information. 

    // After a final confirmation of non-corrupted correct data (ACK_OK), 
    // it will add that device's MAC address to a blocklist (stopping connecting)
    // before disconnecting from each other and looking for other users. (starting cycle over again)

    // Blocklist can be edited through web server (for now, until displays can get working).
    // Blocklists contain the device MAC address and the information about the device + user info.
    // Blocklists only stop the MCU from connecting back to the device again, it is not a permanent thing
    

    Ok(())
}