use crate::ext::io::*;
use anyhow::Result;

pub struct BitStream<'a> {
    m_input: MemReaderRef<'a>,
    m_bits: u32,
    m_cached_bits: u32,
}

impl<'a> BitStream<'a> {
    pub fn new(input: MemReaderRef<'a>) -> Self {
        BitStream {
            m_input: input,
            m_bits: 0,
            m_cached_bits: 0,
        }
    }

    pub fn get_bits(&mut self, count: u32) -> Result<u32> {
        while self.m_cached_bits < count {
            let byte = self.m_input.read_u8()?;
            self.m_bits = (self.m_bits << 8) | byte as u32;
            self.m_cached_bits += 8;
        }
        let mask = (1 << count) - 1;
        self.m_cached_bits -= count;
        let result = (self.m_bits >> self.m_cached_bits) & mask;
        Ok(result)
    }
}

pub struct LZWDecoder<'a> {
    m_input: BitStream<'a>,
    m_output_size: u32,
}

impl<'a> LZWDecoder<'a> {
    pub fn new(input: &'a [u8]) -> Result<Self> {
        let mut input_reader = MemReaderRef::new(input);
        let size = input_reader.peek_u32_be_at(0x4)?;
        let m_input = BitStream::new(MemReaderRef::new(&input[0x8..]));
        Ok(LZWDecoder {
            m_input,
            m_output_size: size,
        })
    }

    pub fn unpack(&mut self) -> Result<Vec<u8>> {
        let size = self.m_output_size as usize;
        let mut output = Vec::with_capacity(size);
        output.resize(size, 0);
        let mut dict = Vec::with_capacity(0x8900);
        dict.resize(0x8900, 0u32);
        let mut token_width = 9;
        let mut dict_pos = 0;
        let mut dst = 0;
        while dst < size {
            let mut token = self.m_input.get_bits(token_width)?;
            if token == 0x100 {
                // End of stream
                break;
            } else if token == 0x101 {
                token_width += 1;
                if token_width > 24 {
                    return Err(anyhow::anyhow!("Token width exceeded maximum of 12 bits"));
                }
            } else if token == 0x102 {
                token_width = 9;
                dict_pos = 0;
            } else {
                if dict_pos > 0x8900 {
                    return Err(anyhow::anyhow!(
                        "Dictionary position exceeded maximum of 0x8900"
                    ));
                }
                dict[dict_pos] = dst as u32;
                dict_pos += 1;
                if token < 0x100 {
                    output[dst] = token as u8;
                    dst += 1;
                } else {
                    token -= 0x103;
                    if token >= dict_pos as u32 {
                        return Err(anyhow::anyhow!("Token out of bounds: {}", token));
                    }
                    let src = dict[token as usize];
                    let count =
                        (self.m_output_size - dst as u32).min(dict[token as usize + 1] - src + 1);
                    for i in 0..count {
                        output[dst + i as usize] = output[src as usize + i as usize];
                    }
                    dst += count as usize;
                }
            }
        }
        Ok(output)
    }
}
