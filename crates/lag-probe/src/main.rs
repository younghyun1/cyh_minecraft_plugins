//! Bounded Minecraft status timing probe; no login or server mutation.
use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpStream},
    time::{Duration, Instant},
};

fn varint(mut value: u32, output: &mut Vec<u8>) {
    loop {
        let byte = (value & 127) as u8;
        value >>= 7;
        output.push(if value == 0 { byte } else { byte | 128 });
        if value == 0 {
            break;
        }
    }
}

fn frame(body: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    varint(body.len() as u32, &mut output);
    output.extend_from_slice(body);
    output
}

fn read_frame(stream: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut length = 0u32;
    for shift in (0..35).step_by(7) {
        let mut byte = [0];
        stream.read_exact(&mut byte)?;
        if shift == 28 && byte[0] > 0x0f {
            return Err(io::Error::other("invalid frame length"));
        }
        length |= u32::from(byte[0] & 127) << shift;
        if byte[0] & 128 == 0 {
            if length > 1_048_576 {
                return Err(io::Error::other("oversized frame"));
            }
            let mut body = vec![0; length as usize];
            stream.read_exact(&mut body)?;
            return Ok(body);
        }
    }
    Err(io::Error::other("invalid frame length"))
}

fn probe(address: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
    let host = address.ip().to_string();
    let start = Instant::now();
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let connected = start.elapsed();
    let mut handshake = vec![0];
    varint(u32::MAX, &mut handshake);
    varint(host.len() as u32, &mut handshake);
    handshake.extend_from_slice(host.as_bytes());
    handshake.extend_from_slice(&address.port().to_be_bytes());
    handshake.push(1);
    let mut request = frame(&handshake);
    request.extend_from_slice(&[1, 0]);
    stream.write_all(&request)?;
    let status = read_frame(&mut stream)?;
    let received = start.elapsed();
    if status.first() != Some(&0) {
        return Err("unexpected status packet".into());
    }
    let mut ping = vec![1];
    ping.extend_from_slice(&42i64.to_be_bytes());
    let ping_start = Instant::now();
    stream.write_all(&frame(&ping))?;
    if read_frame(&mut stream)? != ping {
        return Err("unexpected pong".into());
    }
    println!(
        "target={address} connect_ms={:.3} status_ms={:.3} pong_ms={:.3} status_bytes={}",
        connected.as_secs_f64() * 1000.,
        (received - connected).as_secs_f64() * 1000.,
        ping_start.elapsed().as_secs_f64() * 1000.,
        status.len()
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.as_slice() == ["--help"] {
        println!(
            "Usage: mc-lag-probe <IP:PORT> [IP:PORT ...]\nRuns five status and ping probes per target; at most 16 targets. IPv6 uses [ADDRESS]:PORT."
        );
        return Ok(());
    }
    if arguments.is_empty() || arguments.len() > 16 {
        return Err("provide 1 to 16 IP:PORT targets; use --help for usage".into());
    }
    let targets: Result<Vec<SocketAddr>, _> = arguments.iter().map(|value| value.parse()).collect();
    let targets = targets?;
    for _ in 0..5 {
        for address in &targets {
            probe(*address)?;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{frame, read_frame};

    #[test]
    fn accepts_bounded_frames_and_rejects_invalid_lengths() -> std::io::Result<()> {
        for length in [0, 1, 127, 128, 4096, 1_048_576] {
            let body = vec![42; length];
            assert_eq!(read_frame(&mut frame(&body).as_slice())?, body);
        }
        for bytes in [
            vec![0xff, 0xff, 0xff, 0xff, 0x7f],
            vec![0x80, 0x80, 0x80, 0x80, 0x10],
            vec![0x80, 0x80, 0x80, 0x80, 0x80],
            vec![0x81, 0x80, 0x40],
            vec![2, 1],
        ] {
            assert!(read_frame(&mut bytes.as_slice()).is_err());
        }
        Ok(())
    }
}
