//! PCAP test-data generator.
//!
//! Produces one synthetic UDP packet per leaf container, wrapped in standard
//! Ethernet II + IPv4 + UDP headers, encoded in libpcap format (little-endian,
//! link type 1 = Ethernet).
//!
//! # Packet structure
//!
//! ```text
//! PCAP global header  (24 bytes)
//! ── for each leaf container ──
//!   PCAP packet record header  (16 bytes)
//!   Ethernet II header         (14 bytes)
//!   IPv4 header                (20 bytes, no options)
//!   UDP header                 (8 bytes)
//!   payload                    (ceil(container.total_bits / 8) bytes)
//! ```
//!
//! # Payload generation
//!
//! Each field in the payload is filled with a deterministic pattern:
//! `(field_index * 3) mod 256`, repeated to fill the field's bit width.
//! This makes the values recognisable without being all-zeros, which aids
//! visual inspection in Wireshark.
//!
//! # Limitations
//!
//! - Source/destination MAC and IP addresses are fixed synthetic values.
//! - No IP checksum is computed (Wireshark accepts this for test files).
//! - Dynamic-size fields (variable-length strings, arrays) are treated as
//!   their maximum declared size.

use crate::layout::{DiscriminatorInfo, FieldLayout, LeafContainer, TypeInfo};

// ─────────────────────────────────────────────────────────────────────────────
// Pseudo-random number generator
// ─────────────────────────────────────────────────────────────────────────────

/// Minimal xorshift64 PRNG — no external dependency, good distribution.
///
/// Seeded per-packet from the wall clock + leaf index so that every call to
/// `generate_pcap` produces unique traffic and different leaves within the
/// same call also differ from one another.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // xorshift64 must not start at zero.
        Self(if seed == 0 { 0x853c_49e6_748f_ea9b } else { seed })
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform random value in `[0, 2^n - 1]`.
    fn next_bits(&mut self, n: u32) -> u64 {
        if n == 0 { return 0; }
        if n >= 64 { return self.next_u64(); }
        self.next_u64() & ((1u64 << n) - 1)
    }

    /// Random f32 in `[-1000.0, +1000.0]` — covers a plausible sensor range.
    fn next_f32(&mut self) -> f32 {
        let frac = self.next_bits(23) as f32 / (1u32 << 23) as f32;
        frac * 2000.0 - 1000.0
    }

    /// Random f64 in `[-1000.0, +1000.0]`.
    fn next_f64(&mut self) -> f64 {
        let frac = self.next_bits(52) as f64 / (1u64 << 52) as f64;
        frac * 2000.0 - 1000.0
    }
}

/// Seed a fresh RNG for the leaf at position `leaf_index`.
///
/// The wall-clock nanosecond timestamp provides entropy across runs; the leaf
/// index is mixed in so that packets generated in the same call also differ.
fn make_rng(leaf_index: usize) -> Rng {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xdead_beef_cafe_babe);
    // Knuth-style multiplicative hash keeps adjacent indices far apart.
    let seed = nanos ^ (leaf_index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    Rng::new(seed)
}

// ─────────────────────────────────────────────────────────────────────────────
// Public entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Build a PCAP file (returned as raw bytes) containing one packet per leaf.
pub fn generate_pcap(leaves: &[LeafContainer], port: u16) -> Vec<u8> {
    let mut out = Vec::with_capacity(64 * 1024);

    write_pcap_global_header(&mut out);

    for (i, lc) in leaves.iter().enumerate() {
        let payload = build_payload(lc, i);
        write_pcap_packet(&mut out, &payload, port, i as u32);
    }

    out
}

// ─────────────────────────────────────────────────────────────────────────────
// PCAP global header  (24 bytes, little-endian)
// ─────────────────────────────────────────────────────────────────────────────

/// Write the 24-byte libpcap global header (magic number, version, link type).
fn write_pcap_global_header(buf: &mut Vec<u8>) {
    buf.extend_from_slice(&0xa1b2c3d4_u32.to_le_bytes()); // magic
    buf.extend_from_slice(&2_u16.to_le_bytes()); // version major
    buf.extend_from_slice(&4_u16.to_le_bytes()); // version minor
    buf.extend_from_slice(&0_i32.to_le_bytes()); // thiszone
    buf.extend_from_slice(&0_u32.to_le_bytes()); // sigfigs
    buf.extend_from_slice(&65535_u32.to_le_bytes()); // snaplen
    buf.extend_from_slice(&1_u32.to_le_bytes()); // network = Ethernet
}

// ─────────────────────────────────────────────────────────────────────────────
// Ethernet + IPv4 + UDP framing
// ─────────────────────────────────────────────────────────────────────────────

/// Wrap `payload` in Ethernet II + IPv4 + UDP headers and append both the
/// pcap per-packet record header and the full frame to `buf`.
/// `seq` is used as a fake timestamp and IP identification field.
fn write_pcap_packet(buf: &mut Vec<u8>, payload: &[u8], dst_port: u16, seq: u32) {
    // Build the full frame bottom-up so we know sizes.
    let udp_len = (8 + payload.len()) as u16;
    let ip_len = (20 + udp_len as usize) as u16;
    let frame_len = 14 + ip_len as usize;

    let mut frame = Vec::with_capacity(frame_len);

    // Ethernet II header (14 bytes)
    frame.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x02]); // dst MAC
    frame.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x01]); // src MAC
    frame.extend_from_slice(&0x0800_u16.to_be_bytes()); // EtherType = IPv4

    // IPv4 header (20 bytes, no options)
    let ip_start = frame.len();
    frame.push(0x45); // version=4, IHL=5
    frame.push(0x00); // DSCP/ECN
    frame.extend_from_slice(&ip_len.to_be_bytes());
    frame.extend_from_slice(&(seq as u16).to_be_bytes()); // identification
    frame.extend_from_slice(&0x0000_u16.to_be_bytes()); // flags/fragment offset
    frame.push(64); // TTL
    frame.push(17); // protocol = UDP
    frame.extend_from_slice(&0x0000_u16.to_be_bytes()); // checksum placeholder
    frame.extend_from_slice(&[192, 168, 1, 1]); // src IP
    frame.extend_from_slice(&[192, 168, 1, 2]); // dst IP

    // Fill in IPv4 checksum.
    let checksum = ipv4_checksum(&frame[ip_start..ip_start + 20]);
    frame[ip_start + 10] = (checksum >> 8) as u8;
    frame[ip_start + 11] = (checksum & 0xff) as u8;

    // UDP header (8 bytes)
    frame.extend_from_slice(&1234_u16.to_be_bytes()); // src port
    frame.extend_from_slice(&dst_port.to_be_bytes()); // dst port
    frame.extend_from_slice(&udp_len.to_be_bytes()); // length
    frame.extend_from_slice(&0x0000_u16.to_be_bytes()); // checksum (optional, zero)

    // Payload
    frame.extend_from_slice(payload);

    // pcap per-packet header (16 bytes)
    let ts_sec = seq; // use seq as fake timestamp for variety
    let ts_usec = 0_u32;
    let incl_len = frame.len() as u32;
    buf.extend_from_slice(&ts_sec.to_le_bytes());
    buf.extend_from_slice(&ts_usec.to_le_bytes());
    buf.extend_from_slice(&incl_len.to_le_bytes());
    buf.extend_from_slice(&incl_len.to_le_bytes()); // orig_len == incl_len

    buf.extend_from_slice(&frame);
}

/// Compute the one's-complement checksum of a 20-byte IPv4 header.
fn ipv4_checksum(header: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    for chunk in header.chunks(2) {
        let word = if chunk.len() == 2 {
            u16::from_be_bytes([chunk[0], chunk[1]]) as u32
        } else {
            (chunk[0] as u32) << 8
        };
        sum += word;
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

// ─────────────────────────────────────────────────────────────────────────────
// Payload synthesis
// ─────────────────────────────────────────────────────────────────────────────

/// Build a randomised payload byte buffer for the container.
///
/// The discriminator field is always written with its exact required value so
/// the dissector's `XTCE_MAP` dispatch table can route the packet.  Every
/// other field receives a random value that is valid for its type and width.
fn build_payload(lc: &LeafContainer, leaf_index: usize) -> Vec<u8> {
    let byte_count = ((lc.total_bits + 7) / 8).max(1) as usize;
    let mut buf = vec![0u8; byte_count];
    let mut rng = make_rng(leaf_index);

    for field in &lc.fields {
        write_field_value(&mut buf, field, lc.discriminator.as_ref(), &mut rng);
    }

    buf
}

/// Write a random valid value for `field` into the byte buffer.
///
/// The discriminator field receives its exact required value so the
/// dissector's dispatch table matches.  For every other field:
///
/// - **Integer** (signed or unsigned): random value across the full bit-width,
///   giving signed fields the full two's-complement range.
/// - **Float32 / Float64**: random finite value in `[-1000, +1000]`.
/// - **Enum**: random pick from the declared enumeration labels.
/// - **Boolean**: random 0 or 1.
/// - **String**: random printable ASCII (letters and digits).
/// - **Binary / Unknown**: random bytes.
fn write_field_value(
    buf: &mut Vec<u8>,
    field: &FieldLayout,
    discriminator: Option<&DiscriminatorInfo>,
    rng: &mut Rng,
) {
    if let Some(disc) = discriminator {
        if field.name == disc.param_name {
            write_bits(buf, field.bit_offset, field.type_info.size_in_bits(), disc.value as u64);
            return;
        }
    }

    match &field.type_info {
        TypeInfo::Integer { size_in_bits, .. } => {
            // Full-range random bits — covers both signed and unsigned correctly
            // since write_bits masks to the field width.
            let val = rng.next_bits(*size_in_bits);
            write_bits(buf, field.bit_offset, *size_in_bits, val);
        }
        TypeInfo::Float { size_in_bits, .. } => {
            let bits: u64 = if *size_in_bits == 64 {
                rng.next_f64().to_bits()
            } else {
                rng.next_f32().to_bits() as u64
            };
            write_bits(buf, field.bit_offset, *size_in_bits, bits);
        }
        TypeInfo::Enum { size_in_bits, values } => {
            let val = if values.is_empty() {
                rng.next_bits(*size_in_bits)
            } else {
                let idx = (rng.next_u64() as usize) % values.len();
                values[idx].value as u64
            };
            write_bits(buf, field.bit_offset, *size_in_bits, val);
        }
        TypeInfo::Boolean { size_in_bits } => {
            write_bits(buf, field.bit_offset, *size_in_bits, rng.next_bits(1));
        }
        TypeInfo::StringField { size_in_bits } => {
            // Random printable ASCII: letters and digits only, easy to read in
            // Wireshark and guaranteed to survive string display filters.
            const CHARS: &[u8] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
            let base = (field.bit_offset / 8) as usize;
            let count = (*size_in_bits / 8) as usize;
            for i in 0..count {
                if base + i < buf.len() {
                    buf[base + i] = CHARS[(rng.next_u64() as usize) % CHARS.len()];
                }
            }
        }
        TypeInfo::Binary { size_in_bits } | TypeInfo::Unknown { size_in_bits } => {
            let base = (field.bit_offset / 8) as usize;
            let count = (*size_in_bits / 8) as usize;
            for i in 0..count {
                if base + i < buf.len() {
                    buf[base + i] = rng.next_bits(8) as u8;
                }
            }
        }
    }
}

/// Write `value` into `buf` starting at `bit_offset` for `bit_count` bits,
/// big-endian bit order.
fn write_bits(buf: &mut [u8], bit_offset: u32, bit_count: u32, value: u64) {
    if bit_count == 0 || bit_count > 64 {
        return;
    }
    // Mask to valid bits.
    let mask = if bit_count == 64 {
        u64::MAX
    } else {
        (1_u64 << bit_count) - 1
    };
    let value = value & mask;

    for bit in 0..bit_count {
        // Source bit: MSB first.
        let src_bit = (bit_count - 1 - bit) as u64;
        let src_val = (value >> src_bit) & 1;

        let dst_bit = bit_offset + bit;
        let byte_idx = (dst_bit / 8) as usize;
        let bit_in_byte = 7 - (dst_bit % 8); // MSB-first within byte
        if byte_idx < buf.len() {
            if src_val == 1 {
                buf[byte_idx] |= 1 << bit_in_byte;
            } else {
                buf[byte_idx] &= !(1 << bit_in_byte);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{DiscriminatorInfo, FieldLayout, LeafContainer, TypeInfo};

    fn make_leaf(
        name: &str,
        discriminator: Option<DiscriminatorInfo>,
        fields: Vec<FieldLayout>,
    ) -> LeafContainer {
        let total_bits = fields
            .iter()
            .map(|f| f.bit_offset + f.type_info.size_in_bits())
            .max()
            .unwrap_or(0);
        LeafContainer {
            name: name.to_string(),
            full_path: format!("Root/{name}"),
            discriminator,
            fields,
            total_bits,
        }
    }

    /// Mimic Wireshark's `TvbRange:bitfield(position, length)`: extract
    /// `count` bits starting at bit `bit_offset` from a big-endian byte
    /// buffer, returning the MSB-first integer value.
    fn extract_bitfield(buf: &[u8], bit_offset: u32, count: u32) -> u64 {
        let mut result: u64 = 0;
        for i in 0..count {
            let src_bit = bit_offset + i;
            let byte_idx = (src_bit / 8) as usize;
            let bit_in_byte = 7 - (src_bit % 8);
            if byte_idx < buf.len() {
                let bit_val = (buf[byte_idx] >> bit_in_byte) & 1;
                result = (result << 1) | (bit_val as u64);
            }
        }
        result
    }

    // ── S4: discriminator value written to payload ────────────────────────────

    /// The discriminator field in the generated payload must equal the
    /// discriminator value so the dissector's XTCE_MAP dispatch table matches.
    #[test]
    fn test_discriminator_value_written_to_payload() {
        // 11-bit APID at bit 0 with discriminator value 104
        let lc = make_leaf(
            "SystemStatusPacket",
            Some(DiscriminatorInfo { param_name: "APID".to_string(), value: 104 }),
            vec![FieldLayout {
                name: "APID".to_string(),
                type_info: TypeInfo::Integer {
                    signed: false,
                    size_in_bits: 11,
                    byte_order_lsb: false,
                },
                bit_offset: 0,
            }],
        );

        let payload = build_payload(&lc, 0);
        let decoded = extract_bitfield(&payload, 0, 11);
        assert_eq!(decoded, 104, "APID field must contain discriminator value 104");
    }

    /// A second discriminator value to confirm the logic with a byte-aligned field.
    #[test]
    fn test_discriminator_value_byte_aligned() {
        let lc = make_leaf(
            "HkPacket",
            Some(DiscriminatorInfo { param_name: "APID".to_string(), value: 200 }),
            vec![FieldLayout {
                name: "APID".to_string(),
                type_info: TypeInfo::Integer {
                    signed: false,
                    size_in_bits: 16,
                    byte_order_lsb: false,
                },
                bit_offset: 0,
            }],
        );

        let payload = build_payload(&lc, 0);
        let decoded = u16::from_be_bytes([payload[0], payload[1]]) as u64;
        assert_eq!(decoded, 200, "16-bit APID must contain discriminator value 200");
    }

    /// A non-discriminator field must receive a value in the valid range for its
    /// type — any value, but not the discriminator value of another field.
    #[test]
    fn test_non_discriminator_field_in_valid_range() {
        let lc = make_leaf(
            "HkPacket",
            Some(DiscriminatorInfo { param_name: "APID".to_string(), value: 100 }),
            vec![
                FieldLayout {
                    name: "APID".to_string(),
                    type_info: TypeInfo::Integer {
                        signed: false,
                        size_in_bits: 16,
                        byte_order_lsb: false,
                    },
                    bit_offset: 0,
                },
                FieldLayout {
                    name: "SeqCount".to_string(),
                    type_info: TypeInfo::Integer {
                        signed: false,
                        size_in_bits: 16,
                        byte_order_lsb: false,
                    },
                    bit_offset: 16,
                },
            ],
        );

        let payload = build_payload(&lc, 0);
        // APID must be the exact discriminator value.
        let apid = u16::from_be_bytes([payload[0], payload[1]]) as u64;
        assert_eq!(apid, 100, "APID must be the discriminator value 100");
        // SeqCount must be a valid 16-bit unsigned integer (any value in range).
        let seq_count = u16::from_be_bytes([payload[2], payload[3]]) as u64;
        assert!(seq_count <= 0xFFFF, "SeqCount must fit in 16 bits, got {seq_count}");
    }

    // ── S5: float encoding produces finite values ─────────────────────────────

    /// A 32-bit float field must decode as a finite f32 (not NaN or infinite).
    #[test]
    fn test_float32_field_is_finite() {
        let lc = make_leaf(
            "Pkt",
            None,
            vec![FieldLayout {
                name: "Val".to_string(),
                type_info: TypeInfo::Float { size_in_bits: 32, byte_order_lsb: false },
                bit_offset: 0,
            }],
        );

        let payload = build_payload(&lc, 0);
        assert_eq!(payload.len(), 4);
        let bits = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
        let val = f32::from_bits(bits);
        assert!(val.is_finite(),
            "32-bit float must be finite, got {val} (bits 0x{bits:08X})");
        assert!(val >= -1000.0 && val <= 1000.0,
            "32-bit float should be in [-1000, 1000] (sensor range), got {val}");
    }

    /// A 64-bit float field must decode as a finite f64 (not NaN or infinite).
    #[test]
    fn test_float64_field_is_finite() {
        let lc = make_leaf(
            "Pkt",
            None,
            vec![FieldLayout {
                name: "Val".to_string(),
                type_info: TypeInfo::Float { size_in_bits: 64, byte_order_lsb: false },
                bit_offset: 0,
            }],
        );

        let payload = build_payload(&lc, 0);
        assert_eq!(payload.len(), 8);
        let bits = u64::from_be_bytes([
            payload[0], payload[1], payload[2], payload[3],
            payload[4], payload[5], payload[6], payload[7],
        ]);
        let val = f64::from_bits(bits);
        assert!(val.is_finite(),
            "64-bit float must be finite, got {val} (bits 0x{bits:016X})");
        assert!(val >= -1000.0 && val <= 1000.0,
            "64-bit float should be in [-1000, 1000] (sensor range), got {val}");
    }

    /// String fields must contain only printable ASCII characters.
    #[test]
    fn test_string_field_is_printable_ascii() {
        let lc = make_leaf(
            "Pkt",
            None,
            vec![FieldLayout {
                name: "Msg".to_string(),
                type_info: TypeInfo::StringField { size_in_bits: 64 }, // 8 bytes
                bit_offset: 0,
            }],
        );

        let payload = build_payload(&lc, 0);
        for &b in &payload {
            assert!(b.is_ascii_alphanumeric(),
                "string bytes must be alphanumeric ASCII, got 0x{b:02X}");
        }
    }

    /// Enum fields must contain one of the declared enumeration values.
    #[test]
    fn test_enum_field_uses_declared_value() {
        use xtce_core::model::types::ValueEnumeration;

        let values = vec![
            ValueEnumeration { value: 0, label: "OFF".to_string(),     max_value: None, short_description: None },
            ValueEnumeration { value: 1, label: "STANDBY".to_string(), max_value: None, short_description: None },
            ValueEnumeration { value: 2, label: "ACTIVE".to_string(),  max_value: None, short_description: None },
        ];
        let lc = make_leaf(
            "Pkt",
            None,
            vec![FieldLayout {
                name: "Mode".to_string(),
                type_info: TypeInfo::Enum { size_in_bits: 8, values: values.clone() },
                bit_offset: 0,
            }],
        );

        // Run several times (different RNG seeds via leaf_index) to check all
        // generated values are from the declared set.
        let valid: Vec<u64> = values.iter().map(|v| v.value as u64).collect();
        for i in 0..20 {
            let payload = build_payload(&lc, i);
            let val = payload[0] as u64;
            assert!(valid.contains(&val),
                "enum value {val} must be one of {valid:?} (leaf_index={i})");
        }
    }
}
