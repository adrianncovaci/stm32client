use std::net::TcpStream;

use clap::{Parser, Subcommand};

/// Interacts with a bootloader over the network
#[derive(Parser, Debug)]
#[command(name = "Bootloader Client", version = "0.1.0")]
struct Cli {
    /// IP address/hostname of bootloader
    hostname: String,

    /// Bootloader port, default 7777
    #[arg(long, default_value_t = 7777)]
    port: u16,

    /// Send an initial boot request to user firmware
    #[arg(long)]
    boot_req: bool,

    /// UDP port for boot request, default 1735
    #[arg(long, default_value_t = 1735)]
    boot_req_port: u16,

    /// Don't send a reboot request after completion
    #[arg(long)]
    no_reboot: bool,

    /// Size of chunks to write to flash, default 512
    #[arg(long, default_value_t = 512)]
    chunk_size: usize,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Just read bootloader information without rebooting
    Info,

    /// Bootload new firmware image
    Program {
        /// Address to load to, default 0x08010000
        #[arg(long, default_value = "0x08010000")]
        lma: String,

        /// Raw binary file to program
        binfile: String,
    },

    /// Load new configuration
    Configure {
        /// Address to write to, default 0x0800C000
        #[arg(long, default_value = "0x0800C000")]
        lma: String,

        /// MAC address, in format XX:XX:XX:XX:XX:XX
        mac_address: String,

        /// IP address, in format XXX.XXX.XXX.XXX
        ip_address: String,

        /// Gateway address, in format XXX.XXX.XXX.XXX
        gateway_address: String,

        /// Subnet prefix length
        prefix_length: u8,
    },

    /// Send immediate reboot request
    Boot,
}

struct Client {
    socket: TcpStream,
}

fn main() {}

