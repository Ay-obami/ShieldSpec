use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Pool {
    Transparent,
    Sapling,
    Orchard,
    Ironwood,
    Unknown,
}

impl Pool {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Transparent => "transparent",
            Self::Sapling => "sapling",
            Self::Orchard => "orchard",
            Self::Ironwood => "ironwood",
            Self::Unknown => "unknown",
        }
    }

    pub(crate) const fn is_shielded(self) -> bool {
        matches!(self, Self::Sapling | Self::Orchard | Self::Ironwood)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputRole {
    Recipient,
    Change,
    Other,
}

impl OutputRole {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Recipient => "recipient",
            Self::Change => "change",
            Self::Other => "other",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InputFact {
    pub pool: Pool,
    pub value_zat: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OutputFact {
    pub pool: Pool,
    pub role: OutputRole,
    pub value_zat: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StepSnapshot {
    pub index: usize,
    pub inputs: Vec<InputFact>,
    pub outputs: Vec<OutputFact>,
    pub fee_zat: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProposalSnapshot {
    pub schema_version: u32,
    pub network: String,
    pub steps: Vec<StepSnapshot>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Finding {
    pub rule_id: String,
    pub severity: Severity,
    pub step_index: usize,
    pub message: String,
    pub evidence: BTreeMap<String, String>,
    pub fingerprint: String,
}
