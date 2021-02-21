use std::str;

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

impl Message {
    pub fn new(message_type: &[u8], bytes_read: usize) -> Result<Message, &'static str> {
        if bytes_read < 2 {
            return Err("Read fewer than 2 bytes");
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
            _ => Err("Unknown message type"),
        }
    }
}

fn create_connection(bytes_read: usize, message_type: &[u8]) -> Result<Message, &'static str> {
    if bytes_read < 6 {
        return Err("Read fewer than 6 bytes for message that should contain 6 bytes");
    }
    let array = &message_type[2..6];
    let port = byte_utils::u32_from_u8_array(array);

    Ok(Message::Connection(port))
}

fn create_info_file(bytes_read: usize, message_type: &[u8]) -> Result<Message, &'static str> {
    if bytes_read < 25 {
        return Err("Read fewer than 25 bytes for message that should contain 25 bytes");
    }
    let filename = match str::from_utf8(&message_type[2..17]) {
        Ok(str) => String::from(str.trim_matches(char::from(0))),
        Err(_e) => return Err("Falha ao converter bytes para string"),
    };

    let file_size = byte_utils::u64_from_u8_array(&message_type[17..25]);

    Ok(Message::InfoFile(FileData {
        filename,
        file_size,
    }))
}

fn create_file(bytes_read: usize, message_type: &[u8]) -> Result<Message, &'static str> {
    if bytes_read < 8 {
        return Err("Read fewer than 8 bytes for message that should contain at least 8 bytes");
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

fn create_ack(bytes_read: usize, message_type: &[u8]) -> Result<Message, &'static str> {
    if bytes_read < 6 {
        return Err("Read fewer than 6 bytes for message that should contain 6 bytes");
    }

    let sequence_number = byte_utils::u32_from_u8_array(&message_type[2..6]);
    Ok(Message::Ack(sequence_number))
}
