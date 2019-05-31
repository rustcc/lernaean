use serde::Deserialize;

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

fn upstream_url(name: &str, version: &str) -> String {
    // TODO read from 'upstream' argument
    format!(
        "https://static.crates.io/crates/{name}/{name}-{version}.crate",
        name = name,
        version = version
    )
}

impl CrateIdentity {
    pub fn upstream_url(&self) -> String {
        upstream_url(&self.name, &self.version)
    }
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

impl CrateMetadata {
    pub fn upstream_url(&self) -> String {
        upstream_url(&self.name, &self.version)
    }
}
