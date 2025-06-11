use crate::ext::io::*;
use crate::utils::bit_stream::*;
use anyhow::Result;
use std::io::Write;

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

pub struct LZWEncoder {
    buf: MemWriter,
}

impl LZWEncoder {
    pub fn new() -> Self {
        LZWEncoder {
            buf: MemWriter::new(),
        }
    }

    pub fn encode(mut self, input: &[u8], fake: bool) -> Result<Vec<u8>> {
        self.buf.write_all(b"acp\0")?;
        self.buf.write_u32_be(input.len() as u32)?;
        let mut writer = BitWriter::new(&mut self.buf);
        if fake {
            for i in 0..input.len() {
                if i > 0 && i % 0x4000 == 0 {
                    writer.put_bits(0x102, 9)?;
                }
                writer.put_bits(input[i] as u32, 9)?;
            }
            writer.put_bits(0x100, 9)?; // End of stream
            writer.flush()?;
        } else {
            let mut dict = std::collections::HashMap::new();
            for i in 0..256 {
                dict.insert(vec![i as u8], i);
            }
            let mut next_code = 0x103u32;
            let mut token_width = 9;

            let mut i = 0;
            while i < input.len() {
                let mut current = vec![input[i]];
                i += 1;

                while i < input.len()
                    && dict.contains_key(&{
                        let mut temp = current.clone();
                        temp.push(input[i]);
                        temp
                    })
                {
                    current.push(input[i]);
                    i += 1;
                }

                let code = dict[&current];
                writer.put_bits(code, token_width)?;

                if i < input.len() {
                    let mut new_entry = current.clone();
                    new_entry.push(input[i]);
                    dict.insert(new_entry, next_code);
                    next_code += 1;

                    if next_code >= (1 << token_width) && token_width < 24 {
                        writer.put_bits(0x101, token_width)?; // Increase token width
                        token_width += 1;
                    }

                    if dict.len() >= 0x8900 {
                        writer.put_bits(0x102, token_width)?; // Clear dictionary
                        dict.clear();
                        for j in 0..256 {
                            dict.insert(vec![j as u8], j);
                        }
                        next_code = 0x103;
                        token_width = 9;
                    }
                }
            }
            writer.put_bits(0x100, token_width)?; // End of stream
            writer.flush()?;
        }

        Ok(self.buf.into_inner())
    }
}
