use core::panic;
use std::env;
use std::net::TcpListener;
use std::process;
use std::{collections::BTreeMap, fs::File, str};
use std::{
    io::Write,
    net::{TcpStream, UdpSocket},
};

use common::{send_message, ChunkData, FileData, Message};

mod server_config;
use server_config::ServerConfig;

fn main() {
    let config = ServerConfig::new(env::args()).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {}", err);
        process::exit(1);
    });

    let address = format!("0.0.0.0:{}", config.port);

    println!("Binding to {}", address);
    let listener = TcpListener::bind(address)
        .expect(format!("Failed to bind to port {}", config.port).as_str());

    for stream in listener.incoming() {
        // TODO: spawn new thread to handle connection
        let stream = stream.unwrap();

        if let Err(msg) = handle_connection(stream) {
            eprintln!("{}", msg);
        }

        println!("Finishing connection");
    }
}

fn handle_connection(mut stream: TcpStream) -> Result<(), &'static str> {
    // Wait for hello
    common::receive_message(&mut stream)?;

    // TODO: this variable will be mutable
    let udp_port: u32 = 30000;
    let udp_socket =
        UdpSocket::bind(("127.0.0.1", udp_port as u16)).expect("Não foi possível fazer bind UDP");

    send_connection_message(udp_port, &mut stream)?;

    // Wait for info file
    let message = common::receive_message(&mut stream);

    // TODO: remove panic
    let file_data = match message {
        Ok(Message::InfoFile(file_data)) => file_data,
        _ => panic!("Não foi possível obter dados do arquivo"),
    };

    send_ok_message(&mut stream)?;

    receive_file(&mut stream, udp_socket, file_data);

    Ok(())
}

fn send_connection_message(udp_port: u32, stream: &mut TcpStream) -> Result<usize, &'static str> {
    let data = build_connection_message(udp_port);

    send_message(stream, &data)
}

fn send_ok_message(stream: &mut TcpStream) -> Result<usize, &'static str> {
    let data = build_ok_message();

    send_message(stream, &data)
}

fn build_ok_message() -> Vec<u8> {
    vec![0, 4]
}

fn build_connection_message(udp_port: u32) -> Vec<u8> {
    let mut connection: Vec<u8> = vec![0, 2];
    connection.extend(udp_port.to_be_bytes().iter());

    connection
}

fn receive_file(stream: &mut TcpStream, udp_socket: UdpSocket, file_data: FileData) {
    let mut map: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let mut total_received: u64 = 0;
    while total_received < file_data.file_size {
        let mut buffer = [0; 1024];
        let bytes_read = udp_socket.recv(&mut buffer);
        let bytes_read = match bytes_read {
            Ok(amt) => amt,
            _ => 0,
        };
        println!("Read {} from udp socket", bytes_read);
        let message = Message::new(&buffer, bytes_read);
        if let Ok(Message::File(ChunkData {
            sequence_number,
            data,
            payload_size,
        })) = message
        {
            if !map.contains_key(&sequence_number) {
                map.insert(sequence_number, data);
                total_received += payload_size as u64;
            }
            let mut ack: Vec<u8> = vec![0, 7];
            for byte in sequence_number.to_be_bytes().iter() {
                ack.push(*byte);
            }

            let bytes_written = stream.write(&ack);

            if let Err(_e) = bytes_written {
                panic!("Failed to send bytes");
            }
        }
    }

    let mut file_contents: Vec<u8> = Vec::with_capacity(total_received as usize);
    for (seq_number, chunk) in map.iter() {
        println!("Seq number: {}, size: {}", seq_number, chunk.len());
        for byte in chunk {
            file_contents.push(*byte);
        }
    }

    println!(
        "Size of content to be written to file: {}",
        file_contents.len()
    );

    let mut file = File::create("debug.txt").expect("Failed to create output file.");

    file.write_all(&file_contents)
        .expect("Failed to write to file.");

    let fin: Vec<u8> = vec![0, 5];
    let res = stream.write(&fin);
    if let Err(_e) = res {
        panic!("Falha ao enviar mensagem de fim de conexão.");
    }
}
