use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::thread;

pub struct MinecraftScanner {
    port: Arc<Mutex<Vec<u16>>>,
    _holder: Sender<()>,
}

fn write_varint(buf: &mut Vec<u8>, mut value: u32) {
    loop {
        if value & 0xFFFFFF80 == 0 {
            buf.push(value as u8);
            return;
        }
        buf.push((value as u8) | 0x80);
        value >>= 7;
    }
}

fn probe_mc_server(port: u16) -> bool {
    let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
    let Ok(mut stream) = TcpStream::connect_timeout(&addr.into(), Duration::from_millis(500)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));

    let host = b"127.0.0.1";
    let mut payload = Vec::new();
    payload.push(0x00);
    write_varint(&mut payload, host.len() as u32);
    payload.extend_from_slice(host);
    payload.extend_from_slice(&port.to_be_bytes());
    payload.push(0x01);

    let mut buf = Vec::new();
    write_varint(&mut buf, (payload.len() + 1) as u32);
    buf.push(0x00);
    buf.extend_from_slice(&payload);

    if stream.write_all(&buf).is_err() {
        return false;
    }
    if stream.write_all(&[0x01, 0x00]).is_err() {
        return false;
    }

    let mut raw = [0u8; 2048];
    let n = match stream.read(&mut raw) {
        Ok(n) if n > 0 => n,
        _ => return false,
    };
    let raw = &raw[..n];

    let mut off = 0;
    loop {
        if off >= raw.len() {
            return false;
        }
        if raw[off] & 0x80 == 0 {
            off += 1;
            break;
        }
        off += 1;
    }
    if off >= raw.len() || raw[off] != 0x00 {
        return false;
    }
    off += 1;
    let mut json_len = 0i32;
    let mut shift = 0;
    loop {
        if off >= raw.len() {
            return false;
        }
        let byte = raw[off];
        off += 1;
        json_len |= ((byte & 0x7F) as i32) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    let json_len = json_len as usize;
    if off + json_len > raw.len() {
        return false;
    }

    let json_str = &raw[off..off + json_len];
    std::str::from_utf8(json_str).map_or(false, |s| s.contains("version"))
}

impl MinecraftScanner {
    pub fn create() -> MinecraftScanner {
        let (tx, rx) = mpsc::channel::<()>();
        let port = Arc::new(Mutex::new(vec![]));

        let port_cloned = Arc::clone(&port);
        thread::spawn(move || {
            Self::run(rx, port_cloned);
        });

        MinecraftScanner { _holder: tx, port }
    }

    fn run(signal: Receiver<()>, output: Arc<Mutex<Vec<u16>>>) {
        loop {
            if let Err(mpsc::TryRecvError::Disconnected) = signal.try_recv() {
                return;
            }

            let found = if probe_mc_server(25565) { vec![25565] } else { vec![] };

            {
                let mut output = output.lock().unwrap();
                if *output != found {
                    *output = found;
                    logging!("Server Scanner", "Updated server list: {:?}", *output);
                }
            }

            thread::sleep(Duration::from_secs(3));
        }
    }

    pub fn get_ports(&self) -> Vec<u16> {
        self.port.lock().unwrap().clone()
    }
}
