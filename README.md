# ESP-NOW StreetPass Protocol PROOF OF CONCEPT

A Rust implementation of a Nintendo StreetPass-like protocol for ESP32 devices using ESP-NOW for direct device-to-device communication.

## Overview

This project enables ESP32 devices to automatically discover and exchange information with other nearby ESP32 devices running the same firmware, similar to Nintendo's StreetPass feature used in Nintendo 3DS systems. When two devices come within range of each other, they'll exchange user profiles and can pass small amounts of game-specific data.

## Features

- **Automatic Discovery**: Devices automatically broadcast their presence and discover others without any user intervention
- **User Profiles**: Each device has a customizable user profile with username, avatar, and status message
- **Visual Feedback**: LED flash when a new device is encountered
- **Interaction Tracking**: Keeps record of encounters with other devices
- **Time-based Cleanup**: Old encounters are automatically removed after a configurable time period
- **Game Data Exchange**: Support for exchanging game/application-specific data
- **Efficient Protocol**: Lightweight protocol designed for embedded devices

## Requirements

- ESP32 development board
- Rust with ESP-IDF toolchain

## Building

```bash
cargo build
```

## Flashing

```bash
cargo flash --release
```

## Usage

1. Flash the firmware to your ESP32 device
2. The device will automatically start broadcasting and listening for other devices
3. When another device running the same firmware comes within range:
   - Both devices will exchange user information
   - The LED will flash to indicate a new encounter
   - The encounter will be logged

## Customization

You can modify the following constants in the code:

- `BROADCAST_INTERVAL_MS`: How often to broadcast presence (default: 2000ms)
- `INTERACTION_TTL`: How long to remember interactions (default: 1 day)
- `MAX_PEERS`: Maximum number of peers to track (default: 20)
- `LED_PIN`: GPIO pin for the indicator LED (default: GPIO2)

## Implementation Details

The implementation uses ESP-NOW, a connectionless communication protocol that enables direct device-to-device communication without the need for an access point. The protocol is designed to be lightweight and efficient for embedded devices.

