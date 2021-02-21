use std::io::{Read, Write};
use std::net::TcpStream;

use crate::Message;

pub fn receive_message(stream: &mut TcpStream) -> Result<Message, &'static str> {
    let mut buffer = [0; 1024];

    let bytes_read = stream.read(&mut buffer);

    match bytes_read {
        Err(_e) => return Err("Falha ao receber da stream TCP"),
        Ok(value) => Message::new(&buffer, value),
    }
}

pub fn send_message(stream: &mut TcpStream, data: &Vec<u8>) -> Result<usize, &'static str> {
    let bytes_written = stream.write(data);

    match bytes_written {
        Err(_e) => return Err("Failed to send bytes"),
        Ok(amt) => Ok(amt),
    }
}
