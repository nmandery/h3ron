use std::cmp::Eq;
use std::fmt::Debug;

use ordered_float::OrderedFloat;
use serde::{Serialize, Deserialize};

#[derive(Debug, PartialEq, Hash, Clone, Serialize, Deserialize)]
pub enum Value {
    Int16(i16),
    Uint8(u8),
    Uint16(u16),
    Int32(i32),
    Uint32(u32),
    Float32(OrderedFloat<f32>),
    Float64(OrderedFloat<f64>),
}

impl Eq for Value {}

pub trait ToValue {
    fn to_value(&self) -> Value where Self: Sized;
}

impl ToValue for u8 {
    fn to_value(&self) -> Value { Value::Uint8(*self) }
}

impl ToValue for u16 {
    fn to_value(&self) -> Value { Value::Uint16(*self) }
}

impl ToValue for u32 {
    fn to_value(&self) -> Value { Value::Uint32(*self) }
}

impl ToValue for i16 {
    fn to_value(&self) -> Value { Value::Int16(*self) }
}

impl ToValue for i32 {
    fn to_value(&self) -> Value { Value::Int32(*self) }
}

impl ToValue for f32 {
    fn to_value(&self) -> Value { Value::Float32(self.clone().into()) }
}

impl ToValue for f64 {
    fn to_value(&self) -> Value { Value::Float64(self.clone().into()) }
}

pub trait ToOption<T> {
    fn to_option(&self) -> Option<T> where Self: Sized;
}

impl ToOption<u8> for Value {
    fn to_option(&self) -> Option<u8> where Self: Sized {
        match self {
            Self::Uint8(v) => Some(*v),
            _ => None
        }
    }
}

impl ToOption<u16> for Value {
    fn to_option(&self) -> Option<u16> where Self: Sized {
        match self {
            Self::Uint16(v) => Some(*v),
            _ => None
        }
    }
}

impl ToOption<u32> for Value {
    fn to_option(&self) -> Option<u32> where Self: Sized {
        match self {
            Self::Uint32(v) => Some(*v),
            _ => None
        }
    }
}

impl ToOption<i16> for Value {
    fn to_option(&self) -> Option<i16> where Self: Sized {
        match self {
            Self::Int16(v) => Some(*v),
            _ => None
        }
    }
}

impl ToOption<i32> for Value {
    fn to_option(&self) -> Option<i32> where Self: Sized {
        match self {
            Self::Int32(v) => Some(*v),
            _ => None
        }
    }
}

impl ToOption<f32> for Value {
    fn to_option(&self) -> Option<f32> where Self: Sized {
        match self {
            Self::Float32(v) => Some(v.into_inner()),
            _ => None
        }
    }
}

impl ToOption<f64> for Value {
    fn to_option(&self) -> Option<f64> where Self: Sized {
        match self {
            Self::Float64(v) => Some(v.into_inner()),
            _ => None
        }
    }
}


pub trait Classifier {
    fn classify(&self, value: Value) -> Option<Value>;
    fn value_type(&self) -> &Value;
}

pub struct NoData {
    pub no_data_value: Value,
}

impl NoData {
    pub fn new(no_data_value: Value) -> Self {
        Self { no_data_value }
    }
}

impl Classifier for NoData {
    fn classify(&self, other: Value) -> Option<Value> {
        if self.no_data_value == other {
            None
        } else {
            Some(other)
        }
    }

    fn value_type(&self) -> &Value {
        &self.no_data_value
    }
}

pub struct ClassifiedBand {
    pub source_band: u8,
    pub classifier: Box<dyn Classifier>,
}

#[cfg(test)]
mod tests {
    use crate::input::{ToOption, ToValue, Value};

    #[test]
    fn test_value_conversion() {
        let i = 5u8;
        let v = i.to_value();
        assert_eq!(Value::Uint8(i), v);
        let i2 = v.to_option();
        assert!(i2.is_some());
        assert_eq!(Some(i), i2);
    }
}
