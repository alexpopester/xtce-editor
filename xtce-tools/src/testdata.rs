//! PCAP test-data generator.
//!
//! Produces one synthetic UDP packet per leaf container, wrapped in
//! Ethernet II + IPv4 + UDP headers, encoded in standard libpcap format.

use crate::layout::{FieldLayout, LeafContainer, TypeInfo};

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
/// Each field is filled with a predictable mid-range value.
fn build_payload(lc: &LeafContainer) -> Vec<u8> {
    let byte_count = ((lc.total_bits + 7) / 8).max(1) as usize;
    let mut buf = vec![0u8; byte_count];

    for field in &lc.fields {
        write_field_value(&mut buf, field);
    }

    buf
}

fn write_field_value(buf: &mut Vec<u8>, field: &FieldLayout) {
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
                (1.5_f32.to_bits() as u64) << 32
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
