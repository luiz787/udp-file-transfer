use std::{io::Read, net::TcpStream, str};
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
            2 => {
                if bytes_read < 6 {
                    return Err("Read fewer than 6 bytes for message that should contain 6 bytes");
                }
                let array = &message_type[2..6];
                let port = u32_from_u8_array(array);

                Ok(Self::Connection(port))
            }
            3 => {
                if bytes_read < 25 {
                    return Err(
                        "Read fewer than 25 bytes for message that should contain 25 bytes",
                    );
                }
                let filename = match str::from_utf8(&message_type[2..17]) {
                    Ok(str) => String::from(str.trim_matches(char::from(0))),
                    Err(_e) => return Err("Falha ao converter bytes para string"),
                };

                let file_size = u64_from_u8_array(&message_type[17..25]);

                Ok(Self::InfoFile(FileData {
                    filename,
                    file_size,
                }))
            }
            4 => Ok(Self::Ok),
            5 => Ok(Self::End),
            6 => {
                if bytes_read < 8 {
                    return Err(
                        "Read fewer than 8 bytes for message that should contain at least 8 bytes",
                    );
                }

                let sequence_number = u32_from_u8_array(&message_type[2..6]);
                let payload_size = u16_from_u8_array(&message_type[6..8]);

                // TODO: avoid clone
                let file_content = (&message_type[8..bytes_read].to_vec()).clone();
                Ok(Self::File(ChunkData {
                    sequence_number,
                    payload_size,
                    data: file_content,
                }))
            }
            7 => {
                if bytes_read < 6 {
                    return Err("Read fewer than 6 bytes for message that should contain 6 bytes");
                }

                let sequence_number = u32_from_u8_array(&message_type[2..6]);
                Ok(Self::Ack(sequence_number))
            }
            _ => Err("Unknown message type"),
        }
    }
}

fn u16_from_u8_array(u8_array: &[u8]) -> u16 {
    ((u8_array[0] as u16) << 8) + ((u8_array[1] as u16) << 0)
}

fn u32_from_u8_array(u8_array: &[u8]) -> u32 {
    ((u8_array[0] as u32) << 24)
        + ((u8_array[1] as u32) << 16)
        + ((u8_array[2] as u32) << 8)
        + ((u8_array[3] as u32) << 0)
}

fn u64_from_u8_array(u8_array: &[u8]) -> u64 {
    ((u8_array[0] as u64) << 56)
        + ((u8_array[1] as u64) << 48)
        + ((u8_array[2] as u64) << 40)
        + ((u8_array[3] as u64) << 32)
        + ((u8_array[4] as u64) << 24)
        + ((u8_array[5] as u64) << 16)
        + ((u8_array[6] as u64) << 8)
        + ((u8_array[7] as u64) << 0)
}

pub fn receive_message(stream: &mut TcpStream) -> Result<Message, &'static str> {
    let mut buffer = [0; 1024];

    let bytes_read = stream.read(&mut buffer);

    match bytes_read {
        Err(_e) => return Err("Falha ao receber da stream TCP"),
        Ok(value) => Message::new(&buffer, value),
    }
}
