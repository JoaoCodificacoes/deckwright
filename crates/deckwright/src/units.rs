//! Integer English Metric Units. Conversion rounds once, halfway away from zero.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Emu(pub i64);
impl Emu {
    pub fn from_inches(value: f64) -> Result<Self> {
        Self::convert(value, 914400.0)
    }
    pub fn from_points(value: f64) -> Result<Self> {
        Self::convert(value, 12700.0)
    }
    fn convert(value: f64, scale: f64) -> Result<Self> {
        let rounded = (value * scale).round();
        if !rounded.is_finite() || rounded < i64::MIN as f64 || rounded >= i64::MAX as f64 {
            return Err(Error::InvalidPart {
                part: "units".into(),
                message: format!("EMU conversion outside i64 range: {value}"),
            });
        }
        Ok(Self(rounded as i64))
    }
}
