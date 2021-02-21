pub fn u16_from_u8_array(u8_array: &[u8]) -> u16 {
    ((u8_array[0] as u16) << 8) + ((u8_array[1] as u16) << 0)
}

pub fn u32_from_u8_array(u8_array: &[u8]) -> u32 {
    ((u8_array[0] as u32) << 24)
        + ((u8_array[1] as u32) << 16)
        + ((u8_array[2] as u32) << 8)
        + ((u8_array[3] as u32) << 0)
}

pub fn u64_from_u8_array(u8_array: &[u8]) -> u64 {
    ((u8_array[0] as u64) << 56)
        + ((u8_array[1] as u64) << 48)
        + ((u8_array[2] as u64) << 40)
        + ((u8_array[3] as u64) << 32)
        + ((u8_array[4] as u64) << 24)
        + ((u8_array[5] as u64) << 16)
        + ((u8_array[6] as u64) << 8)
        + ((u8_array[7] as u64) << 0)
}
