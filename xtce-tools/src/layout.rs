//! Container flattening and bit-layout resolution.
//!
//! Takes a parsed `SpaceSystem` tree and produces a flat list of
//! `LeafContainer` values, each carrying a fully-resolved list of
//! `FieldLayout` entries with absolute bit offsets.

use std::collections::{HashMap, HashSet};

use xtce_core::model::{
    container::{
        BaseContainer, ComparisonOperator, ReferenceLocation, RestrictionCriteria, SequenceEntry,
    },
    space_system::SpaceSystem,
    telemetry::{ParameterType, TelemetryMetaData},
    types::{FloatSizeInBits, IntegerEncoding, StringSize, ValueEnumeration},
};

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

/// How the raw bits of a field should be interpreted.
#[derive(Debug, Clone)]
pub enum TypeInfo {
    Integer {
        signed: bool,
        size_in_bits: u32,
        /// Reserved for future use (byte-swap support).
        #[allow(dead_code)]
        byte_order_lsb: bool,
    },
    Float {
        size_in_bits: u32,
        /// Reserved for future use (byte-swap support).
        #[allow(dead_code)]
        byte_order_lsb: bool,
    },
    Enum {
        size_in_bits: u32,
        values: Vec<ValueEnumeration>,
    },
    Boolean {
        size_in_bits: u32,
    },
    StringField {
        size_in_bits: u32,
    },
    Binary {
        size_in_bits: u32,
    },
    Unknown {
        size_in_bits: u32,
    },
}

impl TypeInfo {
    pub fn size_in_bits(&self) -> u32 {
        match self {
            TypeInfo::Integer { size_in_bits, .. } => *size_in_bits,
            TypeInfo::Float { size_in_bits, .. } => *size_in_bits,
            TypeInfo::Enum { size_in_bits, .. } => *size_in_bits,
            TypeInfo::Boolean { size_in_bits } => *size_in_bits,
            TypeInfo::StringField { size_in_bits } => *size_in_bits,
            TypeInfo::Binary { size_in_bits } => *size_in_bits,
            TypeInfo::Unknown { size_in_bits } => *size_in_bits,
        }
    }
}

/// One parameter entry in a flattened container, with resolved bit offset.
#[derive(Debug, Clone)]
pub struct FieldLayout {
    /// Parameter name.
    pub name: String,
    pub type_info: TypeInfo,
    /// Absolute bit offset from the start of the container payload.
    pub bit_offset: u32,
}

/// A single equality-based restriction discriminator.
#[derive(Debug, Clone)]
pub struct DiscriminatorInfo {
    /// The parameter name used in the restriction (e.g. "APID").
    pub param_name: String,
    /// The required value (parsed as i64).
    pub value: i64,
}

/// A fully resolved container that is a leaf (no other container extends it).
#[derive(Debug, Clone)]
pub struct LeafContainer {
    pub name: String,
    /// Slash-joined path through the SpaceSystem hierarchy.
    pub full_path: String,
    pub discriminator: Option<DiscriminatorInfo>,
    pub fields: Vec<FieldLayout>,
    /// Total packet payload size in bits (may be 0 if no fields).
    pub total_bits: u32,
}

// ─────────────────────────────────────────────────────────────────────────────
// Flat index
// ─────────────────────────────────────────────────────────────────────────────

/// Flat lookup tables built from a `SpaceSystem` tree.
pub struct SsIndex<'a> {
    /// container_name → (full_path, &SequenceContainer)
    pub containers: HashMap<String, (String, &'a xtce_core::model::container::SequenceContainer)>,
    /// parameter_name → &ParameterType
    pub param_types: HashMap<String, &'a ParameterType>,
    /// parameter_name → parameter_type_ref
    pub param_type_refs: HashMap<String, String>,
}

impl<'a> SsIndex<'a> {
    pub fn build(root: &'a SpaceSystem) -> Self {
        let mut idx = Self {
            containers: HashMap::new(),
            param_types: HashMap::new(),
            param_type_refs: HashMap::new(),
        };
        idx.walk(root, &root.name);
        idx
    }

    fn walk(&mut self, ss: &'a SpaceSystem, path: &str) {
        if let Some(tm) = &ss.telemetry {
            self.index_tm(tm, path);
        }
        for child in &ss.sub_systems {
            let child_path = format!("{}/{}", path, child.name);
            self.walk(child, &child_path);
        }
    }

    fn index_tm(&mut self, tm: &'a TelemetryMetaData, path: &str) {
        for (name, pt) in &tm.parameter_types {
            self.param_types.insert(name.clone(), pt);
        }
        for (name, p) in &tm.parameters {
            self.param_type_refs
                .insert(name.clone(), p.parameter_type_ref.clone());
        }
        for (name, c) in &tm.containers {
            let full = format!("{}/{}", path, name);
            self.containers.insert(name.clone(), (full, c));
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TypeInfo resolution
// ─────────────────────────────────────────────────────────────────────────────

fn resolve_type_info(param_name: &str, idx: &SsIndex<'_>) -> TypeInfo {
    let type_ref = match idx.param_type_refs.get(param_name) {
        Some(r) => r.as_str(),
        None => return TypeInfo::Unknown { size_in_bits: 8 },
    };
    let pt = match idx.param_types.get(type_ref) {
        Some(pt) => pt,
        None => return TypeInfo::Unknown { size_in_bits: 8 },
    };
    match pt {
        ParameterType::Integer(t) => {
            let enc = t.encoding.as_ref();
            let size = enc.map(|e| e.size_in_bits).unwrap_or(8);
            let signed = match enc.map(|e| &e.encoding) {
                Some(IntegerEncoding::TwosComplement)
                | Some(IntegerEncoding::SignMagnitude)
                | Some(IntegerEncoding::OnesComplement) => true,
                _ => t.signed,
            };
            let lsb = enc
                .and_then(|e| e.byte_order.as_ref())
                .map(|o| {
                    matches!(
                        o,
                        xtce_core::model::types::ByteOrder::LeastSignificantByteFirst
                    )
                })
                .unwrap_or(false);
            TypeInfo::Integer {
                signed,
                size_in_bits: size,
                byte_order_lsb: lsb,
            }
        }
        ParameterType::Float(t) => {
            let enc = t.encoding.as_ref();
            let size = enc
                .map(|e| match e.size_in_bits {
                    FloatSizeInBits::F32 => 32,
                    FloatSizeInBits::F64 => 64,
                    FloatSizeInBits::F128 => 128,
                })
                .unwrap_or(32);
            let lsb = enc
                .and_then(|e| e.byte_order.as_ref())
                .map(|o| {
                    matches!(
                        o,
                        xtce_core::model::types::ByteOrder::LeastSignificantByteFirst
                    )
                })
                .unwrap_or(false);
            TypeInfo::Float {
                size_in_bits: size,
                byte_order_lsb: lsb,
            }
        }
        ParameterType::Enumerated(t) => {
            let size = t.encoding.as_ref().map(|e| e.size_in_bits).unwrap_or(8);
            TypeInfo::Enum {
                size_in_bits: size,
                values: t.enumeration_list.clone(),
            }
        }
        ParameterType::Boolean(t) => {
            let size = t.encoding.as_ref().map(|e| e.size_in_bits).unwrap_or(1);
            TypeInfo::Boolean { size_in_bits: size }
        }
        ParameterType::String(t) => {
            let size = t
                .encoding
                .as_ref()
                .and_then(|e| e.size_in_bits.as_ref())
                .map(|s| match s {
                    StringSize::Fixed(b) => *b,
                    StringSize::Variable { max_size_in_bits } => *max_size_in_bits,
                    StringSize::TerminationChar(_) => 64,
                })
                .unwrap_or(64);
            TypeInfo::StringField { size_in_bits: size }
        }
        ParameterType::Binary(t) => {
            let size = t
                .encoding
                .as_ref()
                .map(|e| match &e.size_in_bits {
                    xtce_core::model::types::BinarySize::Fixed(b) => *b,
                    xtce_core::model::types::BinarySize::Variable { .. } => 64,
                })
                .unwrap_or(64);
            TypeInfo::Binary { size_in_bits: size }
        }
        ParameterType::Aggregate(_)
        | ParameterType::Array(_)
        | ParameterType::AbsoluteTime(_)
        | ParameterType::RelativeTime(_) => TypeInfo::Unknown { size_in_bits: 32 },
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Container flattening
// ─────────────────────────────────────────────────────────────────────────────

/// Partial field entry before offsets are assigned.
struct PendingField {
    name: String,
    type_info: TypeInfo,
    location: Option<xtce_core::model::container::EntryLocation>,
}

/// Recursively walks the base chain and collects all entries (base first).
fn collect_entries<'a>(
    container_name: &str,
    idx: &SsIndex<'a>,
    visited: &mut HashSet<String>,
) -> Vec<PendingField> {
    if !visited.insert(container_name.to_string()) {
        // Cycle guard.
        return Vec::new();
    }
    let (_path, container) = match idx.containers.get(container_name) {
        Some(c) => c,
        None => return Vec::new(),
    };

    let mut fields: Vec<PendingField> = Vec::new();

    // First: recurse into base container.
    if let Some(BaseContainer { container_ref, .. }) = &container.base_container {
        fields.extend(collect_entries(container_ref, idx, visited));
    }

    // Then: append this container's own entries.
    for entry in &container.entry_list {
        match entry {
            SequenceEntry::ParameterRef(p) => {
                let ti = resolve_type_info(&p.parameter_ref, idx);
                fields.push(PendingField {
                    name: p.parameter_ref.clone(),
                    type_info: ti,
                    location: p.location.clone(),
                });
            }
            SequenceEntry::FixedValue(f) => {
                // Padding — emit as Unknown so offset tracking still works.
                fields.push(PendingField {
                    name: format!("_pad_{}", f.size_in_bits),
                    type_info: TypeInfo::Unknown {
                        size_in_bits: f.size_in_bits,
                    },
                    location: f.location.clone(),
                });
            }
            SequenceEntry::ContainerRef(c) => {
                // Inline the referenced container's entry list, base-first.
                //
                // If the ContainerRefEntry has an explicit location, push a
                // zero-width anchor field to move the offset cursor before the
                // embedded fields start (it will be filtered from dissector
                // output by the `_pad_` prefix).
                //
                // Known limitation: fields inside the embedded container that
                // carry a ContainerStart location are treated as absolute from
                // the *parent* container's start, not the embedded container's
                // start, because offsets aren't resolved until compute_offsets
                // runs over the whole flat list.
                if let Some(loc) = &c.location {
                    fields.push(PendingField {
                        name: format!("_pad_ref_{}", c.container_ref),
                        type_info: TypeInfo::Unknown { size_in_bits: 0 },
                        location: Some(loc.clone()),
                    });
                }
                // Use a separate visited clone so that ContainerRef cycle
                // detection is independent of the base-container chain path,
                // allowing legitimately reused embedded containers.
                let mut inner_visited = visited.clone();
                fields.extend(collect_entries(&c.container_ref, idx, &mut inner_visited));
            }
            SequenceEntry::ArrayParameterRef(a) => {
                // Array element counts are not stored in the model (they are
                // dynamic), so we cannot compute the field's bit size.
                // Skipping this entry leaves the offset cursor in place, which
                // means any fields that follow will have incorrect offsets.
                eprintln!(
                    "xtce-tools: ArrayParameterRef '{}' skipped — dynamic array size not supported",
                    a.parameter_ref
                );
            }
        }
    }

    fields
}

/// Assign absolute bit offsets to a flat list of pending fields.
fn compute_offsets(pending: Vec<PendingField>) -> Vec<FieldLayout> {
    let mut result = Vec::with_capacity(pending.len());
    let mut cursor: u32 = 0; // tracks end of last field

    for pf in pending {
        let size = pf.type_info.size_in_bits();
        let bit_offset = match &pf.location {
            None => cursor,
            Some(loc) => match loc.reference_location {
                ReferenceLocation::ContainerStart => {
                    // Absolute from container start; offset may be negative in theory but we clamp.
                    loc.bit_offset.max(0) as u32
                }
                ReferenceLocation::PreviousEntry => {
                    // Relative to end of previous entry.
                    (cursor as i64 + loc.bit_offset).max(0) as u32
                }
            },
        };
        cursor = bit_offset + size;
        result.push(FieldLayout {
            name: pf.name,
            type_info: pf.type_info,
            bit_offset,
        });
    }
    result
}

// ─────────────────────────────────────────────────────────────────────────────
// Discriminator extraction
// ─────────────────────────────────────────────────────────────────────────────

fn extract_discriminator(base: &BaseContainer) -> Option<DiscriminatorInfo> {
    let rc = base.restriction_criteria.as_ref()?;
    match rc {
        RestrictionCriteria::Comparison(c) => {
            if c.comparison_operator == ComparisonOperator::Equality {
                let value = c.value.parse::<i64>().ok()?;
                Some(DiscriminatorInfo {
                    param_name: c.parameter_ref.clone(),
                    value,
                })
            } else {
                None
            }
        }
        RestrictionCriteria::ComparisonList(list) => {
            // Take first equality comparison.
            list.iter().find_map(|c| {
                if c.comparison_operator == ComparisonOperator::Equality {
                    let value = c.value.parse::<i64>().ok()?;
                    Some(DiscriminatorInfo {
                        param_name: c.parameter_ref.clone(),
                        value,
                    })
                } else {
                    None
                }
            })
        }
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Build the flat index and return one `LeafContainer` per non-abstract,
/// non-base container in the XTCE document.
pub fn find_leaf_containers(root: &SpaceSystem) -> Vec<LeafContainer> {
    let idx = SsIndex::build(root);

    // Determine which containers are used as a base (non-leaf).
    let mut base_set: HashSet<&str> = HashSet::new();
    for (_, container) in idx.containers.values() {
        if let Some(bc) = &container.base_container {
            base_set.insert(bc.container_ref.as_str());
        }
    }

    let mut leaves = Vec::new();
    for (name, (full_path, container)) in &idx.containers {
        // Skip abstract containers.
        if container.r#abstract {
            continue;
        }
        // Containers that are referenced as a base are not leaves.
        if base_set.contains(name.as_str()) {
            continue;
        }

        let discriminator = container
            .base_container
            .as_ref()
            .and_then(extract_discriminator);

        let mut visited = HashSet::new();
        let pending = collect_entries(name, &idx, &mut visited);
        let fields = compute_offsets(pending);
        let total_bits = fields
            .iter()
            .map(|f| f.bit_offset + f.type_info.size_in_bits())
            .max()
            .unwrap_or(0);

        leaves.push(LeafContainer {
            name: name.clone(),
            full_path: full_path.clone(),
            discriminator,
            fields,
            total_bits,
        });
    }

    // Stable sort by full_path for deterministic output.
    leaves.sort_by(|a, b| a.full_path.cmp(&b.full_path));
    leaves
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use xtce_core::model::{
        container::{
            BaseContainer, ContainerRefEntry, EntryLocation, ParameterRefEntry,
            ReferenceLocation, SequenceContainer, SequenceEntry,
        },
        space_system::SpaceSystem,
        telemetry::{IntegerParameterType, Parameter, ParameterType, TelemetryMetaData},
        types::{IntegerDataEncoding, IntegerEncoding},
    };

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_ss(
        types: Vec<(String, ParameterType)>,
        params: Vec<(String, Parameter)>,
        containers: Vec<(String, SequenceContainer)>,
    ) -> SpaceSystem {
        let mut ss = SpaceSystem::new("Root");
        let mut tm = TelemetryMetaData::default();
        for (name, pt) in types {
            tm.parameter_types.insert(name, pt);
        }
        for (name, p) in params {
            tm.parameters.insert(name, p);
        }
        for (name, c) in containers {
            tm.containers.insert(name, c);
        }
        ss.telemetry = Some(tm);
        ss
    }

    fn uint_type(name: &str, bits: u32) -> (String, ParameterType) {
        let mut t = IntegerParameterType::new(name);
        t.encoding = Some(IntegerDataEncoding {
            size_in_bits: bits,
            encoding: IntegerEncoding::Unsigned,
            byte_order: None,
            default_calibrator: None,
        });
        (name.to_string(), ParameterType::Integer(t))
    }

    fn param(name: &str, type_ref: &str) -> (String, Parameter) {
        (name.to_string(), Parameter::new(name, type_ref))
    }

    fn param_ref(name: &str) -> SequenceEntry {
        SequenceEntry::ParameterRef(ParameterRefEntry {
            parameter_ref: name.to_string(),
            location: None,
            include_condition: None,
        })
    }

    fn container_ref(name: &str) -> SequenceEntry {
        SequenceEntry::ContainerRef(ContainerRefEntry {
            container_ref: name.to_string(),
            location: None,
            include_condition: None,
        })
    }

    fn container_ref_at(name: &str, bit_offset: i64) -> SequenceEntry {
        SequenceEntry::ContainerRef(ContainerRefEntry {
            container_ref: name.to_string(),
            location: Some(EntryLocation {
                reference_location: ReferenceLocation::ContainerStart,
                bit_offset,
            }),
            include_condition: None,
        })
    }

    fn visible_fields(leaf: &LeafContainer) -> Vec<&FieldLayout> {
        leaf.fields.iter().filter(|f| !f.name.starts_with("_pad_")).collect()
    }

    // ── Task 1: base-chain field flattening ───────────────────────────────────

    /// A two-level chain: Base [A(8), B(8)] ← abstract, Child extends Base [C(16)].
    /// Expected flattened fields: A@0, B@8, C@16.
    #[test]
    fn test_base_chain_fields_come_first() {
        let mut base = SequenceContainer::new("Base");
        base.r#abstract = true;
        base.entry_list = vec![param_ref("A"), param_ref("B")];

        let mut child = SequenceContainer::new("Child");
        child.base_container = Some(BaseContainer {
            container_ref: "Base".to_string(),
            restriction_criteria: None,
        });
        child.entry_list = vec![param_ref("C")];

        let ss = make_ss(
            vec![uint_type("U8", 8), uint_type("U16", 16)],
            vec![param("A", "U8"), param("B", "U8"), param("C", "U16")],
            vec![("Base".to_string(), base), ("Child".to_string(), child)],
        );

        let leaves = find_leaf_containers(&ss);
        assert_eq!(leaves.len(), 1);
        let fields = visible_fields(&leaves[0]);

        assert_eq!(leaves[0].name, "Child");
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].name, "A"); assert_eq!(fields[0].bit_offset, 0);
        assert_eq!(fields[1].name, "B"); assert_eq!(fields[1].bit_offset, 8);
        assert_eq!(fields[2].name, "C"); assert_eq!(fields[2].bit_offset, 16);
        assert_eq!(fields[2].type_info.size_in_bits(), 16);
    }

    /// Three levels: GrandBase [X(8)], Base extends GrandBase [Y(8)], Child extends Base [Z(8)].
    /// Expected: X@0, Y@8, Z@16.
    #[test]
    fn test_two_level_base_chain() {
        let mut grand = SequenceContainer::new("GrandBase");
        grand.r#abstract = true;
        grand.entry_list = vec![param_ref("X")];

        let mut base = SequenceContainer::new("Base");
        base.r#abstract = true;
        base.base_container = Some(BaseContainer {
            container_ref: "GrandBase".to_string(),
            restriction_criteria: None,
        });
        base.entry_list = vec![param_ref("Y")];

        let mut child = SequenceContainer::new("Child");
        child.base_container = Some(BaseContainer {
            container_ref: "Base".to_string(),
            restriction_criteria: None,
        });
        child.entry_list = vec![param_ref("Z")];

        let ss = make_ss(
            vec![uint_type("U8", 8)],
            vec![param("X", "U8"), param("Y", "U8"), param("Z", "U8")],
            vec![
                ("GrandBase".to_string(), grand),
                ("Base".to_string(), base),
                ("Child".to_string(), child),
            ],
        );

        let leaves = find_leaf_containers(&ss);
        assert_eq!(leaves.len(), 1);
        let fields = visible_fields(&leaves[0]);
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[0].name, "X"); assert_eq!(fields[0].bit_offset, 0);
        assert_eq!(fields[1].name, "Y"); assert_eq!(fields[1].bit_offset, 8);
        assert_eq!(fields[2].name, "Z"); assert_eq!(fields[2].bit_offset, 16);
    }

    /// Abstract containers must not appear as leaves.
    #[test]
    fn test_abstract_container_not_a_leaf() {
        let mut base = SequenceContainer::new("AbstractBase");
        base.r#abstract = true;
        base.entry_list = vec![param_ref("A")];

        let mut child = SequenceContainer::new("Concrete");
        child.base_container = Some(BaseContainer {
            container_ref: "AbstractBase".to_string(),
            restriction_criteria: None,
        });
        child.entry_list = vec![param_ref("B")];

        let ss = make_ss(
            vec![uint_type("U8", 8)],
            vec![param("A", "U8"), param("B", "U8")],
            vec![
                ("AbstractBase".to_string(), base),
                ("Concrete".to_string(), child),
            ],
        );

        let leaves = find_leaf_containers(&ss);
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].name, "Concrete");
    }

    // ── Task 2: ContainerRef inlining ─────────────────────────────────────────

    /// Packet embeds Header via ContainerRef (not base inheritance).
    /// Expected: H@0 (from Header), P@8 (from Packet).
    #[test]
    fn test_container_ref_inlined() {
        let mut header = SequenceContainer::new("Header");
        header.entry_list = vec![param_ref("H")];

        let mut packet = SequenceContainer::new("Packet");
        packet.entry_list = vec![container_ref("Header"), param_ref("P")];

        let ss = make_ss(
            vec![uint_type("U8", 8)],
            vec![param("H", "U8"), param("P", "U8")],
            vec![
                ("Header".to_string(), header),
                ("Packet".to_string(), packet),
            ],
        );

        let leaves = find_leaf_containers(&ss);
        let packet_leaf = leaves.iter().find(|l| l.name == "Packet")
            .expect("Packet should be a leaf");
        let fields = visible_fields(packet_leaf);

        assert_eq!(fields.len(), 2, "Packet fields: H (inlined from Header) + P");
        assert_eq!(fields[0].name, "H"); assert_eq!(fields[0].bit_offset, 0);
        assert_eq!(fields[1].name, "P"); assert_eq!(fields[1].bit_offset, 8);
    }

    /// ContainerRef with an explicit ContainerStart location moves the cursor
    /// before the embedded fields start.
    /// Header: [H(8)].  Packet: ContainerRef(Header) at offset 8, then P(8).
    /// Expected: H@8, P@16.
    #[test]
    fn test_container_ref_with_location_shifts_cursor() {
        let mut header = SequenceContainer::new("Header");
        header.entry_list = vec![param_ref("H")];

        let mut packet = SequenceContainer::new("Packet");
        packet.entry_list = vec![container_ref_at("Header", 8), param_ref("P")];

        let ss = make_ss(
            vec![uint_type("U8", 8)],
            vec![param("H", "U8"), param("P", "U8")],
            vec![
                ("Header".to_string(), header),
                ("Packet".to_string(), packet),
            ],
        );

        let leaves = find_leaf_containers(&ss);
        let packet_leaf = leaves.iter().find(|l| l.name == "Packet")
            .expect("Packet should be a leaf");
        let fields = visible_fields(packet_leaf);

        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].name, "H"); assert_eq!(fields[0].bit_offset, 8);
        assert_eq!(fields[1].name, "P"); assert_eq!(fields[1].bit_offset, 16);
    }

    // ── Task 5: Integration test against sample.xml ───────────────────────────

    #[test]
    fn test_sample_xml_leaf_names_and_discriminators() {
        let xml_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../test_data/sample.xml");
        let ss = xtce_core::parser::parse_file(&xml_path)
            .expect("sample.xml should parse without errors");

        let leaves = find_leaf_containers(&ss);

        // CCSDSPrimaryHeader is abstract — must not appear.
        let mut names: Vec<&str> = leaves.iter().map(|l| l.name.as_str()).collect();
        names.sort_unstable();
        assert_eq!(names, ["HkPacket", "SciPacket", "SensorPacket"]);

        // Every leaf should have an APID equality discriminator.
        for leaf in &leaves {
            let disc = leaf.discriminator.as_ref()
                .unwrap_or_else(|| panic!("{} should have a discriminator", leaf.name));
            assert_eq!(disc.param_name, "APID",
                "{} discriminator param should be APID", leaf.name);
        }

        let apid = |name: &str| -> i64 {
            leaves.iter().find(|l| l.name == name).unwrap()
                .discriminator.as_ref().unwrap().value
        };
        assert_eq!(apid("HkPacket"),    100);
        assert_eq!(apid("SciPacket"),   200);
        assert_eq!(apid("SensorPacket"), 300);
    }

    #[test]
    fn test_sample_xml_hkpacket_fields() {
        let xml_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../test_data/sample.xml");
        let ss = xtce_core::parser::parse_file(&xml_path)
            .expect("sample.xml should parse without errors");

        let leaves = find_leaf_containers(&ss);
        let hk = leaves.iter().find(|l| l.name == "HkPacket")
            .expect("HkPacket should be a leaf");
        let fields = visible_fields(hk);

        // CCSDS primary header (inherited from CCSDSPrimaryHeader)
        assert_eq!(fields[0].name, "APID");       assert_eq!(fields[0].bit_offset, 0);
        assert_eq!(fields[1].name, "SeqCount");   assert_eq!(fields[1].bit_offset, 16);
        assert_eq!(fields[2].name, "DataLength"); assert_eq!(fields[2].bit_offset, 32);
        // Housekeeping payload
        assert_eq!(fields[3].name, "Mode");        assert_eq!(fields[3].bit_offset, 48);
        assert_eq!(fields[4].name, "Uptime");      assert_eq!(fields[4].bit_offset, 56);
        assert_eq!(fields[5].name, "BattVoltage"); assert_eq!(fields[5].bit_offset, 88);
        assert_eq!(fields[6].name, "CpuLoad");     assert_eq!(fields[6].bit_offset, 120);
        assert_eq!(fields[7].name, "SafeFlag");    assert_eq!(fields[7].bit_offset, 128);
    }

    #[test]
    fn test_sample_xml_sensorpacket_fields() {
        let xml_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../test_data/sample.xml");
        let ss = xtce_core::parser::parse_file(&xml_path)
            .expect("sample.xml should parse without errors");

        let leaves = find_leaf_containers(&ss);
        let sensor = leaves.iter().find(|l| l.name == "SensorPacket")
            .expect("SensorPacket should be a leaf");
        let fields = visible_fields(sensor);

        // CCSDS primary header inherited from parent SpaceSystem scope
        assert_eq!(fields[0].name, "APID");       assert_eq!(fields[0].bit_offset, 0);
        assert_eq!(fields[1].name, "SeqCount");   assert_eq!(fields[1].bit_offset, 16);
        assert_eq!(fields[2].name, "DataLength"); assert_eq!(fields[2].bit_offset, 32);
        // Sensor payload
        assert_eq!(fields[3].name, "Flux");             assert_eq!(fields[3].bit_offset, 48);
        assert_eq!(fields[4].name, "InstrumentStatus"); assert_eq!(fields[4].bit_offset, 80);
    }
}
