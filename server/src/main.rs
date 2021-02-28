use core::panic;
use std::fs::File;
use std::net::TcpListener;
use std::process;
use std::thread;
use std::{env, usize};
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

    let address = format!("[::]:{}", config.port);
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
    let udp_socket = UdpSocket::bind(("::", port)).expect("Não foi possível fazer bind UDP");

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
    println!("Starting to receive file");
    let expected_chunks = (file_data.file_size / 1000) + 1;
    println!("Expected chunks={}", expected_chunks);
    let mut contents: Vec<Vec<u8>> = vec![Vec::new(); expected_chunks as usize];
    let mut acked_chunks = vec![false; expected_chunks as usize];
    let mut received_chunks = vec![false; expected_chunks as usize];

    let rws = 10;
    let mut last_acceptable_chunk = rws;
    let mut last_chunk_read = 0;

    loop {
        let mut buffer = [0; 1024];
        let bytes_read = udp_socket.recv(&mut buffer);
        let bytes_read = match bytes_read {
            Ok(amt) => amt,
            _ => 0,
        };
        println!("Read {} from udp socket", bytes_read);
        let message = GenericError::transform_logic(Message::new(&buffer, bytes_read));
        match message {
            Ok(Message::File(ChunkData {
                sequence_number,
                data,
                ..
            })) => {
                println!("Received chunk {}", sequence_number);
                if last_chunk_read <= sequence_number && sequence_number <= last_acceptable_chunk {
                    received_chunks[sequence_number as usize] = true;
                    println!(
                        "Received chunk {}, which is inside the current window.",
                        sequence_number
                    );
                    println!("LAF={}, LFR={}", last_acceptable_chunk, last_chunk_read);
                    contents[sequence_number as usize] = data.clone();

                    let all_received = received_chunks.iter().all(|item| *item);
                    let should_ack = sequence_number == last_chunk_read
                        || sequence_number == last_chunk_read + 1
                        || all_received;
                    if should_ack {
                        let mut ack_sent = None;

                        if received_chunks.iter().all(|item| *item) {
                            println!("All chunks received. Sending ack to last.");

                            let mut ack: Vec<u8> = vec![0, 7];
                            let ack_idx: u32 = (expected_chunks - 1) as u32;

                            ack.extend(ack_idx.to_be_bytes().iter());
                            ack_sent = Some(ack_idx);
                            println!("Sending ack={}", ack_idx);

                            GenericError::transform_io(send_message(stream, &ack))?;
                        } else {
                            for (idx, received) in received_chunks.iter().enumerate() {
                                if !received && idx > 0 {
                                    // Send ack to last that was received.
                                    let mut ack: Vec<u8> = vec![0, 7];
                                    let ack_idx: u32 = (idx as u32) - 1;

                                    ack.extend(ack_idx.to_be_bytes().iter());
                                    ack_sent = Some(ack_idx);

                                    println!("Sending ack={}", ack_sent.unwrap());
                                    GenericError::transform_io(send_message(stream, &ack))?;

                                    for i in last_chunk_read..idx as u32 {
                                        acked_chunks[i as usize] = true;
                                    }
                                    let amt = idx as u32 - last_chunk_read;
                                    last_chunk_read += amt;
                                    last_acceptable_chunk += amt;

                                    println!(
                                        "LAF={}, LFR={}",
                                        last_acceptable_chunk, last_chunk_read
                                    );
                                    break;
                                }
                            }
                        }

                        if ack_sent == None {
                            if sequence_number as u64 == expected_chunks - 1 {
                                println!("Sending ack to last chunk");
                                let mut ack: Vec<u8> = vec![0, 7];
                                let ack_idx: u32 = sequence_number;

                                ack.extend(ack_idx.to_be_bytes().iter());
                                GenericError::transform_io(send_message(stream, &ack))?;

                                println!("Exiting as last ack was sent");
                                break;
                            }
                        }

                        if ack_sent.is_some() && ack_sent.unwrap() as u64 == expected_chunks - 1 {
                            println!("Sent last ack, exiting");
                            // Sent last ack
                            break;
                        }
                    }
                } else {
                    println!("Chunk received is outside window.");
                    println!(
                        "Sending ack to last frame received, lfr={}, seqnum={}",
                        last_chunk_read - 1,
                        sequence_number
                    );

                    let mut ack: Vec<u8> = vec![0, 7];
                    let ack_idx: u32 = last_chunk_read - 1;

                    ack.extend(ack_idx.to_be_bytes().iter());

                    GenericError::transform_io(send_message(stream, &ack))?;
                    println!("AckSent={}, last={}", ack_idx, expected_chunks - 1);
                    if ack_idx as u64 == expected_chunks - 1 {
                        println!("Sent last ack, exiting");
                        // Sent last ack
                        break;
                    }
                }
            }
            Ok(_val) => {
                println!("Invalid message type");
                return Err(GenericError::Logic(MessageCreationError::new(
                    "Invalid message type",
                )));
            }
            Err(e) => return Err(e),
        }
    }
    let mut file =
        GenericError::transform_io(File::create(format!("output/{}", file_data.filename)))?;

    let contents = contents.iter().flat_map(|v| v.clone()).collect::<Vec<_>>();

    GenericError::transform_io(file.write_all(&contents))?;

    println!("Sending fin message...");
    let fin: Vec<u8> = vec![0, 5];
    let res = GenericError::transform_io(stream.write(&fin));

    res.map(|_val| ())
}
