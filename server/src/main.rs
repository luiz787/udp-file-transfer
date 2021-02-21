use std::net::TcpListener;
use std::process;
use std::str;
use std::{
    convert::TryInto,
    io::Write,
    net::{TcpStream, UdpSocket},
};
use std::{env, io::Read};

use common::{FileData, Message};

struct Config {
    port: u16,
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
        let stream = stream.unwrap();

        if let Err(msg) = handle_connection(stream) {
            eprintln!("{}", msg);
        }
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
    let _udp_socket = UdpSocket::bind(("127.0.0.1", udp_port as u16));

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
