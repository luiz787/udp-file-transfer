use std::io::{Error, Read, Write};
use std::net::TcpStream;

use crate::{Message, MessageCreationError};

pub enum GenericError {
    IO(std::io::Error),
    Logic(MessageCreationError),
}

impl GenericError {
    pub fn transform_io<T>(original_result: Result<T, std::io::Error>) -> Result<T, GenericError> {
        original_result.map_err(|e| GenericError::IO(e))
    }

    pub fn transform_logic<T>(
        original_result: Result<T, MessageCreationError>,
    ) -> Result<T, GenericError> {
        original_result.map_err(|e| GenericError::Logic(e))
    }
}

pub fn receive_message(stream: &mut TcpStream) -> Result<Message, GenericError> {
    let mut buffer = [0; 1024];

    let bytes_read = stream
        .read(&mut buffer)
        .map_err(|err| GenericError::IO(err));

    bytes_read
        .and_then(|value| Message::new(&buffer, value).map_err(|err| GenericError::Logic(err)))
}

pub fn send_message(stream: &mut TcpStream, data: &Vec<u8>) -> Result<usize, Error> {
    let bytes_written = stream.write(data);

    bytes_written
}
