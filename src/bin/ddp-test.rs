#[path = "../ddp.rs"]
mod ddp;
use anyhow::{Result, ensure};
use clap::Parser;
use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

#[derive(Parser)]
#[command(
    about = "Brief low-brightness DDP addressing test; configure LED count in the receiver first"
)]
struct Args {
    #[arg(long, default_value = "192.168.4.23:4048")]
    address: SocketAddr,
    #[arg(long, default_value_t = 100)]
    leds: usize,
}
fn main() -> Result<()> {
    let args = Args::parse();
    ensure!(
        (3..=10000).contains(&args.leds),
        "LED count must be 3..10000"
    );
    let mut sender = ddp::Sender::new(args.address)?;
    let mut pixels = vec![[0; 3]; args.leds];
    let result = (|| -> Result<()> {
        for stage in 0..4 {
            pixels.fill([0; 3]);
            if stage < 3 {
                let index = [0, (args.leds - 1) / 2, args.leds - 1][stage];
                pixels[index] = [16; 3];
                eprintln!("Dim white: LED {} (zero-based index {index})", index + 1);
            } else {
                pixels[..3].copy_from_slice(&[[16, 0, 0], [0, 16, 0], [0, 0, 16]]);
                eprintln!("LEDs 1,2,3: red, green, blue");
            }
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                sender.send(&pixels)?;
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        Ok(())
    })();
    pixels.fill([0; 3]);
    let cleanup = sender.send(&pixels);
    result?;
    cleanup?;
    eprintln!("Test complete; sent black. UDP transmission does not prove LEDs lit.");
    Ok(())
}
