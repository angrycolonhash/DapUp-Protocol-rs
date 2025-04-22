mod utils;

use esp32_nimble::{uuid128, BLEAdvertisementData, BLEDevice, BLEScan, NimbleProperties};
use esp_idf_hal::{
    delay::FreeRtos,
    prelude::Peripherals,
    task::block_on,
    timer::{TimerConfig, TimerDriver},
};
use esp_idf_sys::{self as _};
use std::{
    thread,
    time::SystemTime,
    sync::{Arc, Mutex},
    collections::HashMap,
};
use utils::serial::*;

// Define our UUIDs
const SERVICE_UUID: &str = "FF440000-0000-0000-0000-000000000000";
const SERIAL_UUID: &str = "53657269-616C-4E75-6D62-657200000000"; // "SerialNumber" in hex
const DEVICE_NAME_UUID: &str = "64657669-6365-6E61-6D65-000000000000"; // "devicename" in hex
const DEVICE_OWNER_UUID: &str = "6465766F-776E-6572-0000-000000000000"; // "devowner" in hex
const TIMESTAMP_UUID: &str = "74696D65-7374-616D-7000-000000000000"; // "timestamp" in hex

// Increased stack sizes for ESP32-WROOM-32
const ADVERTISE_STACK_SIZE: usize = 8192; // Doubled from typical 4096
const SCAN_STACK_SIZE: usize = 8192;      // Doubled from typical 4096

// Struct to hold blocklist information - simplified to reduce memory usage
#[derive(Clone, Debug)]
struct BlockedDevice {
    serial_num: String,
    device_name: String,
    device_owner: String,
    timestamp: u64,
}

// Global blocklist
struct AppState {
    blocklist: HashMap<String, BlockedDevice>,
    is_scanning: bool,
    is_advertising: bool,
}

fn main() -> anyhow::Result<()> {
    // -----------------------------
    // Required stuff, do not remove
    // -----------------------------
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::set_max_level(log::LevelFilter::Info); // Changing to Info level to reduce memory usage
    // -----------------------------
    
    log::info!("Starting Dap-Up Protocol on ESP32-WROOM-32");

    DeviceInfo::store_device_owner("tk");
    
    // Create shared app state with smaller initial capacity for memory conservation
    let app_state = Arc::new(Mutex::new(AppState {
        blocklist: HashMap::with_capacity(5), // Reduced capacity to save memory
        is_scanning: false,
        is_advertising: false,
    }));
    
    // Clone the app state for each thread
    let state_adv = Arc::clone(&app_state);
    let state_scan = Arc::clone(&app_state);

    // Start advertising thread with increased stack size
    let adv_handle = thread::Builder::new()
        .name("ble_advertise".to_string())
        .stack_size(ADVERTISE_STACK_SIZE)
        .spawn(move || {
            match ble_advertise(state_adv) {
                Ok(_) => log::info!("BLE advertise thread terminated normally"),
                Err(e) => log::error!("BLE advertise thread error: {:?}", e),
            }
        })
        .unwrap();
    
    // Short delay to let advertising thread initialize
    thread::sleep(std::time::Duration::from_millis(500));
    
    // Start scanning thread with increased stack size
    let scan_handle = thread::Builder::new()
        .name("ble_scan".to_string())
        .stack_size(SCAN_STACK_SIZE)
        .spawn(move || {
            match block_on(ble_scan(state_scan)) {
                Ok(_) => log::info!("BLE scan thread terminated normally"),
                Err(e) => log::error!("BLE scan thread error: {:?}", e),
            }
        })
        .unwrap();
    
    // Keep main thread alive
    adv_handle.join().unwrap();
    scan_handle.join().unwrap();
    
    Ok(())
}

fn ble_advertise(app_state: Arc<Mutex<AppState>>) -> anyhow::Result<()> {
    log::info!("Starting BLE advertising thread");
    let device_info = DeviceInfo::new();
    let ble_device = BLEDevice::take();
    let ble_advertising = ble_device.get_advertising();

    // Mark that we're now advertising
    {
        let mut state = app_state.lock().unwrap();
        state.is_advertising = true;
    }

    let server = ble_device.get_server();
    server.on_connect(|server, desc| {
        log::info!("Client connected: {:?}", desc);
        server
            .update_conn_params(desc.conn_handle(), 24, 48, 0, 60)
            .unwrap();
    });

    server.on_disconnect(|_desc, reason| {
        log::info!("Client disconnected ({:?})", reason);
    });

    // Create service using uuid128! macro
    let service = server.create_service(uuid128!(SERVICE_UUID));
    
    // Create characteristics for all device info
    let serial_char = service.lock().create_characteristic(
        uuid128!(SERIAL_UUID),
        NimbleProperties::READ,
    );
    serial_char.lock().set_value(device_info.serial_num.as_bytes());
    
    let name_char = service.lock().create_characteristic(
        uuid128!(DEVICE_NAME_UUID),
        NimbleProperties::READ,
    );
    name_char.lock().set_value(device_info.device_name.as_bytes());
    
    let owner_char = service.lock().create_characteristic(
        uuid128!(DEVICE_OWNER_UUID),
        NimbleProperties::READ,
    );
    owner_char.lock().set_value(device_info.device_owner.as_bytes());
    
    // Timestamp characteristic (will be updated on connection)
    let timestamp_char = service.lock().create_characteristic(
        uuid128!(TIMESTAMP_UUID),
        NimbleProperties::READ | NimbleProperties::WRITE | NimbleProperties::NOTIFY,
    );
    
    // Handler for timestamp updates
    timestamp_char.lock().on_write(|args| {
        if let Ok(timestamp_str) = core::str::from_utf8(args.recv_data()) {
            log::info!("Received timestamp data: {}", timestamp_str);
        } else {
            log::info!("Received timestamp data as bytes: {:?}", args.recv_data());
        }
    });
    
    // Set up advertising data
    ble_advertising.lock().set_data(
        BLEAdvertisementData::new()
            .name(&device_info.device_name)
            .add_service_uuid(uuid128!(SERVICE_UUID)),
    )?;
    
    // Main advertising loop
    ble_advertising.lock().start()?;
    log::info!("Started advertising as '{}'", device_info.device_name);
    
    // Server monitoring loop
    loop {
        // Check if we should stop advertising
        {
            let state = app_state.lock().unwrap();
            if !state.is_advertising {
                log::info!("Stopping advertising as requested");
                break;
            }
        }
        
        // Update the timestamp when a client is connected
        if server.connected_count() > 0 {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            timestamp_char.lock()
                .set_value(now.to_string().as_bytes())
                .notify();
        }
        
        FreeRtos::delay_ms(1000);
    }
    
    ble_advertising.lock().stop()?;
    log::info!("Advertising stopped");
    Ok(())
}

// For ESP32-WROOM-32, make sure we define static configs for BLE operations
const BLE_SCAN_INTERVAL: u16 = 200;  // Longer scan interval (in 0.625ms units)
const BLE_SCAN_WINDOW: u16 = 60;     // Smaller scan window (in 0.625ms units)
const BLE_SCAN_DURATION: u32 = 4000; // Shorter scan duration (in ms)
const BLE_CONN_TIMEOUT: u32 = 4000;  // Connection timeout (in ms)

async fn ble_scan(app_state: Arc<Mutex<AppState>>) -> anyhow::Result<()> {
    log::info!("Starting BLE scan thread");
    
    // Mark that we're now scanning
    {
        let mut state = app_state.lock().unwrap();
        state.is_scanning = true;
    }
    
    let peripherals = Peripherals::take()?;
    let mut timer = TimerDriver::new(peripherals.timer00, &TimerConfig::new())?;
    
    let ble_device = BLEDevice::take();
    
    // Main scanning loop
    loop {
        // Check if we should stop scanning
        {
            let state = app_state.lock().unwrap();
            if !state.is_scanning {
                log::info!("Stopping scanning as requested");
                break;
            }
        }
        
        // Create a new scan instance each time to avoid memory leaks
        let mut ble_scan = BLEScan::new();
        
        // Configure scan with memory-optimized parameters
        ble_scan
            .active_scan(true)
            .interval(BLE_SCAN_INTERVAL)
            .window(BLE_SCAN_WINDOW);
        
        log::info!("Scanning for BLE devices...");
        
        // Start a short scan to conserve memory
        let device = ble_scan.start(ble_device, BLE_SCAN_DURATION.try_into().unwrap(), |device, data| {
            // First check if this device is in our blocklist
            let addr_str = device.addr().to_string();
            let is_blocked = {
                let state = app_state.lock().unwrap();
                state.blocklist.contains_key(&addr_str)
            };
            
            if is_blocked {
                log::info!("Skipping blocked device: {}", addr_str);
                return None;
            }
            
            // Then look for devices advertising our service UUID
            for uuid in data.service_uuids() {
                if uuid.to_string() == SERVICE_UUID {
                    log::info!("Found device with our service: {}", addr_str);
                    return Some(*device);
                }
            }
            None
        })
        .await?;
        
        if let Some(device) = device {
            // Stop scanning and advertising when device found
            {
                let mut state = app_state.lock().unwrap();
                state.is_advertising = false;
                state.is_scanning = false;
            }
            
            // Explicitly drop the scanner to free up resources
            std::mem::drop(ble_scan);
            
            // Process the connection with memory cleanup
            process_connection(ble_device, device, app_state.clone()).await?;
            
            // Restart advertising and scanning
            {
                let mut state = app_state.lock().unwrap();
                state.is_advertising = true;
                state.is_scanning = true;
            }
            
            // Give the system time to clean up resources
            FreeRtos::delay_ms(500);
        } else {
            // Explicitly drop the scanner to free up resources
            std::mem::drop(ble_scan);
            
            // Longer delay between scans to reduce memory pressure
            FreeRtos::delay_ms(2000);
        }
        
        // Manual cleanup to help with memory management
        unsafe {
            // Request a garbage collection cycle if available
            esp_idf_sys::heap_caps_check_integrity_all(true);
        }
    }
    
    log::info!("Scanning stopped");
    Ok(())
}

// Process a connection to another device
async fn process_connection(ble_device: &mut BLEDevice, device: esp32_nimble::BLEAdvertisedDevice, app_state: Arc<Mutex<AppState>>) -> anyhow::Result<()> {
    log::info!("Processing connection to device: {}", device.addr());
    
    // Create a client and connect to the found device
    let mut client = ble_device.new_client();
    let addr = device.addr();
    
    client.on_connect(|client| {
        log::info!("Connected to server");
        client.update_conn_params(120, 120, 0, 60).unwrap();
    });
    
    // Connect to the device with timeout
    log::info!("Connecting to {}", addr);
    match client.connect(&addr).await {
        Ok(_) => log::info!("Successfully connected to {}", addr),
        Err(e) => {
            log::error!("Failed to connect to {}: {:?}", addr, e);
            return Ok(());
        }
    }
    
    // Use scoped blocks to limit variable lifetimes and free memory earlier
    let device_data = {
        // Get our service
        let service = match client.get_service(uuid128!(SERVICE_UUID)).await {
            Ok(svc) => svc,
            Err(e) => {
                log::error!("Failed to get service: {:?}", e);
                client.disconnect()?;
                return Ok(());
            }
        };
        
        // Read all characteristics
        let mut device_data = BlockedDevice {
            serial_num: String::new(),
            device_name: String::new(),
            device_owner: String::new(),
            timestamp: 0,
        };
        
        // Read Serial Number
        if let Ok(char) = service.get_characteristic(uuid128!(SERIAL_UUID)).await {
            if let Ok(value) = char.read_value().await {
                if let Ok(str_val) = core::str::from_utf8(&value) {
                    device_data.serial_num = str_val.to_string();
                    log::info!("Read serial number: {}", str_val);
                }
            }
        }
        
        // Read Device Name
        if let Ok(char) = service.get_characteristic(uuid128!(DEVICE_NAME_UUID)).await {
            if let Ok(value) = char.read_value().await {
                if let Ok(str_val) = core::str::from_utf8(&value) {
                    device_data.device_name = str_val.to_string();
                    log::info!("Read device name: {}", str_val);
                }
            }
        }
        
        // Read Device Owner
        if let Ok(char) = service.get_characteristic(uuid128!(DEVICE_OWNER_UUID)).await {
            if let Ok(value) = char.read_value().await {
                if let Ok(str_val) = core::str::from_utf8(&value) {
                    device_data.device_owner = str_val.to_string();
                    log::info!("Read device owner: {}", str_val);
                }
            }
        }
        
        // Read Timestamp and send ACK
        let mut success = false;
        if let Ok(char) = service.get_characteristic(uuid128!(TIMESTAMP_UUID)).await {
            if let Ok(value) = char.read_value().await {
                if let Ok(str_val) = core::str::from_utf8(&value) {
                    if let Ok(ts) = str_val.parse::<u64>() {
                        device_data.timestamp = ts;
                        log::info!("Read timestamp: {}", ts);
                    }
                }
            }
            
            // Send ACK_OK by writing to the timestamp characteristic
            let ack_message = "ACK_OK".as_bytes();
            match char.write_value(ack_message, false).await {
                Ok(_) => {
                    log::info!("Sent ACK_OK confirmation");
                    success = true;
                },
                Err(e) => log::error!("Failed to send ACK_OK: {:?}", e)
            }
        }
        
        // Only return device data if successful
        if success {
            Some(device_data)
        } else {
            None
        }
    };
    
    // Add to blocklist if we successfully got data
    if let Some(data) = device_data {
        // Add to blocklist
        {
            let mut state = app_state.lock().unwrap();
            let addr_str = addr.to_string();
            state.blocklist.insert(addr_str, data.clone());
            log::info!("Added device to blocklist: {:?}", data);
        }
    }
    
    // Disconnect from device and clean up
    log::info!("Disconnecting from {}", addr);
    if let Err(e) = client.disconnect() {
        log::error!("Error disconnecting: {:?}", e);
    }
    
    // Force a small delay to allow resources to be freed
    FreeRtos::delay_ms(100);
    
    Ok(())
}