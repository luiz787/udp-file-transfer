use std::ffi::OsStr;
use std::io::prelude::*;
use std::net::IpAddr;
use std::net::TcpStream;
use std::path::Path;
use std::process;
use std::str;
use std::{env, io::Write};

use common::Message;

struct Config {
    ip: IpAddr,
    port: u16,
    filename: Filename,
}

struct Filename {
    filename: String,
}

impl Filename {
    pub fn new(filename: Option<String>) -> Result<Filename, &'static str> {
        let filename = match filename {
            Some(filename) => filename,
            None => return Err("Nome do arquivo não especificado"),
        };

        if filename.len() > 15 {
            return Err("Nome não permitido");
        }
        if !filename.contains(".") {
            return Err("Nome não permitido");
        }
        if filename.matches(".").count() > 1 {
            return Err("Nome não permitido");
        }

        let extension = Path::new(&filename)
            .extension()
            .and_then(OsStr::to_str)
            .unwrap();
        if extension.len() > 3 {
            return Err("Nome não permitido");
        }

        if !filename.chars().all(|ch| ch.is_ascii()) {
            return Err("Nome não permitido");
        }

        return Ok(Filename { filename });
    }
}

impl Config {
    pub fn new(mut args: env::Args) -> Result<Config, &'static str> {
        args.next();

        let ip = match args.next() {
            Some(ip) => ip,
            None => return Err("Server ip not specified"),
        };

        let port = match args.next() {
            Some(port) => port,
            None => return Err("Server port not specified"),
        };

        // TODO: refactor to return Err instead of panic
        let port = port
            .parse()
            .expect("Port should be a 16 bit unsigned integer");

        let ip = ip.parse::<IpAddr>();

        let ip = match ip {
            Ok(ip) => ip,
            Err(_error) => return Err("Failed to parse ip address"),
        };

        let filename = Filename::new(args.next());
        let filename = match filename {
            Ok(filename) => filename,
            Err(msg) => return Err(msg),
        };

        Ok(Config { ip, port, filename })
    }
}

fn main() {
    let config = Config::new(env::args()).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {}", err);
        process::exit(1);
    });

    let mut stream =
        TcpStream::connect((config.ip, config.port)).expect("Failed to connect to remote server.");

    let mut hello = [0 as u8; 2];
    hello[1] = 1 as u8;

    let bytes_sent = stream.write(&hello).expect("Failed to write buffer");
    println!("{} bytes sent", bytes_sent);

    let mut buffer = [0; 1024];
    let bytes_read = stream.read(&mut buffer).unwrap();

    let message = Message::new(&buffer, bytes_read);

    // TODO: handle error
    match message {
        Ok(Message::Connection(port)) => println!("Port is: {}", port),
        _ => {}
    }

    let mut info_file: Vec<u8> = vec![0, 3];
    let filename = config.filename.filename.as_bytes();

    for _padding_byte in 0..15 - filename.len() {
        info_file.push(0);
    }

    for byte in filename.iter() {
        info_file.push(*byte);
    }

    let file_contents = std::fs::read(config.filename.filename).expect("Falha ao abrir o arquivo");

    let file_length = file_contents.len();

    for byte in file_length.to_be_bytes().iter() {
        info_file.push(*byte);
    }

    let bytes_sent = stream.write(&info_file).expect("Failed to write buffer");
    println!("{} bytes sent", bytes_sent);

    let mut buffer = [0; 1024];
    let bytes_read = stream.read(&mut buffer).unwrap();

    let message = Message::new(&buffer, bytes_read);

    if let Ok(Message::Ok) = message {
        println!("Ready to start file transfer.");
    }
}
