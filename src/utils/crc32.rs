use lazy_static::lazy_static;

fn get_crc32normal_table() -> [u32; 256] {
    let mut table = [0; 256];
    for i in 0..256u32 {
        let mut c = i << 24;
        for _ in 0..8 {
            if c & 0x80000000 != 0 {
                c = (c << 1) ^ 0x04C11DB7; // Polynomial for CRC-32
            } else {
                c <<= 1;
            }
        }
        table[i as usize] = c;
    }
    table
}

lazy_static! {
    pub static ref CRC32NORMAL_TABLE: [u32; 256] = get_crc32normal_table();
}

pub struct Crc32Normal {
    crc: u32,
}

impl Crc32Normal {
    pub fn new() -> Self {
        Crc32Normal { crc: 0xFFFFFFFF }
    }

    pub fn update_crc(init_crc: u32, data: &[u8]) -> u32 {
        let mut crc = init_crc;
        for &byte in data {
            let index = ((crc >> 24) ^ byte as u32) & 0xFF;
            crc = (crc << 8) ^ CRC32NORMAL_TABLE[index as usize];
        }
        crc ^ 0xFFFFFFFF
    }

    pub fn update(&mut self, data: &[u8]) {
        self.crc = Self::update_crc(self.crc, data);
    }

    pub fn value(&self) -> u32 {
        self.crc
    }
}
