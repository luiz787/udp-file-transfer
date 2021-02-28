use std::str;
use std::{error::Error, fmt};

use crate::byte_utils;

pub struct FileData {
    pub filename: String,
    pub file_size: u64,
}

pub struct ChunkData {
    pub sequence_number: u32,
    pub payload_size: u16,
    pub data: Vec<u8>,
}
pub enum Message {
    Hello,
    Connection(u32),
    InfoFile(FileData),
    Ok,
    End,
    File(ChunkData),
    Ack(u32),
}

#[derive(Debug)]
pub struct MessageCreationError {
    msg: String,
}

impl MessageCreationError {
    pub fn new(msg: &str) -> MessageCreationError {
        MessageCreationError {
            msg: msg.to_string(),
        }
    }
}

impl fmt::Display for MessageCreationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl Error for MessageCreationError {
    fn description(&self) -> &str {
        &self.msg
    }
}

impl Message {
    pub fn new(message_type: &[u8], bytes_read: usize) -> Result<Message, MessageCreationError> {
        if bytes_read < 2 {
            return Err(MessageCreationError::new("Foram lidos menos de 2 bytes, o que é insuficiente para determinar o tipo de mensagem"));
        }
        let message_type_byte = message_type[1];

        match message_type_byte {
            1 => Ok(Self::Hello),
            2 => create_connection(bytes_read, &message_type),
            3 => create_info_file(bytes_read, &message_type),
            4 => Ok(Self::Ok),
            5 => Ok(Self::End),
            6 => create_file(bytes_read, &message_type),
            7 => create_ack(bytes_read, &message_type),
            other => {
                println!("Tipo de mensagem ({}) desconhecido.", other);
                Err(MessageCreationError::new("Tipo de mensagem desconhecido."))
            }
        }
    }
}

fn create_connection(
    bytes_read: usize,
    message_type: &[u8],
) -> Result<Message, MessageCreationError> {
    if bytes_read < 6 {
        return Err(MessageCreationError::new(
            "Foram lidos menos de 6 bytes para uma mensagem que deve conter no mínimo 6 bytes",
        ));
    }
    let array = &message_type[2..6];
    let port = byte_utils::u32_from_u8_array(array);

    Ok(Message::Connection(port))
}

fn create_info_file(
    bytes_read: usize,
    message_type: &[u8],
) -> Result<Message, MessageCreationError> {
    if bytes_read < 25 {
        return Err(MessageCreationError::new(
            "Foram lidos menos de 25 bytes para uma mensagem que deve conter no mínimo 25 bytes",
        ));
    }
    let filename = match str::from_utf8(&message_type[2..17]) {
        Ok(str) => String::from(str.trim_matches(char::from(0))),
        Err(_e) => {
            return Err(MessageCreationError::new(
                "Falha ao converter bytes para string",
            ))
        }
    };

    let file_size = byte_utils::u64_from_u8_array(&message_type[17..25]);

    Ok(Message::InfoFile(FileData {
        filename,
        file_size,
    }))
}

fn create_file(bytes_read: usize, message_type: &[u8]) -> Result<Message, MessageCreationError> {
    if bytes_read < 8 {
        return Err(MessageCreationError::new(
            "Foram lidos menos de 8 bytes para uma mensagem que deve conter no mínimo 8 bytes",
        ));
    }

    let sequence_number = byte_utils::u32_from_u8_array(&message_type[2..6]);
    let payload_size = byte_utils::u16_from_u8_array(&message_type[6..8]);

    // TODO: avoid clone
    let file_content = (&message_type[8..bytes_read].to_vec()).clone();
    Ok(Message::File(ChunkData {
        sequence_number,
        payload_size,
        data: file_content,
    }))
}

fn create_ack(bytes_read: usize, message_type: &[u8]) -> Result<Message, MessageCreationError> {
    if bytes_read < 6 {
        return Err(MessageCreationError::new(
            "Foram lidos menos de 6 bytes para uma mensagem que deve conter no mínimo 6 bytes",
        ));
    }

    let sequence_number = byte_utils::u32_from_u8_array(&message_type[2..6]);
    Ok(Message::Ack(sequence_number))
}
