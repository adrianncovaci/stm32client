use std::{
    io::{Read, Write},
    net::TcpStream,
    path::PathBuf,
    time::Duration,
};

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum Command<'a> {
    Info,
    Read,
    Erase { address: u32, length: u32 },
    Write { sector: &'a [u8], data: &'a [u8] },
    Boot,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum BootloadError {
    Success,
    InvalidAddress,
    LengthNotMultiple32,
    LengthTooLong,
    DataLengthIncorrect,
    EraseError,
    WriteError,
    FlashError,
    NetworkError,
    InternalError,
    PartialWriteSuccess(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadResponse<'a> {
    pub bootloader_version: &'a [u8],
    pub git_version: &'a [u8],
    pub built_time: &'a [u8],
    //pub mcu_id: String,
    pub flash_bank1_len: usize,
    pub flash_bank2_len: usize,
}

/// Interacts with a bootloader over the network
#[derive(Parser, Debug)]
#[command(name = "Bootloader Client", version = "0.1.0")]
struct Cli {
    /// IP address/hostname of bootloader
    hostname: String,

    /// Bootloader port, default 7777
    #[arg(long, default_value_t = 6971)]
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

    /// Timeout for socket operations, default 5s
    #[arg(long, default_value_t = 200)]
    timeout: u64,

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
        #[arg(long, default_value_t = 0x08010000)]
        lma: u64,

        /// Raw binary file to program
        binfile: PathBuf,
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
    Erase,
}

struct Client {
    socket: TcpStream,
    chunk_size: u32,
}

impl Client {
    fn new(
        hostname: &str,
        port: u16,
        timeout: u64,
        chunk_size: u32,
    ) -> Result<Self, std::io::Error> {
        let socket = TcpStream::connect((hostname, port))?;
        socket.set_read_timeout(Some(Duration::from_secs(timeout)))?;
        socket.set_write_timeout(Some(Duration::from_secs(timeout)))?;
        //Just block for now as we only do one op at a time
        socket.set_nonblocking(false)?;

        Ok(Self { socket, chunk_size })
    }

    fn send_program_request(&mut self, lma: u64, binfile: PathBuf) -> Result<(), std::io::Error> {
        let mut binfile = std::fs::read(binfile).expect("Failed to read binfile");

        let len = binfile.len();
        let padding = if len % 32 == 0 { 0 } else { 32 - (len % 32) };
        binfile.resize(len + padding, 0xFF);
        let mut segments = binfile.chunks(self.chunk_size as usize);

        println!("Erasing flash sector");
        self.erase_flash(0x08010000, len as u32)?;

        let segments_len = segments.len();

        for (i, segment) in segments.into_iter().enumerate() {
            println!(
                "Writing segment(size={}) {} of {}",
                segment.len(),
                i,
                segments_len
            );
            self.write_flash(i as u32, segment).unwrap();
        }

        Ok(())
    }

    fn erase_flash(&mut self, address: u32, length: u32) -> Result<(), std::io::Error> {
        let cmd = Command::Erase { address, length };
        let cmd = postcard::to_stdvec(&cmd).expect("Failed to serialize erase command");
        self.socket.write_all(&cmd)?;

        println!("{:?} erasing the flash", self.get_reply());

        Ok(())
    }

    fn get_reply(&mut self) -> Result<BootloadError, std::io::Error> {
        let mut buf = vec![0; 256];
        self.socket.read(&mut buf)?;
        postcard::from_bytes(&buf).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to deserialize error")
        })
    }

    fn write_flash(&mut self, sector: u32, data: &[u8]) -> Result<(), std::io::Error> {
        let sector = &sector.to_le_bytes();
        let cmd = Command::Write { sector, data };
        let cmd = postcard::to_stdvec(&cmd).expect("Failed to serialize write command");
        println!("Writing {} bytes to the socket", cmd.len());
        self.socket.write_all(&cmd).unwrap();

        println!("Writing status: {:?}", self.get_reply());

        Ok(())
    }

    fn send_info_request(&mut self) -> Result<(), std::io::Error> {
        let cmd = Command::Info;
        let cmd = postcard::to_stdvec(&cmd).expect("Failed to serialize info command");
        self.socket.write_all(&cmd)?;

        //println!("Info reply: {:?}", self.get_reply()?);

        //let mut buf = vec![0; 1024];
        //self.socket.read_to_end(&mut buf)?;
        //let respionse = postcard::from_bytes::<ReadResponse>(&buf)
        //    .map(|el| el.clone())
        //    .map_err(|_| {
        //        std::io::Error::new(
        //            std::io::ErrorKind::Other,
        //            "Failed to deserialize info response",
        //        )
        //    })?;

        //println!("Info response: {:?}", respionse);

        Ok(())
    }
}

fn main() {
    let args = Cli::parse();
    let mut client = Client::new(
        &args.hostname,
        args.port,
        args.timeout,
        args.chunk_size as u32,
    )
    .expect("Failed to connect");

    match args.command {
        Commands::Info => {
            println!("Info {:?}", client.send_info_request())
        }
        Commands::Program { lma, binfile } => {
            client
                .send_program_request(lma, binfile)
                .expect("Failed to send program request");
        }
        Commands::Configure {
            lma,
            mac_address,
            ip_address,
            gateway_address,
            prefix_length,
        } => {
            //let cmd = Command::Write;
            //let cmd = postcard::to_stdvec(&cmd).expect("Failed to serialize write command");
            //client
            //    .socket
            //    .write_all(&cmd)
            //    .expect("Failed to send write command");
        }
        Commands::Boot => {
            let cmd = Command::Boot;
            let cmd = postcard::to_stdvec(&cmd).expect("Failed to serialize boot command");
            client
                .socket
                .write_all(&cmd)
                .expect("Failed to send boot command");
        }
        Commands::Erase => {
            client
                .erase_flash(0x08010000, 0x1000)
                .expect("Failed to send erase command");
        }
    }
}
