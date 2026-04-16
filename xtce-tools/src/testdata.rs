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
// Public entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Build a PCAP file (returned as raw bytes) containing one packet per leaf.
pub fn generate_pcap(leaves: &[LeafContainer], port: u16) -> Vec<u8> {
    let mut out = Vec::with_capacity(64 * 1024);

    write_pcap_global_header(&mut out);

    for (i, lc) in leaves.iter().enumerate() {
        let payload = build_payload(lc);
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

/// Build a synthetic payload byte buffer for the container.
/// Each field is filled with a predictable mid-range value, except the
/// discriminator field which is filled with the discriminator value so the
/// generated dissector's dispatch table can route the packet.
fn build_payload(lc: &LeafContainer) -> Vec<u8> {
    let byte_count = ((lc.total_bits + 7) / 8).max(1) as usize;
    let mut buf = vec![0u8; byte_count];

    for field in &lc.fields {
        write_field_value(&mut buf, field, lc.discriminator.as_ref());
    }

    buf
}

/// Write a deterministic synthetic value for `field` into the byte buffer.
///
/// If `discriminator` names this field, its value is written verbatim so the
/// generated dissector's `XTCE_MAP` dispatch table can route the packet.
/// Otherwise values are chosen to be recognisable in Wireshark without being
/// all-zeros: signed integers use `1`, unsigned use half of their maximum
/// value, floats use `1.5`, enums use the first declared enumeration value,
/// strings use repeated `'A'`, and binary/unknown fields use `0xAB`.
fn write_field_value(buf: &mut Vec<u8>, field: &FieldLayout, discriminator: Option<&DiscriminatorInfo>) {
    // Discriminator field gets its exact required value so the dissector's
    // dispatch table matches this packet.
    if let Some(disc) = discriminator {
        if field.name == disc.param_name {
            write_bits(buf, field.bit_offset, field.type_info.size_in_bits(), disc.value as u64);
            return;
        }
    }

    match &field.type_info {
        TypeInfo::Integer { signed, size_in_bits, .. } => {
            let val: u64 = if *signed {
                // Use 1 as a small positive integer (avoids sign issues).
                1
            } else {
                // Half of max unsigned value for readability.
                (1_u64 << (*size_in_bits).min(63)) / 2
            };
            write_bits(buf, field.bit_offset, *size_in_bits, val);
        }
        TypeInfo::Float { size_in_bits, .. } => {
            let val: u64 = if *size_in_bits == 64 {
                1.5_f64.to_bits()
            } else {
                // Encode as big-endian IEEE 754 single — no shift needed.
                1.5_f32.to_bits() as u64
            };
            write_bits(buf, field.bit_offset, *size_in_bits, val);
        }
        TypeInfo::Enum { size_in_bits, values } => {
            // Use the first enumeration value if available.
            let val = values.first().map(|v| v.value as u64).unwrap_or(0);
            write_bits(buf, field.bit_offset, *size_in_bits, val);
        }
        TypeInfo::Boolean { size_in_bits } => {
            write_bits(buf, field.bit_offset, *size_in_bits, 1);
        }
        TypeInfo::StringField { size_in_bits } => {
            // Write ASCII 'A' repeated.
            let bytes = (size_in_bits / 8).min(buf.len() as u32);
            for i in 0..bytes {
                let byte_off = (field.bit_offset / 8 + i) as usize;
                if byte_off < buf.len() {
                    buf[byte_off] = b'A';
                }
            }
        }
        TypeInfo::Binary { size_in_bits } | TypeInfo::Unknown { size_in_bits } => {
            // Write 0xAB pattern.
            let bytes = (size_in_bits / 8).min(buf.len() as u32);
            for i in 0..bytes {
                let byte_off = (field.bit_offset / 8 + i) as usize;
                if byte_off < buf.len() {
                    buf[byte_off] = 0xAB;
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

        let payload = build_payload(&lc);
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

        let payload = build_payload(&lc);
        let decoded = u16::from_be_bytes([payload[0], payload[1]]) as u64;
        assert_eq!(decoded, 200, "16-bit APID must contain discriminator value 200");
    }

    /// A non-discriminator field in the same container must keep its generic fill.
    #[test]
    fn test_non_discriminator_field_uses_generic_fill() {
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

        let payload = build_payload(&lc);
        // SeqCount (non-discriminator, 16-bit unsigned) should be half of 2^16 = 32768
        let seq_count = u16::from_be_bytes([payload[2], payload[3]]) as u64;
        assert_eq!(seq_count, (1_u64 << 16) / 2,
            "non-discriminator field must use half-max fill");
    }

    // ── S5: float encoding correctness ────────────────────────────────────────

    /// A 32-bit float field must encode 1.5 as its IEEE 754 bit pattern.
    #[test]
    fn test_float32_field_encodes_correctly() {
        let lc = make_leaf(
            "Pkt",
            None,
            vec![FieldLayout {
                name: "Val".to_string(),
                type_info: TypeInfo::Float { size_in_bits: 32, byte_order_lsb: false },
                bit_offset: 0,
            }],
        );

        let payload = build_payload(&lc);
        assert_eq!(payload.len(), 4);
        let bits = u32::from_be_bytes([payload[0], payload[1], payload[2], payload[3]]);
        assert_eq!(bits, 1.5_f32.to_bits(),
            "32-bit float must encode 1.5 (got 0x{bits:08X})");
    }

    /// A 64-bit float field must encode 1.5 as its IEEE 754 bit pattern.
    #[test]
    fn test_float64_field_encodes_correctly() {
        let lc = make_leaf(
            "Pkt",
            None,
            vec![FieldLayout {
                name: "Val".to_string(),
                type_info: TypeInfo::Float { size_in_bits: 64, byte_order_lsb: false },
                bit_offset: 0,
            }],
        );

        let payload = build_payload(&lc);
        assert_eq!(payload.len(), 8);
        let bits = u64::from_be_bytes([
            payload[0], payload[1], payload[2], payload[3],
            payload[4], payload[5], payload[6], payload[7],
        ]);
        assert_eq!(bits, 1.5_f64.to_bits(),
            "64-bit float must encode 1.5 (got 0x{bits:016X})");
    }
}
