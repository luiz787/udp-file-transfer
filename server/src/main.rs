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
        eprintln!("Problema ao interpretar argumentos: {}", err);
        process::exit(1);
    });

    let address = format!("[::]:{}", config.port);
    let udp_port = Arc::new(AtomicU16::new(30000));

    println!("Fazendo bind em {}", address);
    let listener = TcpListener::bind(address)
        .expect(format!("Falha ao realizar bind na porta {}", config.port).as_str());

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
            println!("Fechando conexão");
        });

        println!("Uma thread foi criada para lidar com a conexão. Esperando novas conexões...");
    }
}

fn handle_connection(mut stream: TcpStream, udp_port: Arc<AtomicU16>) -> Result<(), GenericError> {
    // Wait for hello
    common::receive_message(&mut stream)?;

    let port = udp_port.fetch_add(10, std::sync::atomic::Ordering::SeqCst);
    println!("Usará UDP na porta {}", port);
    let udp_socket = UdpSocket::bind(("::", port)).expect("Não foi possível fazer bind UDP");

    GenericError::transform_io(send_connection_message(port, &mut stream))?;

    // Wait for info file
    let message = common::receive_message(&mut stream)?;

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
    println!("Começando a receber o arquivo");
    let expected_chunks = (file_data.file_size / 1000) + 1;
    println!("Quantidade de blocos esperados={}", expected_chunks);
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
        println!("{} bytes lidos do socket udp", bytes_read);
        let message = GenericError::transform_logic(Message::new(&buffer, bytes_read));
        match message {
            Ok(Message::File(ChunkData {
                sequence_number,
                data,
                ..
            })) => {
                println!("Bloco {} recebido", sequence_number);
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
                            println!("Todos os blocos recebidos. Enviando ack para o último.");

                            let mut ack: Vec<u8> = vec![0, 7];
                            let ack_idx: u32 = (expected_chunks - 1) as u32;

                            ack.extend(ack_idx.to_be_bytes().iter());
                            ack_sent = Some(ack_idx);

                            GenericError::transform_io(send_message(stream, &ack))?;
                        } else {
                            for (idx, received) in received_chunks.iter().enumerate() {
                                if !received && idx > 0 {
                                    // Send ack to last that was received.
                                    let mut ack: Vec<u8> = vec![0, 7];
                                    let ack_idx: u32 = (idx as u32) - 1;

                                    ack.extend(ack_idx.to_be_bytes().iter());
                                    ack_sent = Some(ack_idx);

                                    println!("Enviando ack para o bloco {}", ack_sent.unwrap());
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
                                println!("Enviando ack para o último bloco");
                                let mut ack: Vec<u8> = vec![0, 7];
                                let ack_idx: u32 = sequence_number;

                                ack.extend(ack_idx.to_be_bytes().iter());
                                GenericError::transform_io(send_message(stream, &ack))?;

                                println!("Finalizando, uma vez que o último ack foi enviado");
                                break;
                            }
                        }

                        if ack_sent.is_some() && ack_sent.unwrap() as u64 == expected_chunks - 1 {
                            println!("Último ack enviado, finalizando");
                            break;
                        }
                    }
                } else {
                    println!("Bloco recebido está fora da janela.");
                    println!(
                        "Enviando ack para o último bloco válido recebido, lfr={}, seqnum={}",
                        last_chunk_read - 1,
                        sequence_number
                    );

                    let mut ack: Vec<u8> = vec![0, 7];
                    let ack_idx: u32 = last_chunk_read - 1;

                    ack.extend(ack_idx.to_be_bytes().iter());

                    GenericError::transform_io(send_message(stream, &ack))?;
                    if ack_idx as u64 == expected_chunks - 1 {
                        println!("Último ack enviado, finalizando");
                        break;
                    }
                }
            }
            Ok(_val) => {
                let message = "Tipo de mensagem inválido";
                println!("{}", message);
                return Err(GenericError::Logic(MessageCreationError::new(
                    message,
                )));
            }
            Err(e) => return Err(e),
        }
    }
    let mut file =
        GenericError::transform_io(File::create(format!("output/{}", file_data.filename)))?;

    let contents = contents.iter().flat_map(|v| v.clone()).collect::<Vec<_>>();

    GenericError::transform_io(file.write_all(&contents))?;

    println!("Enviando mensagem de fim de transmissão.");
    let fin: Vec<u8> = vec![0, 5];
    let res = GenericError::transform_io(stream.write(&fin));

    res.map(|_val| ())
}
