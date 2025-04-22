use anyhow::Result;
use esp_idf_hal::{
    gpio::PinDriver,
    peripherals::Peripherals,
};
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    wifi::{AuthMethod, ClientConfiguration, Configuration, EspWifi},
    nvs::EspDefaultNvsPartition,
};
use esp_idf_sys::{
    self as _, 
    esp_now_add_peer, 
    esp_now_deinit, 
    esp_now_init, 
    esp_now_register_recv_cb, 
    esp_now_register_send_cb, 
    esp_now_send, 
    esp_now_unregister_recv_cb, 
    esp_now_unregister_send_cb, 
    esp_random, 
    wifi_interface_t_WIFI_IF_STA, 
    esp_now_peer_info_t, 
    ESP_OK,
    esp_now_recv_cb_t,
    esp_now_send_cb_t,
    esp_wifi_get_mac,
    esp_now_recv_info_t,
};
use std::{
    sync::{Arc, Mutex},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
    collections::HashMap,
    ptr,
};
use serde::{Serialize, Deserialize};
use heapless::Vec as HVec;

// Define StreetPass-like feature constants
const BROADCAST_INTERVAL_MS: u32 = 2000;  // How often to broadcast presence
const INTERACTION_TTL: u64 = 86400;       // How long to remember interactions (1 day)
const MAX_PEERS: usize = 20;              // Maximum number of peers to track
const LED_PIN: i32 = 2;                   // ESP32 built-in LED pin
const MAX_DATA_LEN: usize = 200;          // Max ESP-NOW data length

// MAC address length
const MAC_ADDR_LEN: usize = 6;

// Broadcast address (all FFs)
const BROADCAST_MAC: [u8; MAC_ADDR_LEN] = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];

// User-configurable device info
#[derive(Serialize, Deserialize, Debug, Clone)]
struct DeviceInfo {
    username: String,       // Username for display
    avatar_id: u8,          // Avatar/icon identifier 
    status_message: String, // Short status message
    game_id: u16,           // Current "game" or activity identifier
}

impl DeviceInfo {
    fn new() -> Self {
        unsafe {
            DeviceInfo {
                username: "StreetPassUser".to_string(),
                avatar_id: (esp_random() % 10) as u8,
                status_message: "Hello from ESP32!".to_string(),
                game_id: 0,
            }
        }
    }
}

// StreetPass packet definition
#[derive(Serialize, Deserialize, Debug, Clone)]
enum StreetPassPacket {
    // Basic presence broadcast with device info
    Beacon {
        device_info: DeviceInfo,
        timestamp: u64,
    },
    
    // Acknowledgment of receiving a beacon
    Ack {
        timestamp: u64,
    },
    
    // Game-specific data exchange
    GameData {
        game_id: u16,
        data: HVec<u8, 128>,  // Limited size payload
    },
}

// Struct to hold encounter information
#[derive(Debug, Clone)]
struct Encounter {
    device_info: DeviceInfo,
    first_seen: u64,
    last_seen: u64,
    interaction_count: u32,
}

// Application state
struct AppState {
    device_info: DeviceInfo,
    encounters: HashMap<[u8; MAC_ADDR_LEN], Encounter>,
    my_mac: [u8; MAC_ADDR_LEN],
    led_on: bool,
}

// Global application state
static mut APP_STATE: Option<Arc<Mutex<AppState>>> = None;

fn main() -> Result<()> {
    // Required initializations
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    log::set_max_level(log::LevelFilter::Info);
    
    log::info!("Starting StreetPass-like ESP-NOW Protocol");
    
    // Initialize hardware - we'll only initialize peripherals once
    // and pass the modem to WiFi setup
    let peripherals = Peripherals::take()?;
    
    // Setup WiFi in station mode (required for ESP-NOW)
    setup_wifi(peripherals.modem)?;
    
    // Get our own MAC address
    let mut mac = [0u8; MAC_ADDR_LEN];
    unsafe {
        esp_wifi_get_mac(wifi_interface_t_WIFI_IF_STA, mac.as_mut_ptr());
    }
    
    log::info!("Device MAC address: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}", 
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
    
    // Create application state
    let app_state = Arc::new(Mutex::new(AppState {
        device_info: DeviceInfo::new(),
        encounters: HashMap::with_capacity(MAX_PEERS),
        my_mac: mac,
        led_on: false,
    }));
    
    // Store app state in global variable for callbacks
    unsafe {
        APP_STATE = Some(app_state.clone());
    }
    
    // Initialize ESP-NOW
    unsafe {
        let result = esp_now_init();
        if result != ESP_OK {
            log::error!("Failed to initialize ESP-NOW: {}", result);
            return Err(anyhow::anyhow!("ESP-NOW init failed"));
        }
        
        // Register receive callback
        let recv_cb: esp_now_recv_cb_t = Some(esp_now_receive_callback);
        esp_now_register_recv_cb(recv_cb);
        
        // Register send callback
        let send_cb: esp_now_send_cb_t = Some(esp_now_send_callback);
        esp_now_register_send_cb(send_cb);
        
        // Add broadcast peer
        let mut peer_info: esp_now_peer_info_t = std::mem::zeroed();
        peer_info.peer_addr.copy_from_slice(&BROADCAST_MAC);
        peer_info.channel = 0; // Auto channel
        peer_info.ifidx = wifi_interface_t_WIFI_IF_STA;
        
        let result = esp_now_add_peer(&peer_info);
        if result != ESP_OK {
            log::error!("Failed to add broadcast peer: {}", result);
        }
    }
    
    // Create threads for broadcasting and LED control
    let led_state = app_state.clone();
    let broadcaster_state = app_state.clone();
    
    // LED control thread
    let led_handle = thread::Builder::new()
        .name("led_control".to_string())
        .stack_size(4096)
        .spawn(move || {
            let mut led = PinDriver::output(peripherals.pins.gpio2).unwrap();
            loop {
                // Flash LED when encounters occur
                let should_light = {
                    let state = led_state.lock().unwrap();
                    state.led_on
                };
                
                if should_light {
                    led.set_high().unwrap_or_else(|e| log::error!("Failed to turn on LED: {:?}", e));
                    thread::sleep(Duration::from_millis(100));
                    led.set_low().unwrap_or_else(|e| log::error!("Failed to turn off LED: {:?}", e));
                    
                    // Reset LED state after flashing
                    {
                        let mut state = led_state.lock().unwrap();
                        state.led_on = false;
                    }
                    
                    // Keep LED off for a bit
                    thread::sleep(Duration::from_millis(300));
                } else {
                    thread::sleep(Duration::from_millis(50));
                }
            }
        })?;
    
    // Broadcaster thread
    let broadcaster_handle = thread::Builder::new()
        .name("broadcaster".to_string())
        .stack_size(4096)
        .spawn(move || {
            loop {
                // Create beacon packet
                let packet = {
                    let state = broadcaster_state.lock().unwrap();
                    StreetPassPacket::Beacon {
                        device_info: state.device_info.clone(),
                        timestamp: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or(Duration::from_secs(0))
                            .as_secs(),
                    }
                };
                
                // Serialize packet
                let serialized = match serde_json::to_vec(&packet) {
                    Ok(data) => data,
                    Err(e) => {
                        log::error!("Failed to serialize packet: {:?}", e);
                        thread::sleep(Duration::from_millis(BROADCAST_INTERVAL_MS as u64));
                        continue;
                    }
                };
                
                // Send packet via ESP-NOW
                if serialized.len() <= MAX_DATA_LEN {
                    unsafe {
                        let result = esp_now_send(
                            BROADCAST_MAC.as_ptr(),
                            serialized.as_ptr(),
                            serialized.len()
                        );
                        
                        if result != ESP_OK {
                            log::warn!("Failed to send ESP-NOW packet: {}", result);
                        }
                    }
                } else {
                    log::warn!("Packet too large for ESP-NOW: {} bytes", serialized.len());
                }
                
                // Wait before next broadcast
                thread::sleep(Duration::from_millis(BROADCAST_INTERVAL_MS as u64));
                
                // Clean up old encounters
                cleanup_old_encounters(&broadcaster_state);
            }
        })?;
    
    // Keep main thread alive
    led_handle.join().unwrap();
    broadcaster_handle.join().unwrap();
    
    // Cleanup ESP-NOW (never reached, but for completeness)
    unsafe {
        esp_now_unregister_recv_cb();
        esp_now_unregister_send_cb();
        esp_now_deinit();
    }
    
    Ok(())
}

// Set up WiFi in station mode for ESP-NOW
fn setup_wifi(modem: esp_idf_hal::modem::Modem) -> Result<()> {
    log::info!("Configuring WiFi for ESP-NOW...");
    
    // We'll use esp-idf-svc's WiFi instead of raw FFI calls for proper initialization
    let sysloop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;
    
    let mut wifi = EspWifi::new(
        modem,
        sysloop,
        Some(nvs),
    )?;
    
    log::info!("Setting WiFi configuration...");
    
    // Configure WiFi in station mode with empty SSID (we're just using ESP-NOW)
    let ssid = heapless::String::<32>::new();
    let password = heapless::String::<64>::new();
    
    // Configure WiFi in station mode
    let wifi_config = Configuration::Client(
        ClientConfiguration {
            ssid,
            password,
            auth_method: AuthMethod::None,
            ..Default::default()
        }
    );
    
    wifi.set_configuration(&wifi_config)?;
    
    // We need to start WiFi, but we don't need to connect to any AP
    log::info!("Starting WiFi...");
    wifi.start()?;
    
    // Wait for WiFi to initialize
    std::thread::sleep(Duration::from_millis(100));
    
    log::info!("WiFi configured in station mode");
    
    // Keep WiFi alive to prevent deinitialization
    std::mem::forget(wifi);
    
    Ok(())
}

// ESP-NOW receive callback
unsafe extern "C" fn esp_now_receive_callback(
    info: *const esp_now_recv_info_t, 
    data: *const u8, 
    data_len: i32
) {
    if info.is_null() || data.is_null() || data_len <= 0 {
        return;
    }

    if let Some(app_state) = &APP_STATE {
        // Get source MAC address from the info structure
        let info_ref = &*info;
        let src_mac_ptr = (*info_ref).src_addr;
        
        if src_mac_ptr.is_null() {
            return;
        }
        
        // Copy the MAC address to an array
        let mut src_mac = [0u8; MAC_ADDR_LEN];
        ptr::copy_nonoverlapping(src_mac_ptr, src_mac.as_mut_ptr(), MAC_ADDR_LEN);
        
        // Skip messages from ourselves
        let my_mac = {
            let state = app_state.lock().unwrap();
            state.my_mac
        };
        
        if src_mac == my_mac {
            return;
        }
        
        // Copy data to a Vec
        let mut data_vec = Vec::with_capacity(data_len as usize);
        for i in 0..data_len {
            data_vec.push(*data.add(i as usize));
        }
        
        // Deserialize packet
        if let Ok(packet) = serde_json::from_slice::<StreetPassPacket>(&data_vec) {
            process_packet(app_state.clone(), &src_mac, packet);
        } else {
            log::warn!("Received malformed packet from {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}", 
                src_mac[0], src_mac[1], src_mac[2], src_mac[3], src_mac[4], src_mac[5]);
        }
    }
}

// ESP-NOW send callback
unsafe extern "C" fn esp_now_send_callback(
    mac_addr: *const u8,
    status: u32
) {
    if mac_addr.is_null() {
        return;
    }
    
    let mut dst_mac = [0u8; MAC_ADDR_LEN];
    ptr::copy_nonoverlapping(mac_addr, dst_mac.as_mut_ptr(), MAC_ADDR_LEN);
    
    if status != 0 {
        log::warn!("Failed to send ESP-NOW message to {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}: {}",
            dst_mac[0], dst_mac[1], dst_mac[2], dst_mac[3], dst_mac[4], dst_mac[5], status);
    }
}

// Process received packets
fn process_packet(app_state: Arc<Mutex<AppState>>, src_mac: &[u8; MAC_ADDR_LEN], packet: StreetPassPacket) {
    match packet {
        StreetPassPacket::Beacon { device_info, timestamp } => {
            log::info!("Received beacon from {}: {} ({})", 
                format_mac(src_mac), 
                device_info.username, 
                device_info.status_message);
            
            // Record encounter
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::from_secs(0))
                .as_secs();
            
            {
                let mut state = app_state.lock().unwrap();
                
                // Check if we've seen this device before
                let is_new = !state.encounters.contains_key(src_mac);
                
                // Update encounter record
                let encounter = state.encounters
                    .entry(*src_mac)
                    .or_insert(Encounter {
                        device_info: device_info.clone(),
                        first_seen: now,
                        last_seen: now,
                        interaction_count: 0,
                    });
                
                encounter.last_seen = now;
                encounter.interaction_count += 1;
                encounter.device_info = device_info.clone();
                
                // Flash LED for new encounters
                if is_new {
                    state.led_on = true;
                    log::info!("New StreetPass encounter with {}!", device_info.username);
                }
            }
            
            // Send acknowledgment
            let ack_packet = StreetPassPacket::Ack { timestamp };
            if let Ok(serialized) = serde_json::to_vec(&ack_packet) {
                unsafe {
                    esp_now_send(
                        src_mac.as_ptr(),
                        serialized.as_ptr(),
                        serialized.len()
                    );
                }
            }
        },
        
        StreetPassPacket::Ack { timestamp } => {
            log::debug!("Received acknowledgment for beacon sent at {}", timestamp);
            // Could update stats here if needed
        },
        
        StreetPassPacket::GameData { game_id, data: _ } => {
            log::info!("Received game data for game ID: {}", game_id);
            // Process game-specific data as needed
            // This would be where game-specific logic would go
        }
    }
}

// Format MAC address as string
fn format_mac(mac: &[u8; MAC_ADDR_LEN]) -> String {
    format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}", 
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5])
}

// Clean up old encounters based on TTL
fn cleanup_old_encounters(app_state: &Arc<Mutex<AppState>>) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs();
    
    let mut state = app_state.lock().unwrap();
    state.encounters.retain(|_, encounter| {
        now - encounter.last_seen < INTERACTION_TTL
    });
}