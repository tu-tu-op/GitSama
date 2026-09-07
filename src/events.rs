use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Commit,
    Push,
    MassivePush,
    Merge,
    BranchSwitch,
    BranchCreate,
    BranchDelete,
    Rebase,
}

impl EventKind {
    pub const ALL: [Self; 8] = [
        Self::Commit,
        Self::Push,
        Self::MassivePush,
        Self::Merge,
        Self::BranchSwitch,
        Self::BranchCreate,
        Self::BranchDelete,
        Self::Rebase,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Commit => "commit",
            Self::Push => "push",
            Self::MassivePush => "massive_push",
            Self::Merge => "merge",
            Self::BranchSwitch => "branch_switch",
            Self::BranchCreate => "branch_create",
            Self::BranchDelete => "branch_delete",
            Self::Rebase => "rebase",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Commit => "Commit",
            Self::Push => "Push",
            Self::MassivePush => "Massive Push",
            Self::Merge => "Merge",
            Self::BranchSwitch => "Branch Switch",
            Self::BranchCreate => "Branch Create",
            Self::BranchDelete => "Branch Delete",
            Self::Rebase => "Rebase",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        let normalized = value.trim().to_ascii_lowercase().replace('-', "_");
        Self::from_str(&normalized)
            .map_err(|_| Error::message(format!("unknown GitSama event '{value}'")))
    }

    pub const fn is_push(self) -> bool {
        matches!(self, Self::Push | Self::MassivePush)
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for EventKind {
    type Err = ();

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "commit" => Ok(Self::Commit),
            "push" => Ok(Self::Push),
            "massive_push" => Ok(Self::MassivePush),
            "merge" => Ok(Self::Merge),
            "branch_switch" => Ok(Self::BranchSwitch),
            "branch_create" => Ok(Self::BranchCreate),
            "branch_delete" => Ok(Self::BranchDelete),
            "rebase" => Ok(Self::Rebase),
            _ => Err(()),
        }
    }
}

