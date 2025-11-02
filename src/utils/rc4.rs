use std::io::{Read, Write};

pub struct Rc4 {
    state: [u8; 256],
    i: u8,
    j: u8,
}

impl Rc4 {
    pub fn new(key: &[u8]) -> Self {
        let mut state = [0u8; 256];
        for i in 0..256 {
            state[i] = i as u8;
        }

        let mut j: u8 = 0;
        for i in 0..256 {
            j = j.wrapping_add(state[i]).wrapping_add(key[i % key.len()]);
            state.swap(i, j as usize);
        }

        Rc4 { state, i: 0, j: 0 }
    }

    pub fn next_byte(&mut self) -> u8 {
        self.i = self.i.wrapping_add(1);
        self.j = self.j.wrapping_add(self.state[self.i as usize]);
        self.state.swap(self.i as usize, self.j as usize);
        let k = self.state
            [(self.state[self.i as usize].wrapping_add(self.state[self.j as usize])) as usize];
        k
    }

    pub fn skip_bytes(&mut self, n: usize) {
        for _ in 0..n {
            self.next_byte();
        }
    }

    pub fn generate_block(&mut self, len: usize) -> Vec<u8> {
        (0..len).map(|_| self.next_byte()).collect()
    }

    pub fn process_block(&mut self, data: &mut [u8]) {
        for byte in data.iter_mut() {
            *byte ^= self.next_byte();
        }
    }
}

pub struct Rc4Stream<T> {
    inner: T,
    rc4: Rc4,
}

impl<T> Rc4Stream<T> {
    pub fn new(inner: T, rc4: Rc4) -> Self {
        Rc4Stream { inner, rc4 }
    }

    pub fn new_with_key(inner: T, key: &[u8]) -> Self {
        Rc4Stream {
            inner,
            rc4: Rc4::new(key),
        }
    }
}

impl<T: Read> Read for Rc4Stream<T> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.rc4.process_block(&mut buf[..n]);
        Ok(n)
    }
}

impl<T: Write> Write for Rc4Stream<T> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut data = buf.to_vec();
        self.rc4.process_block(&mut data);
        self.inner.write(&data)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
