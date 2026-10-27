use crate::Severity;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Profile {
    Standard,
    Strict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Policy {
    profile: Profile,
}

impl Policy {
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            profile: Profile::Standard,
        }
    }

    #[must_use]
    pub const fn strict() -> Self {
        Self {
            profile: Profile::Strict,
        }
    }

    pub(crate) const fn severity(self, standard: Severity) -> Severity {
        match self.profile {
            Profile::Standard => standard,
            Profile::Strict => Severity::Error,
        }
    }
}
