use std::cmp::min;
use std::iter::repeat;
use std::net::IpAddr;
use std::net::TcpStream;
use std::net::UdpSocket;
use std::process;
use std::time::{Duration, Instant};
use std::{env, io::Write};

use common::{receive_message, Message};

mod client_config;
use client_config::ClientConfig;

fn main() {
    let config = ClientConfig::new(env::args()).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {}", err);
        process::exit(1);
    });

    let mut stream =
        TcpStream::connect((config.ip, config.port)).expect("Failed to connect to remote server.");

    let hello = create_hello_message();

    let bytes_sent = stream.write(&hello).expect("Failed to write buffer");
    println!("{} bytes sent", bytes_sent);

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

fn create_info_file_message(config: &ClientConfig, file_contents: &Vec<u8>) -> Vec<u8> {
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
    let socket = UdpSocket::bind((ip, 0)).expect("Failed to bind to UDP socket");

    let chunks = file_contents.chunks(1000).collect::<Vec<_>>();

    let mut next_sequence_number = 0;
    let mut send_base: u32 = 0;
    let window_size: u32 = min(10, chunks.len() as u32);
    let mut last_ack_received = Instant::now();

    while (next_sequence_number as usize) < chunks.len() {
        if next_sequence_number < send_base + window_size {
            let current_chunk = chunks[next_sequence_number as usize];
            println!("Sending chunk {}", next_sequence_number);
            send_file_chunk(
                current_chunk.to_vec(),
                next_sequence_number as u32,
                &socket,
                ip,
                port as u16,
            );

            next_sequence_number += 1;
        }

        let message = receive_message(&mut stream);
        let seq_number = match message {
            Ok(Message::Ack(seq_number)) => seq_number,
            _ => panic!("Unknown problem"),
        };

        while seq_number > send_base {
            last_ack_received = Instant::now();
            send_base += 1;
        }

        let duration = Instant::now() - last_ack_received;
        let timed_out = Duration::from_millis(200).lt(&duration);

        if timed_out {
            println!(
                "Timed out, resending window start={}, end={}",
                send_base, next_sequence_number
            );
            for index in send_base..next_sequence_number {
                let current_chunk = chunks[index as usize];
                send_file_chunk(
                    current_chunk.to_vec(),
                    index as u32,
                    &socket,
                    ip,
                    port as u16,
                );
            }
        }
    }

    let message = receive_message(&mut stream);

    if let Ok(Message::End) = message {
        println!("Got fin message from server, quitting.");
    }
}

fn send_file_chunk(chunk: Vec<u8>, index: u32, socket: &UdpSocket, ip: IpAddr, port: u16) -> () {
    let mut data: Vec<u8> = vec![0, 6];

    let mut chunk = chunk.to_vec();

    let index_bytes = (index as u32).to_be_bytes();
    data.extend(index_bytes.iter());

    let payload_size: u16 = chunk.len() as u16;
    println!("Processing chunk {}, size: {}", index, payload_size);

    data.extend(payload_size.to_be_bytes().iter());
    data.append(&mut chunk);

    let bytes_sent = socket.send_to(&data, (ip, port));
    let bytes_sent = match bytes_sent {
        Err(e) => {
            eprintln!("{}", e);
            // TODO: dont panic
            panic!(e);
        }
        Ok(bytes) => bytes,
    };

    println!("Sent {} bytes", bytes_sent);
}
