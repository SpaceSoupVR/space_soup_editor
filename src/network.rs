use std::sync::{Arc, Mutex};
use std::net::TcpListener;

use space_soup_engine::{DebugPacket, debug_receiver::PacketReader};

pub type SharedPacket = Arc<Mutex<DebugPacket>>;

pub fn spawn_listener(addr: &str) -> SharedPacket {
    let shared: SharedPacket = Arc::new(Mutex::new(DebugPacket::default()));
    let shared_writer = shared.clone();

    let addr = addr.to_string();
    std::thread::spawn(move || {
        let listener = match TcpListener::bind(&addr) {
            Ok(l) => l,
            Err(e) => {
                log::error!("debug_viewer: could not bind {addr}: {e}");
                return;
            }
        };
        log::info!("debug_viewer: listening on {addr}");

        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            log::info!("debug_viewer: quest_app connected");

            let mut reader = PacketReader::new(stream);
            loop {
                match reader.read_packet() {
                    Some(packet) => {
                        *shared_writer.lock().unwrap() = packet;
                    }
                    None => {
                        log::info!("debug_viewer: quest_app disconnected");
                        break;
                    }
                }
            }
        }
    });

    shared
}
