use std::io::{Error, ErrorKind, Read, Write};
use std::net::TcpStream;

use crate::{Message, MessageCreationError};

pub enum GenericError {
    IO(std::io::Error),
    Logic(MessageCreationError),
}

impl GenericError {
    /// Transforma um std::io::Error em uma instância de GenericError, para facilitar o uso de Result<T, GenericError>.
    pub fn transform_io<T>(original_result: Result<T, std::io::Error>) -> Result<T, GenericError> {
        original_result.map_err(|e| GenericError::IO(e))
    }

    /// Transforma um MessageCreationError em uma instância de GenericError, para facilitar o uso de Result<T, GenericError>.
    pub fn transform_logic<T>(
        original_result: Result<T, MessageCreationError>,
    ) -> Result<T, GenericError> {
        original_result.map_err(|e| GenericError::Logic(e))
    }
}

/// Recebe uma mensagem do socket TCP, e transforma-a numa instância de Message, ou retorna o erro caso algum problema
/// aconteça (erro de I/O ou lógica).
pub fn receive_message(stream: &mut TcpStream) -> Result<Message, GenericError> {
    let mut buffer = [0; 1024];

    let bytes_read = stream
        .read(&mut buffer)
        .map_err(|err| GenericError::IO(err));

    bytes_read.and_then(|value| {
        if value == 0 {
            Err(GenericError::IO(Error::new(
                ErrorKind::ConnectionAborted,
                "Conexão fechada",
            )))
        } else {
            GenericError::transform_logic(Message::new(&buffer, value))
        }
    })
}

/// Envia um array de bytes para o socket TCP, e retorna quantos bytes foram enviados, ou o erro associado.
pub fn send_message(stream: &mut TcpStream, data: &Vec<u8>) -> Result<usize, Error> {
    let bytes_written = stream.write(data);

    bytes_written
}
