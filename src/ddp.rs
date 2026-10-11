//! DDP v1 RGB24 output. Offsets count bytes, not LEDs; UDP has no acknowledgement.
use anyhow::{Result, ensure};
use std::net::{SocketAddr, UdpSocket};

const PIXELS_PER_PACKET: usize = 480;
pub fn packets(rgb: &[[u8; 3]], sequence: u8) -> Result<Vec<Vec<u8>>> {
    ensure!(!rgb.is_empty(), "at least one LED is required");
    ensure!(rgb.len() <= (u32::MAX as usize) / 3, "DDP offset overflow");
    let mut result = Vec::new();
    let count = rgb.len().div_ceil(PIXELS_PER_PACKET);
    for (index, chunk) in rgb.chunks(PIXELS_PER_PACKET).enumerate() {
        let mut packet = vec![0x40 | u8::from(index + 1 == count), sequence & 15, 0x0b, 1];
        packet.extend_from_slice(&((index * PIXELS_PER_PACKET * 3) as u32).to_be_bytes());
        packet.extend_from_slice(&((chunk.len() * 3) as u16).to_be_bytes());
        for pixel in chunk {
            packet.extend_from_slice(pixel);
        }
        result.push(packet);
    }
    Ok(result)
}
pub struct Sender {
    socket: UdpSocket,
    sequence: u8,
}
impl Sender {
    pub fn new(destination: SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind(if destination.is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        })?;
        socket.connect(destination)?;
        Ok(Self {
            socket,
            sequence: 0,
        })
    }
    pub fn send(&mut self, rgb: &[[u8; 3]]) -> Result<()> {
        self.sequence = self.sequence % 15 + 1;
        for packet in packets(rgb, self.sequence)? {
            ensure!(
                self.socket.send(&packet)? == packet.len(),
                "incomplete UDP send"
            );
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn header_rgb_and_lengths() {
        assert_eq!(
            packets(&[[1, 2, 3], [4, 5, 6]], 7).unwrap()[0],
            vec![0x41, 7, 0x0b, 1, 0, 0, 0, 0, 0, 6, 1, 2, 3, 4, 5, 6]
        );
        assert_eq!(packets(&[[0; 3]; 300], 1).unwrap()[0].len(), 910);
        assert!(packets(&[], 1).is_err());
    }
    #[test]
    fn packet_offsets_and_final_push() {
        let p = packets(&[[12, 34, 56]; 481], 15).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0][0], 0x40);
        assert_eq!(&p[1][..10], &[0x41, 15, 0x0b, 1, 0, 0, 5, 160, 0, 3]);
        assert_eq!(&p[1][10..], &[12, 34, 56]);
    }
}
