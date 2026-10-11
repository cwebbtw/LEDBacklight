#[path = "../ddp.rs"]
mod ddp;
use anyhow::Result;
use clap::Parser;
use std::{
    io::{self, BufRead},
    net::SocketAddr,
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    time::Duration,
};

const LEDS: usize = 140;
#[derive(Parser)]
#[command(about = "Position calibration for your installed 140-LED monitor strip")]
struct Args {
    #[arg(long, default_value = "192.168.4.23:4048")]
    address: SocketAddr,
    /// Step a single dim white LED through all 140 positions with Enter.
    #[arg(long)]
    chase: bool,
}
fn sides() -> Vec<[u8; 3]> {
    let mut pixels = vec![[0; 3]; LEDS];
    pixels[..23].fill([0, 0, 32]);
    pixels[113..].fill([0, 0, 32]);
    pixels[24..43].fill([32, 32, 0]);
    pixels[44..92].fill([32, 0, 0]);
    pixels[93..113].fill([0, 32, 0]);
    pixels[23] = [16, 16, 16];
    pixels[43] = [16, 16, 16];
    pixels[92] = [16, 16, 16];
    pixels
}
fn hold(
    sender: &mut ddp::Sender,
    pixels: &[[u8; 3]],
    commands: &Receiver<io::Result<bool>>,
) -> Result<bool> {
    eprintln!("Take your time. Press ENTER for the next stage, or type Q then ENTER to finish.");
    loop {
        sender.send(pixels)?;
        // Keep refreshing DDP while stdin waits, so WLED does not time out.
        match commands.recv_timeout(Duration::from_millis(50)) {
            Ok(command) => return Ok(command?),
            Err(RecvTimeoutError::Timeout) => (),
            Err(RecvTimeoutError::Disconnected) => return Ok(false),
        }
    }
}
fn main() -> Result<()> {
    let args = Args::parse();
    let mut sender = ddp::Sender::new(args.address)?;
    eprintln!(
        "Close the game-lighting program first. WLED must be on and configured for 140 LEDs."
    );
    eprintln!("Left/right below are viewed from the FRONT. LED 44 is assumed top-left.");
    let (tx, commands) = mpsc::channel();
    std::thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            let command = line.map(|line| !line.trim().eq_ignore_ascii_case("q"));
            let quit = !matches!(command, Ok(true));
            if tx.send(command).is_err() || quit {
                break;
            }
        }
    });
    let result = (|| -> Result<()> {
        if args.chase {
            for led in 1..=LEDS {
                let mut pixels = vec![[0; 3]; LEDS];
                pixels[led - 1] = [32; 3];
                eprintln!("LED {led}");
                if !hold(&mut sender, &pixels, &commands)? {
                    return Ok(());
                }
            }
        } else {
            eprintln!("SIDE TEST: TOP=red, RIGHT=green, BOTTOM=blue, LEFT=yellow; corners=white.");
            if !hold(&mut sender, &sides(), &commands)? {
                return Ok(());
            }
            for (led, label) in [
                (1, "start: bottom split"),
                (24, "bottom-left corner"),
                (44, "top-left corner"),
                (93, "top-right corner"),
                (114, "first bottom LED near right"),
                (140, "last LED beside LED 1"),
            ] {
                let mut pixels = vec![[0; 3]; LEDS];
                pixels[led - 1] = [32; 3];
                eprintln!("MARKER: LED {led} — {label}");
                if !hold(&mut sender, &pixels, &commands)? {
                    return Ok(());
                }
            }
        }
        Ok(())
    })();
    let cleanup = sender.send(&vec![[0; 3]; LEDS]);
    result?;
    cleanup?;
    eprintln!("Finished; sent black. Report any side/marker that appears in the wrong position.");
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manual_stage_refreshes_until_advance_and_quit() {
        let receiver = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        receiver
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let address = receiver.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();
        let task = std::thread::spawn(move || {
            let mut sender = ddp::Sender::new(address).unwrap();
            hold(&mut sender, &[[32, 0, 0]], &rx).unwrap()
        });
        let mut packet = [0u8; 64];
        for _ in 0..2 {
            assert_eq!(receiver.recv(&mut packet).unwrap(), 13);
            assert_eq!(&packet[10..13], &[32, 0, 0]);
        }
        tx.send(Ok(true)).unwrap();
        assert!(task.join().unwrap());
        let (tx, rx) = mpsc::channel();
        tx.send(Ok(false)).unwrap();
        let mut sender = ddp::Sender::new(address).unwrap();
        assert!(!hold(&mut sender, &[[0; 3]], &rx).unwrap());
    }
    #[test]
    fn side_ranges_and_three_corners() {
        let p = sides();
        assert_eq!(p.len(), 140);
        for (colour, count) in [
            ([0, 0, 32], 50),
            ([32, 32, 0], 19),
            ([32, 0, 0], 48),
            ([0, 32, 0], 20),
            ([16, 16, 16], 3),
        ] {
            assert_eq!(p.iter().filter(|&&c| c == colour).count(), count);
        }
        for led in [24, 44, 93] {
            assert_eq!(p[led - 1], [16, 16, 16]);
        }
        assert_eq!(p[0], p[139]);
    }
}
