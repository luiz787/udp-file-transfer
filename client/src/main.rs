use std::iter::repeat;
use std::net::IpAddr;
use std::net::TcpStream;
use std::net::UdpSocket;
use std::process;
use std::time::{Duration, Instant};
use std::{cmp::min, io::ErrorKind, sync::mpsc, thread};
use std::{env, io::Write};

use common::{receive_message, GenericError, Message};

mod client_config;
use client_config::ClientConfig;

fn main() {
    let config = ClientConfig::new(env::args()).unwrap_or_else(|err| {
        eprintln!("Problema ao interpretar argumentos: {}", err);
        process::exit(1);
    });

    let mut stream =
        TcpStream::connect((config.ip, config.port)).expect("Falha ao conectar com o servidor remoto.");

    let hello = create_hello_message();

    stream.write(&hello).expect("Falha ao enviar bytes.");

    let message = common::receive_message(&mut stream);

    let port = match message {
        Ok(Message::Connection(port)) => port,
        _ => panic!("Não foi possível obter a porta UDP"),
    };

    println!("Porta UDP é: {}", port);

    let file_contents = std::fs::read(&config.filename.filename).expect("Falha ao abrir o arquivo");
    let info_file = create_info_file_message(&config, &file_contents);

    let bytes_sent = stream.write(&info_file).expect("Falha ao enviar bytes.");
    println!("{} bytes enviados", bytes_sent);

    let message = common::receive_message(&mut stream);

    if let Ok(Message::Ok) = message {
        println!("Pronto para iniciar transmissão do arquivo.");
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
    println!("Tamanho do arquivo: {}", file_contents.len());
    let socket = UdpSocket::bind((ip, 0)).expect("Falha ao fazer bind no socket UDP");

    // Channel to transmit sequence_numbers
    let (tx_sequence_numbers, rx_sequence_numbers) = mpsc::channel::<u32>();

    // Channel for the main thread to send a signal to UDP thread to finish
    let (tx_continue, rx_continue) = mpsc::channel::<()>();

    let last_chunk = file_contents.chunks(1000).collect::<Vec<_>>().len() - 1;

    let udp_thread_handle = thread::spawn(move || {
        send_file_chunks(
            file_contents,
            rx_continue,
            socket,
            ip,
            port,
            rx_sequence_numbers,
        );
    });

    let mut connection_closed = false;

    loop {
        let message = receive_message(&mut stream);
        let seq_number = match message {
            Ok(Message::Ack(seq_number)) => seq_number,
            Ok(Message::End) => {
                break;
            }
            Ok(_m) => {
                continue;
            }
            Err(e) => match e {
                GenericError::IO(io_error) => {
                    if io_error.kind() == ErrorKind::ConnectionAborted {
                        connection_closed = true;
                        tx_continue.send(()).unwrap();
                        println!("Arquivo enviado com sucesso.");
                        break;
                    }
                    println!("{}", io_error);
                    panic!("Failed to get msg");
                }
                GenericError::Logic(msg_error) => {
                    println!("{}", msg_error);
                    continue;
                }
            },
        };

        match tx_sequence_numbers.send(seq_number) {
            Ok(_v) => {}
            Err(_e) => println!("Udp thread already died"),
        }

        if seq_number == last_chunk as u32 {
            break;
        }
    }

    udp_thread_handle.join().unwrap();

    // If the connection was already closed, there's no point in trying to receive the "Finish" message.
    if !connection_closed {
        let message = receive_message(&mut stream);

        if let Ok(Message::End) = message {
            println!("Arquivo enviado com sucesso.");
        }
    }
}

fn send_file_chunks(
    file_contents: Vec<u8>,
    rx_continue: mpsc::Receiver<()>,
    socket: UdpSocket,
    ip: IpAddr,
    port: u32,
    rx_sequence_numbers: mpsc::Receiver<u32>,
) {
    let chunks = file_contents.chunks(1000).collect::<Vec<_>>();

    let mut next_sequence_number = 0;
    let mut send_base: u32 = 0;
    let window_size: u32 = min(10, chunks.len() as u32);
    let mut last_ack_received = Instant::now();
    
    let is_single_chunk = chunks.len() == 1 && send_base == 0;
    while (send_base as usize) < chunks.len() - 1 || is_single_chunk {
        if let Ok(()) = rx_continue.try_recv() {
            break;
        }

        if (next_sequence_number as usize) < chunks.len()
            && next_sequence_number < send_base + window_size
        {
            let current_chunk = chunks[next_sequence_number as usize];
            send_file_chunk(
                current_chunk.to_vec(),
                next_sequence_number as u32,
                &socket,
                ip,
                port as u16,
            );

            next_sequence_number += 1;
        }

        let seq_number = rx_sequence_numbers.try_recv();

        match seq_number {
            Ok(num) => {
                // Se é chunk único, só pode ter recebido o número de sequência zero, então é seguro finalizar o loop.
                if is_single_chunk {
                    break;
                }
                while num > send_base {
                    last_ack_received = Instant::now();
                    send_base += 1;
                }
                continue;
            }
            Err(_e) => {}
        };
        let duration = Instant::now() - last_ack_received;
        let timed_out = Duration::from_millis(200).lt(&duration);

        // Sleep to yield thread (in some cases, the udp thread was making the tcp thread starve).
        thread::sleep(Duration::from_millis(5));

        if timed_out {
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
}

fn send_file_chunk(chunk: Vec<u8>, index: u32, socket: &UdpSocket, ip: IpAddr, port: u16) -> () {
    let mut data: Vec<u8> = vec![0, 6];

    let mut chunk = chunk.to_vec();

    let index_bytes = (index as u32).to_be_bytes();
    data.extend(index_bytes.iter());

    let payload_size: u16 = chunk.len() as u16;

    data.extend(payload_size.to_be_bytes().iter());
    data.append(&mut chunk);

    let bytes_sent = socket.send_to(&data, (ip, port));

    if let Err(e) = bytes_sent {
        eprintln!("{}", e);
        panic!(e);
    }
}
