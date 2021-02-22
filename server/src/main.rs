use core::panic;
use std::net::TcpListener;
use std::process;
use std::thread;
use std::{collections::BTreeMap, fs::File};
use std::{env, io::ErrorKind};
use std::{
    io::Write,
    net::{TcpStream, UdpSocket},
};

use std::sync::atomic::AtomicU16;
use std::sync::Arc;

use common::{send_message, ChunkData, FileData, GenericError, Message, MessageCreationError};

mod server_config;
use server_config::ServerConfig;

fn main() {
    let config = ServerConfig::new(env::args()).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {}", err);
        process::exit(1);
    });

    let address = format!("0.0.0.0:{}", config.port);
    let udp_port = Arc::new(AtomicU16::new(30000));

    println!("Binding to {}", address);
    let listener = TcpListener::bind(address)
        .expect(format!("Failed to bind to port {}", config.port).as_str());

    for stream in listener.incoming() {
        // TODO: spawn new thread to handle connection
        let stream = stream.unwrap();

        let udp_port_clone = Arc::clone(&udp_port);
        thread::spawn(move || {
            if let Err(msg) = handle_connection(stream, udp_port_clone) {
                match msg {
                    GenericError::IO(e) => {
                        eprintln!("{}", e);
                    }
                    GenericError::Logic(e) => {
                        eprintln!("{}", e);
                    }
                }
            }
            println!("Finishing connection");
        });

        println!("Created a new thread to handle request, listening for other connections.");
    }
}

fn handle_connection(mut stream: TcpStream, udp_port: Arc<AtomicU16>) -> Result<(), GenericError> {
    // Wait for hello
    common::receive_message(&mut stream)?;

    let port = udp_port.fetch_add(10, std::sync::atomic::Ordering::SeqCst);
    println!("Will use udp in port {}", port);
    let udp_socket = UdpSocket::bind(("127.0.0.1", port)).expect("Não foi possível fazer bind UDP");

    GenericError::transform_io(send_connection_message(port, &mut stream))?;

    // Wait for info file
    let message = common::receive_message(&mut stream)?;

    // TODO: remove panic
    let file_data = match message {
        Message::InfoFile(file_data) => file_data,
        _ => panic!("Tipo de mensagem inesperado"),
    };

    GenericError::transform_io(send_ok_message(&mut stream))?;
    receive_file(&mut stream, udp_socket, file_data)
}

fn send_connection_message(udp_port: u16, stream: &mut TcpStream) -> Result<usize, std::io::Error> {
    let data = build_connection_message(udp_port);

    send_message(stream, &data)
}

fn send_ok_message(stream: &mut TcpStream) -> Result<usize, std::io::Error> {
    let data = build_ok_message();

    send_message(stream, &data)
}

fn build_ok_message() -> Vec<u8> {
    vec![0, 4]
}

fn build_connection_message(udp_port: u16) -> Vec<u8> {
    let mut connection: Vec<u8> = vec![0, 2];
    connection.extend((udp_port as u32).to_be_bytes().iter());

    connection
}

fn receive_file(
    stream: &mut TcpStream,
    udp_socket: UdpSocket,
    file_data: FileData,
) -> Result<(), GenericError> {
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

        // TODO: remove thread sleep
        thread::sleep(std::time::Duration::from_secs(5));
        let message = GenericError::transform_logic(Message::new(&buffer, bytes_read));

        match message {
            Ok(Message::File(ChunkData {
                sequence_number,
                data,
                payload_size,
            })) => {
                if !map.contains_key(&sequence_number) {
                    map.insert(sequence_number, data);
                    total_received += payload_size as u64;
                }
                let mut ack: Vec<u8> = vec![0, 7];
                ack.extend(sequence_number.to_be_bytes().iter());

                GenericError::transform_io(send_message(stream, &ack))?;
            }
            Ok(_val) => {
                return Err(GenericError::Logic(MessageCreationError::new(
                    "Invalid message type",
                )))
            }
            Err(e) => return Err(e),
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

    if let Err(e) = std::fs::create_dir("output") {
        if e.kind() == ErrorKind::AlreadyExists {
            println!("Output folder already exists, continuing");
        } else {
            eprintln!("Output folder was not created: {}", e);
            return Err(GenericError::IO(e));
        }
    }
    let mut file =
        GenericError::transform_io(File::create(format!("output/{}", file_data.filename)))?;

    GenericError::transform_io(file.write_all(&file_contents))?;

    let fin: Vec<u8> = vec![0, 5];
    let res = GenericError::transform_io(stream.write(&fin));

    res.map(|_val| ())
}
