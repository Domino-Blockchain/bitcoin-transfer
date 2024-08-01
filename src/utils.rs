use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use serde::{de::DeserializeOwned, Serialize};

pub mod serde_as_str {
    use std::str::FromStr;

    use serde::{Deserialize, Deserializer, Serializer};

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        let s = String::deserialize(deserializer)?;
        T::from_str(&s).map_err(serde::de::Error::custom)
    }

    pub fn serialize<T, S>(object: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: ToString,
    {
        serializer.serialize_str(&object.to_string())
    }
}

pub fn serde_convert<F, T>(a: F) -> T
where
    F: Serialize,
    T: DeserializeOwned,
{
    let string = serde_json::to_string(&a).unwrap();
    serde_json::from_str(&string).unwrap()
}

#[derive(Clone)]
pub struct ArcPathValueParser;

impl clap::builder::TypedValueParser for ArcPathValueParser {
    type Value = Arc<Path>;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let path = PathBuf::from(value);
        Ok(path.into())
    }
}
