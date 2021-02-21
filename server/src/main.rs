use std::process;
use std::str;
use std::{
    convert::TryInto,
    io::Write,
    net::{TcpStream, UdpSocket},
};
use std::{env, io::Read};

use std::net::TcpListener;

struct Config {
    port: u16,
}

struct FileData {
    filename: String,
    file_size: u64,
}

enum Message {
    Hello,
    Connection(u32),
    InfoFile(FileData),
    Ok,
    End,
    File,
    Ack,
}

impl Message {
    pub fn new(message_type: &[u8], bytes_read: usize) -> Result<Message, &'static str> {
        if bytes_read < 2 {
            return Err("Read fewer than 2 bytes");
        }
        let message_type_byte = message_type[1];

        match message_type_byte {
            1 => Ok(Self::Hello),
            2 => {
                if bytes_read < 6 {
                    return Err("Read fewer than 6 bytes for message that should contain 6 bytes");
                }
                let array = &message_type[2..6];
                let port = u32_from_u8_array(array);

                Ok(Self::Connection(port))
            }
            3 => {
                if bytes_read < 25 {
                    return Err(
                        "Read fewer than 25 bytes for message that should contain 25 bytes",
                    );
                }
                let filename = match str::from_utf8(&message_type[2..17]) {
                    Ok(str) => String::from(str.trim_matches(char::from(0))),
                    Err(_e) => return Err("Falha ao converter bytes para string"),
                };

                let file_size = u64_from_u8_array(&message_type[17..25]);

                Ok(Self::InfoFile(FileData {
                    filename,
                    file_size,
                }))
            }
            4 => Ok(Self::Ok),
            5 => Ok(Self::End),
            6 => Ok(Self::File),
            7 => Ok(Self::Ack),
            _ => Err("Unknown message type"),
        }
    }
}

fn u32_from_u8_array(u8_array: &[u8]) -> u32 {
    ((u8_array[0] as u32) << 24)
        + ((u8_array[1] as u32) << 16)
        + ((u8_array[2] as u32) << 8)
        + ((u8_array[3] as u32) << 0)
}

fn u64_from_u8_array(u8_array: &[u8]) -> u64 {
    ((u8_array[0] as u64) << 56)
        + ((u8_array[1] as u64) << 48)
        + ((u8_array[2] as u64) << 40)
        + ((u8_array[3] as u64) << 32)
        + ((u8_array[4] as u64) << 24)
        + ((u8_array[5] as u64) << 16)
        + ((u8_array[6] as u64) << 8)
        + ((u8_array[7] as u64) << 0)
}

impl Config {
    pub fn new(mut args: env::Args) -> Result<Config, &'static str> {
        args.next();

        let port = match args.next() {
            Some(port) => port,
            None => return Err("No port specified"),
        };

        let port = port
            .parse()
            .expect("Port should be a 16 bit unsigned integer");

        Ok(Config { port })
    }
}

fn main() {
    let config = Config::new(env::args()).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {}", err);
        process::exit(1);
    });

    let address = format!("127.0.0.1:{}", config.port);

    println!("{}", address);

    let listener = TcpListener::bind(address)
        .expect(format!("Failed to bind to port {}", config.port).as_str());

    for stream in listener.incoming() {
        let mut stream = stream.unwrap();

        handle_connection(stream);
    }
}

fn handle_connection(mut stream: TcpStream) -> Result<(), &'static str> {
    let mut buffer = [0; 1024];
    let bytes_read = stream.read(&mut buffer).unwrap();
    let message = Message::new(&buffer, bytes_read);
    if let Err(msg) = message {
        return Err(msg);
    }

    // TODO: this variable will be mutable
    let udp_port: u32 = 30000;
    let mut udp_socket = UdpSocket::bind(("127.0.0.1", udp_port as u16));

    let mut connection: Vec<u8> = vec![0, 2];
    for byte in udp_port.to_be_bytes().iter() {
        connection.push(*byte);
    }

    let buffer: [u8; 6] = connection.try_into().unwrap();

    let bytes_written = stream.write(&buffer);

    if let Err(_e) = bytes_written {
        return Err("Failed to send bytes");
    }

    let mut buffer = [0; 1024];
    let bytes_read = stream.read(&mut buffer).unwrap();
    let message = Message::new(&buffer, bytes_read);

    // TODO: remove debug code
    if let Ok(Message::InfoFile(FileData {
        filename,
        file_size,
    })) = message
    {
        println!("Filesize: {}, filename: {}", file_size, filename);
    }

    let ok = vec![0, 4];
    let bytes_written = stream.write(&ok);

    if let Err(_e) = bytes_written {
        return Err("Failed to send bytes");
    }

    Ok(())
}
