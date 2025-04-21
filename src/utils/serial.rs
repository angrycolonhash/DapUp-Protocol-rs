use std::error::Error;

use esp_idf_svc::nvs::{EspNvs, EspDefaultNvsPartition, NvsDefault};
use anyhow::Result;

pub enum NVSKeyword {
    SerialNumber,
    DeviceName,
}

pub fn print_info() {
    println!("Information about device: \nDevice Name: {}\nSerial Number: {}", 
        match read_nvs_data(NVSKeyword::DeviceName).unwrap() {
            Some(value)=>value,
            None => "Unknown".to_string(),
        },
        match read_nvs_data(NVSKeyword::SerialNumber).unwrap() {
            Some(value)=>value,
            None => "Unknown".to_string(),
        },
    );
}

pub(crate) fn store_serial_number(serial: &str) -> Result<()> {
    // Open or create NVS namespace
    let nvs_partition = EspDefaultNvsPartition::take()?;
    let mut nvs = EspNvs::<NvsDefault>::new(nvs_partition, "storage", true)?;
    
    // Store the serial number
    nvs.set_str("serial_num", serial)?;

    
    log::info!("Serial number saved: {}", serial);
    Ok(())
}

pub fn read_nvs_data(keyword:NVSKeyword) -> Result<Option<String>> {
    // Open NVS namespace
    let nvs_partition = EspDefaultNvsPartition::take()?;
    let nvs = EspNvs::<NvsDefault>::new(nvs_partition, "storage", true)?;
    
    match keyword {
        NVSKeyword::SerialNumber => {
            let mut buffer = [0u8; 64]; // Adjust the size as needed
            match nvs.get_str("serial_num", &mut buffer) {
                Ok(Some(serial)) => {
                    log::info!("Read serial number: {}", serial);
                    return Ok(Some(serial.to_string()))
                },
                Ok(None) | Err(_) => {
                    log::warn!("Keyword [{}] not found", "serial_num");
                    return Ok(None)
                }
            }
        }
        NVSKeyword::DeviceName => {
            let mut buffer = [0u8; 64]; // Adjust the size as needed
            match nvs.get_str("device_name", &mut buffer) {
                Ok(Some(device_name)) => {
                    log::info!("Read device name: {}", device_name);
                    return Ok(Some(device_name.to_string()))
                },
                Ok(None) | Err(_) => {
                    log::warn!("Keyword [{}] not found", "device_name");
                    return Ok(None)
                }
            }
        }
        _ => log::error!("Unable to fetch NVSKeyword from enum")
    }

    return Ok(None)
}

pub fn update_nvs_data(new_data: &str, keyword:NVSKeyword) -> Result<Option<()>> {
    // Open NVS namespace (same as when storing)
    let nvs_partition = EspDefaultNvsPartition::take()?;
    let mut nvs = EspNvs::<NvsDefault>::new(nvs_partition, "storage", true)?;
    
    match keyword {
        NVSKeyword::SerialNumber => {
            log::error!("Unable to rewrite over serial number, manual code change required");
        }
        NVSKeyword::DeviceName => {
            let mut buffer = [0u8;64];
            match nvs.get_str("device_name", &mut buffer) {
                Ok(Some(old_device_name)) => {
                    log::info!("Updating device name from '{}' to '{}'...", old_device_name, new_data);
                }
                _ => {
                    log::warn!("No existing device name found, creating new data!");
                }
            }

            nvs.set_str("device_name", new_data)?;

            log::info!("Device name has been updated successfully!");
            return Ok(Some(()))
        }
    }    
    Ok(None)
}

pub(crate) fn store_device_name(name: &str) -> Result<()> {
    // Open or create NVS namespace
    let nvs_partition = EspDefaultNvsPartition::take()?;
    let mut nvs = EspNvs::<NvsDefault>::new(nvs_partition, "storage", true)?;
    
    // Store the serial number
    nvs.set_str("device_name", name)?;
    
    log::info!("Device name saved: {}", name);
    Ok(())
}