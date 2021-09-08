extern crate btleplug;
extern crate rand;
extern crate uuid;

use std::str::FromStr;
use std::thread;
use std::time::Duration;
use async_std::task;
use futures::stream::{Stream, StreamExt};
use btleplug::api::{Central, Manager as _, Peripheral as _};
use btleplug::platform::{Manager};
use uuid::Uuid;

fn u8asu16be(src: [u8; 2]) -> u16 {
    return ((src[1] as u16) << 8) | src[0] as u16;
}

fn crc16(src: &[u8]) -> u16 {
    let mut crc: u16 = 0xffff;
    for b in src {
        crc ^= (*b as u16) << 8;
        for _ in 0..8 {
            if crc & 0x8000 != 0 {
                crc = (crc << 1) ^ 0x1021;
            } else {
                crc = crc << 1;
            }
        }
    }
    return crc;
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    let manager = Manager::new().await.unwrap();

    // get the first bluetooth adapter
    let adapters = manager.adapters().await.unwrap();
    let adapter = adapters.into_iter().nth(0).unwrap();

    // start scanning for devices
    adapter.start_scan().await.unwrap();
    // instead of waiting, you can use central.on_event to be notified of new devices
    thread::sleep(Duration::from_secs(2));

    // find the device we're interested in
    let peripherals = adapter.peripherals().await.unwrap();
    let peripheral = peripherals.into_iter()
        .find(|p| task::block_on(p.properties()).unwrap().unwrap().local_name.iter()
            .any(|name| name.contains("E66"))).unwrap();

    // connect to the device
    println!("Connecting...");
    peripheral.connect().await.unwrap();

    if peripheral.is_connected().await.unwrap() {
        println!(
            "Discover peripheral : \'{:?}\' characteristics...",
            peripheral.properties().await.unwrap().unwrap().local_name
        );
    }

    // println!("Getting characteristics");
    peripheral.discover_characteristics().await.unwrap();
    println!("service_data: {}", peripheral.properties().await.unwrap().unwrap().service_data.len());
    println!("services: {}", peripheral.properties().await.unwrap().unwrap().services.len());
    for service in peripheral.properties().await.unwrap().unwrap().services {
        println!("services: {}", service);
    }

    // find the characteristic we want
    println!("Parsing characteristics");
    let chars = peripheral.characteristics();
    for c in chars.iter() {
        println!("characteristics: {}", c.uuid);
    }

    // Same for E66+
    let sub_uuid = Uuid::from_str("be940001-7333-be46-b7ae-689e71722bd5").unwrap();
    let cmd_uuid = Uuid::from_str("be940001-7333-be46-b7ae-689e71722bd5").unwrap();
    let sub_char = chars.iter().find(|c| c.uuid == sub_uuid).unwrap();
    let cmd_char = chars.iter().find(|c| c.uuid == cmd_uuid).unwrap();

    println!("Registering notification handler");
    let stream = peripheral.notifications().await.unwrap();
    let stream_closure = stream.for_each_concurrent(100, |ev| async move {
        println!("Message received: {:X?}", ev.value);
        println!("- U8: {:X?}", u8asu16be([ev.value[ev.value.len()-2], ev.value[ev.value.len()-1]]));
        println!("- CRC: {:X?}", crc16(&ev.value[..ev.value.len()-2]));
    });
    println!("Subscribing to {}", sub_char.uuid);
    peripheral.subscribe(&sub_char).await.unwrap();

    let message: Vec<u8> = vec![0x01, 0x06, 0x07, 0x00, 0x00, 0x54, 0x19];
    println!("Sending message {:X?}", message);
    peripheral.write(&cmd_char, &message, btleplug::api::WriteType::WithResponse).await.unwrap();
    thread::sleep(Duration::from_millis(200));
    stream_closure.await;
    Ok(())
}
