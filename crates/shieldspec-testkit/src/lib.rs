//! Proposal builders and canonical privacy fixtures for ShieldSpec tests.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::Path;

use shieldspec_core::{InputFact, OutputFact, OutputRole, Pool, ProposalSnapshot, StepSnapshot};

const FIXTURE_NAMES: &[&str] = &[
    "shielded-payment",
    "transparent-recipient",
    "transparent-funding",
    "transparent-change",
    "unified-selected-transparent",
    "multi-step-late-leak",
    "privacy-preserving-migration",
    "full-balance-migration",
];

/// Builds normalized proposal snapshots for tests.
#[derive(Clone, Debug)]
pub struct ProposalBuilder {
    schema_version: u32,
    network: String,
    steps: Vec<StepSnapshot>,
}

impl ProposalBuilder {
    /// Starts a schema-version 1 proposal on the canonical test network.
    #[must_use]
    pub fn new() -> Self {
        Self {
            schema_version: 1,
            network: "test".to_owned(),
            steps: Vec::new(),
        }
    }

    /// Overrides the network label.
    #[must_use]
    pub fn network(mut self, network: impl Into<String>) -> Self {
        self.network = network.into();
        self
    }

    /// Appends a proposal step.
    #[must_use]
    pub fn step(mut self, step: impl Into<StepSnapshot>) -> Self {
        self.steps.push(step.into());
        self
    }

    /// Produces the normalized snapshot.
    #[must_use]
    pub fn build(self) -> ProposalSnapshot {
        ProposalSnapshot {
            schema_version: self.schema_version,
            network: self.network,
            steps: self.steps,
        }
    }
}

impl Default for ProposalBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds one normalized proposal step.
#[derive(Clone, Debug)]
pub struct StepBuilder {
    index: usize,
    inputs: Vec<InputFact>,
    outputs: Vec<OutputFact>,
    fee_zat: u64,
}

impl StepBuilder {
    /// Starts an empty step at `index`.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self {
            index,
            inputs: Vec::new(),
            outputs: Vec::new(),
            fee_zat: 0,
        }
    }

    /// Appends an input fact.
    #[must_use]
    pub fn input(mut self, pool: Pool, value_zat: u64) -> Self {
        self.inputs.push(InputFact { pool, value_zat });
        self
    }

    /// Appends an output fact.
    #[must_use]
    pub fn output(mut self, pool: Pool, role: OutputRole, value_zat: u64) -> Self {
        self.outputs.push(OutputFact {
            pool,
            role,
            value_zat,
        });
        self
    }

    /// Sets the step fee.
    #[must_use]
    pub const fn fee(mut self, fee_zat: u64) -> Self {
        self.fee_zat = fee_zat;
        self
    }

    /// Produces the normalized step snapshot.
    #[must_use]
    pub fn build(self) -> StepSnapshot {
        self.into()
    }
}

impl From<StepBuilder> for StepSnapshot {
    fn from(builder: StepBuilder) -> Self {
        Self {
            index: builder.index,
            inputs: builder.inputs,
            outputs: builder.outputs,
            fee_zat: builder.fee_zat,
        }
    }
}

/// Returns canonical fixture names in stable display order.
#[must_use]
pub const fn fixture_names() -> &'static [&'static str] {
    FIXTURE_NAMES
}

/// Loads a canonical fixture by name.
#[must_use]
pub fn fixture(name: &str) -> Option<ProposalSnapshot> {
    let json = match name {
        "shielded-payment" => include_str!("../../../fixtures/shielded-payment.json"),
        "transparent-recipient" => include_str!("../../../fixtures/transparent-recipient.json"),
        "transparent-funding" => include_str!("../../../fixtures/transparent-funding.json"),
        "transparent-change" => include_str!("../../../fixtures/transparent-change.json"),
        "unified-selected-transparent" => {
            include_str!("../../../fixtures/unified-selected-transparent.json")
        }
        "multi-step-late-leak" => include_str!("../../../fixtures/multi-step-late-leak.json"),
        "privacy-preserving-migration" => {
            include_str!("../../../fixtures/privacy-preserving-migration.json")
        }
        "full-balance-migration" => {
            include_str!("../../../fixtures/full-balance-migration.json")
        }
        _ => return None,
    };

    Some(serde_json::from_str(json).expect("embedded canonical fixture must be valid"))
}

/// Writes a canonical fixture as pretty JSON.
pub fn write_fixture(name: &str, path: impl AsRef<Path>) -> Result<(), FixtureWriteError> {
    let snapshot =
        fixture(name).ok_or_else(|| FixtureWriteError::UnknownFixture(name.to_owned()))?;
    let mut json = serde_json::to_string_pretty(&snapshot).map_err(FixtureWriteError::Serialize)?;
    json.push('\n');
    fs::write(path, json).map_err(FixtureWriteError::Io)
}

/// Error returned by [`write_fixture`].
#[derive(Debug)]
pub enum FixtureWriteError {
    /// The requested name is not in [`fixture_names`].
    UnknownFixture(String),
    /// The fixture could not be serialized.
    Serialize(serde_json::Error),
    /// The destination could not be written.
    Io(std::io::Error),
}

impl Display for FixtureWriteError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFixture(name) => write!(formatter, "unknown fixture: {name}"),
            Self::Serialize(error) => write!(formatter, "could not serialize fixture: {error}"),
            Self::Io(error) => write!(formatter, "could not write fixture: {error}"),
        }
    }
}

impl Error for FixtureWriteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::UnknownFixture(_) => None,
            Self::Serialize(error) => Some(error),
            Self::Io(error) => Some(error),
        }
    }
}
