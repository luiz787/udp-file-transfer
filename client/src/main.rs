use std::io::prelude::*;
use std::iter::repeat;
use std::net::IpAddr;
use std::net::TcpStream;
use std::net::UdpSocket;
use std::process;
use std::{env, io::Write};

use common::Message;

mod config;
use config::Config;

fn main() {
    let config = Config::new(env::args()).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {}", err);
        process::exit(1);
    });

    let mut stream =
        TcpStream::connect((config.ip, config.port)).expect("Failed to connect to remote server.");

    let hello = create_hello_message();

    let bytes_sent = stream.write(&hello).expect("Failed to write buffer");
    println!("{} bytes sent", bytes_sent);

    // TODO: handle error
    let message = common::receive_message(&mut stream);

    let port = match message {
        Ok(Message::Connection(port)) => port,
        _ => panic!("Não foi possível obter a porta UDP"),
    };

    println!("Port is {}", port);

    let file_contents = std::fs::read(&config.filename.filename).expect("Falha ao abrir o arquivo");
    let info_file = create_info_file_message(&config, &file_contents);

    let bytes_sent = stream.write(&info_file).expect("Failed to write buffer");
    println!("{} bytes sent", bytes_sent);

    let message = common::receive_message(&mut stream);

    if let Ok(Message::Ok) = message {
        println!("Ready to start file transfer.");
    }

    transfer_file(stream, config.ip, port, file_contents);
}

fn create_info_file_message(config: &Config, file_contents: &Vec<u8>) -> Vec<u8> {
    let mut info_file: Vec<u8> = vec![0, 3];
    let filename = config.filename.filename.as_bytes();

    let padding_zeroes_iterator = repeat(0).take(15 - filename.len());
    info_file.extend(padding_zeroes_iterator);
    info_file.extend(filename.iter());
    info_file.extend(file_contents.len().to_be_bytes().iter());

    info_file
}

fn create_hello_message() -> Vec<u8> {
    vec![0, 1]
}

fn transfer_file(mut stream: TcpStream, ip: IpAddr, port: u32, file_contents: Vec<u8>) -> () {
    println!("Length of file: {}", file_contents.len());
    let socket = UdpSocket::bind((ip, (port + 1) as u16)).expect("Failed to bind to UDP socket");

    for (index, chunk) in file_contents.chunks(1000).enumerate() {
        let mut data: Vec<u8> = vec![0, 6];

        let mut chunk = chunk.to_vec();
        for byte in (index as u32).to_be_bytes().iter() {
            data.push(*byte);
        }

        let payload_size: u16 = chunk.len() as u16;
        println!("Processing chunk {}, size: {}", index, payload_size);
        for byte in payload_size.to_be_bytes().iter() {
            data.push(*byte);
        }

        data.append(&mut chunk);
        let bytes_sent = socket.send_to(&data, (ip, port as u16));
        let bytes_sent = match bytes_sent {
            Err(e) => {
                eprintln!("{}", e);
                panic!(e);
            }
            Ok(bytes) => bytes,
        };

        println!("Sent {} bytes", bytes_sent);

        let mut buffer = [0; 1024];
        let bytes_read = stream.read(&mut buffer).unwrap();

        let message = Message::new(&buffer, bytes_read);

        if let Ok(Message::Ack(seq_number)) = message {
            println!(
                "Received ack for message {}. Processing next sequence.",
                seq_number
            );
        }
    }

    let mut buffer = [0; 1024];
    let bytes_read = stream.read(&mut buffer).unwrap();
    let message = Message::new(&buffer, bytes_read);

    if let Ok(Message::End) = message {
        println!("Got fin message from server, quitting.");
    }
}
