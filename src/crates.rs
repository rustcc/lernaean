use crate::{GenResult, GLOBAL_CONFIG};
use http::Uri;
use serde::Deserialize;

const CRATE_TEMPLATE: &str = "{crate}";
const VERSION_TEMPLATE: &str = "{version}";

pub fn init() -> GenResult<()> {
    check_upstream_url()?;

    Ok(())
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct CrateIdentity {
    pub name: String,
    pub version: String,
}

impl std::fmt::Display for CrateIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}-{}", self.name, self.version)
    }
}

fn check_upstream_url() -> GenResult<()> {
    let fmt: &str = &GLOBAL_CONFIG.upstream_dl;

    if !fmt.contains(CRATE_TEMPLATE) {
        return Err(format_err!("no {{crate}} in upstream_dl: {}", fmt));
    }
    if !fmt.contains(VERSION_TEMPLATE) {
        return Err(format_err!("no {{version}} in upstream_dl: {}", fmt));
    }

    if fmt
        .replace(CRATE_TEMPLATE, "foo")
        .replace(VERSION_TEMPLATE, "0.1.0")
        .parse::<Uri>()
        .is_err()
    {
        return Err(format_err!("'upstream_dl' isn't valid uri"));
    }

    Ok(())
}

pub fn upstream_url(name: &str, version: &str) -> String {
    let fmt: &str = &GLOBAL_CONFIG.upstream_dl;
    fmt.replace(CRATE_TEMPLATE, name)
        .replace(VERSION_TEMPLATE, version)
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize)]
pub struct CrateMetadata {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "vers")]
    pub version: String,
    #[serde(rename = "cksum", with = "hex_serde")]
    pub checksum: [u8; 32],
}

impl std::fmt::Display for CrateMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}-{}[{}]",
            self.name,
            self.version,
            hex::encode(self.checksum)
        )
    }
}
