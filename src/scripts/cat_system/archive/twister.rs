const STATE_LENGTH: usize = 624;
const STATE_M: usize = 397;
const MATRIX_A: u32 = 0x9908B0DF;
const SIGN_MASK: u32 = 0x80000000;
const LOWER_MASK: u32 = 0x7FFFFFFF;
const TEMPERING_MASK_B: u32 = 0x9D2C5680;
const TEMPERING_MASK_C: u32 = 0xEFC60000;
const DEFAULT_SEED: u32 = 4357;

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

    pub fn s_rand(&mut self, mut seed: u32) {
        for i in 0..STATE_LENGTH {
            let upper = seed & 0xffff0000;
            seed = seed.wrapping_mul(69069).wrapping_add(1);
            self.mt[i] = upper | ((seed & 0xffff0000) >> 16);
            seed = seed.wrapping_mul(69069).wrapping_add(1);
        }
        self.mti = STATE_LENGTH;
    }

    pub fn rand(&mut self) -> u32 {
        const MAG01: [u32; 2] = [0, MATRIX_A];

        if self.mti >= STATE_LENGTH {
            for kk in 0..(STATE_LENGTH - STATE_M) {
                let y = (self.mt[kk] & SIGN_MASK) | (self.mt[kk + 1] & LOWER_MASK);
                self.mt[kk] = self.mt[kk + STATE_M] ^ (y >> 1) ^ MAG01[(y & 1) as usize];
            }
            for kk in (STATE_LENGTH - STATE_M)..(STATE_LENGTH - 1) {
                let y = (self.mt[kk] & SIGN_MASK) | (self.mt[kk + 1] & LOWER_MASK);
                self.mt[kk] =
                    self.mt[kk - (STATE_LENGTH - STATE_M)] ^ (y >> 1) ^ MAG01[(y & 1) as usize];
            }
            let y = (self.mt[STATE_LENGTH - 1] & SIGN_MASK) | (self.mt[0] & LOWER_MASK);
            self.mt[STATE_LENGTH - 1] = self.mt[STATE_M - 1] ^ (y >> 1) ^ MAG01[(y & 1) as usize];

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
}

impl Default for MersenneTwister {
    fn default() -> Self {
        Self::new(DEFAULT_SEED)
    }
}
