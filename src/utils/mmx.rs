//! MMX Operations in pure Rust

/// Packed Add of Bytes (8-bit integers)
pub const fn mmx_p_add_b(a: u64, b: u64) -> u64 {
    let mut mask = 0xFF;
    let mut r = ((a & mask).wrapping_add(b & mask)) & mask; // First byte
    mask <<= 8;
    r |= ((a & mask).wrapping_add(b & mask)) & mask; // Second byte
    mask <<= 8;
    r |= ((a & mask).wrapping_add(b & mask)) & mask; // Third byte
    mask <<= 8;
    r |= ((a & mask).wrapping_add(b & mask)) & mask; // Fourth byte
    mask <<= 8;
    r |= ((a & mask).wrapping_add(b & mask)) & mask; // Fifth byte
    mask <<= 8;
    r |= ((a & mask).wrapping_add(b & mask)) & mask; // Sixth byte
    mask <<= 8;
    r |= ((a & mask).wrapping_add(b & mask)) & mask; // Seventh byte
    mask <<= 8;
    r |= ((a & mask).wrapping_add(b & mask)) & mask; // Eighth byte
    r
}

/// Packed Add of Doublewords (32-bit integers)
pub const fn mmx_p_add_d(a: u64, b: u64) -> u64 {
    let mut mask = 0xffffffff;
    let r = ((a & mask).wrapping_add(b & mask)) & mask;
    mask <<= 32;
    r | ((a & mask).wrapping_add(b & mask)) & mask
}

/// Packed Add of Words (16-bit integers)
pub const fn mmx_p_add_w(a: u64, b: u64) -> u64 {
    let mut mask = 0xFFFF;
    let mut r = ((a & mask).wrapping_add(b & mask)) & mask;
    mask <<= 16;
    r |= ((a & mask).wrapping_add(b & mask)) & mask;
    mask <<= 16;
    r |= ((a & mask).wrapping_add(b & mask)) & mask;
    mask <<= 16;
    r |= ((a & mask).wrapping_add(b & mask)) & mask;
    r
}

/// Packed Shift Left Logical of Doublewords (32-bit integers)
pub const fn mmx_p_sll_d(a: u64, mut count: u32) -> u64 {
    count &= 0x1F;
    let mut mask = (0xFFFFFFFFu32 << count) as u64;
    mask |= mask << 32;
    (a << count) & mask
}

/// Packed Shift Right Logical of Doublewords (32-bit integers)
pub const fn mmx_p_srl_d(a: u64, mut count: u32) -> u64 {
    count &= 0x1F;
    let mut mask = (0xFFFFFFFFu32 >> count) as u64;
    mask |= mask << 32;
    (a >> count) & mask
}

/// Packed Unpack Low Doubleword
pub const fn mmx_punpckldq(a: u64, b: u64) -> u64 {
    (a & 0xFFFFFFFF) | (b & 0xFFFFFFFF) << 32
}

/// Packed Unpack Low Doubleword with itself
pub const fn mmx_punpckldq2(a: u64) -> u64 {
    mmx_punpckldq(a, a)
}

#[test]
fn test_mmx() {
    assert_eq!(mmx_punpckldq(0x1, 0x2), 0x200000001);
}
