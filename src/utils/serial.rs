use anyhow::Result;
use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault};

pub enum NVSKeyword {
    SerialNumber,
    DeviceName,
    DeviceOwner,
}

pub struct DeviceInfo {
    pub serial_num: String,
    pub device_name: String,
    pub device_owner: String,
}

impl DeviceInfo {
    pub fn new() -> Self {
        let nvs_partition = EspDefaultNvsPartition::take().unwrap();
        let nvs = EspNvs::<NvsDefault>::new(nvs_partition, "storage", true).unwrap();

        let mut serial_num = String::new();
        let mut device_name = String::new();
        let mut device_owner = String::new();

        {
            let mut buffer = [0u8; 64];
            match nvs.get_str("serial_num", &mut buffer) {
                Ok(Some(serial)) => {
                    serial_num = serial.to_string();
                }
                Ok(None) | Err(_) => {
                    log::warn!("Keyword [{}] not found", "serial_num");
                    serial_num = "Unknown".to_string();
                }
            }
        }

        {
            let mut buffer = [0u8; 64];
            match nvs.get_str("device_owner", &mut buffer) {
                Ok(Some(value)) => {
                    device_owner = value.to_string();
                }
                Ok(None) | Err(_) => {
                    log::warn!("Keyword [{}] not found", "device_owner");
                    device_owner = "Unknown".to_string();
                }
            }
        }

        {
            let mut buffer = [0u8; 64];
            match nvs.get_str("device_name", &mut buffer) {
                Ok(Some(value)) => {
                    device_name = value.to_string();
                }
                Ok(None) | Err(_) => {
                    log::warn!("Keyword [{}] not found", "device_name");
                    device_name = "Unknown".to_string();
                }
            }
        }

        return DeviceInfo {
            serial_num,
            device_name,
            device_owner,
        };
    }

    pub fn update(new_data: &str, keyword: NVSKeyword) -> anyhow::Result<Option<()>> {
        // Open NVS namespace (same as when storing)
        let nvs_partition = EspDefaultNvsPartition::take()?;
        let mut nvs = EspNvs::<NvsDefault>::new(nvs_partition, "storage", true)?;

        match keyword {
            NVSKeyword::SerialNumber => {
                log::error!("Unable to rewrite over serial number, manual code change required");
            }
            NVSKeyword::DeviceName => {
                let mut buffer = [0u8; 64];
                match nvs.get_str("device_name", &mut buffer) {
                    Ok(Some(old_device_name)) => {
                        log::info!(
                            "Updating device name from '{}' to '{}'...",
                            old_device_name,
                            new_data
                        );
                    }
                    _ => {
                        log::warn!("No existing device name found, creating new data!");
                    }
                }

                nvs.set_str("device_name", new_data)?;

                log::info!("Device name has been updated successfully!");
                return Ok(Some(()));
            }
            NVSKeyword::DeviceOwner => {
                let mut buffer = [0u8; 64];
                match nvs.get_str("device_owner", &mut buffer) {
                    Ok(Some(old_device_owner)) => {
                        log::info!(
                            "Updating device name from '{}' to '{}'...",
                            old_device_owner,
                            new_data
                        );
                    }
                    _ => {
                        log::error!("Device has not been registered to a user, please manually create the user or register to someone");
                        return Ok(None);
                    }
                }

                nvs.set_str("device_owner", new_data)?;

                log::info!("Device name has been updated successfully!");
                return Ok(Some(()));
            }
        }
        Ok(None)
    }

    pub fn print(self) {
        println!(
            "Information about device: \nSerial Number: {}\nDevice Name: {}\nDevice Owner: {}\n",
            self.serial_num, self.device_name, self.device_owner
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

    pub(crate) fn store_device_name(name: &str) -> Result<()> {
        // Open or create NVS namespace
        let nvs_partition = EspDefaultNvsPartition::take()?;
        let mut nvs = EspNvs::<NvsDefault>::new(nvs_partition, "storage", true)?;

        // Store the device name
        nvs.set_str("device_name", name)?;

        log::info!("Device name saved: {}", name);
        Ok(())
    }

    pub(crate) fn store_device_owner(name: &str) -> Result<()> {
        // Open or create NVS namespace
        let nvs_partition = EspDefaultNvsPartition::take()?;
        let mut nvs = EspNvs::<NvsDefault>::new(nvs_partition, "storage", true)?;

        // Store the device owner
        nvs.set_str("device_owner", name)?;

        log::info!("Device owner saved: {}", name);
        Ok(())
    }
}
