//! Shared primitive types used throughout the XTCE model.

/// A unit of measure (e.g., "meters", "seconds").
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    pub value: String,
    pub power: Option<f64>,
    pub factor: Option<String>,
    pub description: Option<String>,
}

/// A name alias from an external namespace.
#[derive(Debug, Clone, PartialEq)]
pub struct Alias {
    pub name_space: String,
    pub alias: String,
}

/// An integer data encoding describing how bits map to an integer value.
#[derive(Debug, Clone, PartialEq)]
pub struct IntegerDataEncoding {
    pub size_in_bits: u32,
    pub encoding: IntegerEncoding,
    pub byte_order: Option<ByteOrder>,
    pub default_calibrator: Option<Calibrator>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum IntegerEncoding {
    #[default]
    Unsigned,
    SignMagnitude,
    TwosComplement,
    OnesComplement,
    BCD,
    PackedBCD,
}

/// A float data encoding.
#[derive(Debug, Clone, PartialEq)]
pub struct FloatDataEncoding {
    pub size_in_bits: FloatSizeInBits,
    pub encoding: FloatEncoding,
    pub byte_order: Option<ByteOrder>,
    pub default_calibrator: Option<Calibrator>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FloatSizeInBits {
    F32,
    F64,
    F128,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum FloatEncoding {
    #[default]
    IEEE754_1985,
    /// MIL-STD-1750A floating-point format.
    MilStd1750A,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StringDataEncoding {
    pub encoding: StringEncoding,
    pub byte_order: Option<ByteOrder>,
    pub size_in_bits: Option<StringSize>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum StringEncoding {
    #[default]
    UTF8,
    UTF16,
    UsAscii,
    Iso8859_1,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StringSize {
    Fixed(u32),
    TerminationChar(u8),
    Variable { max_size_in_bits: u32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct BinaryDataEncoding {
    pub size_in_bits: BinarySize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinarySize {
    Fixed(u32),
    Variable { size_reference: String },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum ByteOrder {
    #[default]
    MostSignificantByteFirst,
    LeastSignificantByteFirst,
}

/// A polynomial or spline calibrator applied during decoding.
#[derive(Debug, Clone, PartialEq)]
pub enum Calibrator {
    Polynomial(PolynomialCalibrator),
    SplineCalibrator(SplineCalibrator),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PolynomialCalibrator {
    /// Coefficients in ascending order of power (index 0 = constant term).
    pub coefficients: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SplineCalibrator {
    pub order: u32,
    pub extrapolate: bool,
    pub points: Vec<SplinePoint>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SplinePoint {
    pub raw: f64,
    pub calibrated: f64,
}

/// A named value→label mapping entry in an enumerated type.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueEnumeration {
    pub value: i64,
    pub label: String,
    pub max_value: Option<i64>,
    pub short_description: Option<String>,
}
