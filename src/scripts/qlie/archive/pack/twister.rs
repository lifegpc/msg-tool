const DEFAULT_SEED: u32 = 5489;
const STATE_LENGTH: usize = 64;
const STATE_M: usize = 39;
const MATRIX_A: u32 = 0x9908B0DF;
const SIGN_MASK: u32 = 0x80000000;
const LOWER_MASK: u32 = 0x7FFFFFFF;
const TEMPERING_MASK_B: u32 = 0x9C4F88E3;
const TEMPERING_MASK_C: u32 = 0xE7F70000;

pub struct MersenneTwister {
    mt: [u32; STATE_LENGTH],
    mti: usize,
}

impl MersenneTwister {
    pub fn new(seed: u32) -> Self {
        let mut twister = Self {
            mt: [0; STATE_LENGTH],
            mti: STATE_LENGTH,
        };
        twister.s_rand(seed);
        twister
    }

    pub fn s_rand(&mut self, seed: u32) {
        self.mt[0] = seed;
        for i in 1..STATE_LENGTH {
            self.mt[i] = (0x6611BC19u32.wrapping_mul(self.mt[i - 1] ^ (self.mt[i - 1] >> 30)))
                .wrapping_add(i as u32);
        }
    }

    pub fn xor_state(&mut self, hash: &[u8]) {
        let length = (hash.len() / 4).min(STATE_LENGTH);
        if length == 0 {
            return;
        }
        for i in 0..length {
            let part = u32::from_le_bytes([
                hash[i * 4],
                hash[i * 4 + 1],
                hash[i * 4 + 2],
                hash[i * 4 + 3],
            ]);
            self.mt[i] ^= part;
        }
    }

    pub fn rand(&mut self) -> u32 {
        const MAG01: [u32; 2] = [0, MATRIX_A];

        if self.mti >= STATE_LENGTH {
            for kk in 0..(STATE_LENGTH - STATE_M) {
                let y = (self.mt[kk] & SIGN_MASK) | (self.mt[kk + 1] & LOWER_MASK) >> 1;
                self.mt[kk] = self.mt[kk + STATE_M] ^ y ^ MAG01[(self.mt[kk + 1] & 1) as usize];
            }
            for kk in (STATE_LENGTH - STATE_M)..(STATE_LENGTH - 1) {
                let y = (self.mt[kk] & SIGN_MASK) | (self.mt[kk + 1] & LOWER_MASK) >> 1;
                self.mt[kk] = self.mt[kk - (STATE_LENGTH - STATE_M)]
                    ^ y
                    ^ MAG01[(self.mt[kk + 1] & 1) as usize];
            }
            let y = (self.mt[STATE_LENGTH - 1] & SIGN_MASK) | (self.mt[0] & LOWER_MASK) >> 1;
            self.mt[STATE_LENGTH - 1] =
                self.mt[STATE_M - 1] ^ y ^ MAG01[(self.mt[STATE_LENGTH - 2] & 1) as usize];

            self.mti = 0;
        }

        let mut y = self.mt[self.mti];
        self.mti += 1;

        y ^= y >> 11;
        y ^= (y << 7) & TEMPERING_MASK_B;
        y ^= (y << 15) & TEMPERING_MASK_C;
        y ^= y >> 18;

        y
    }

    pub fn rand64(&mut self) -> u64 {
        let low = self.rand() as u64;
        let high = self.rand() as u64;
        (high << 32) | low
    }
}
