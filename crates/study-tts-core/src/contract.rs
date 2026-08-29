//! Provisional interface versions and the amendment rules that govern them.
//!
//! These types mechanize the E0-S4 change classes in
//! `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`; that document
//! names this module in return so policy and enforcement remain discoverable.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// One provisional contract governed by the E0-S4 baseline.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractId {
    /// The object-safe asynchronous synthesis executor.
    TtsExecutor,
    /// The NDJSON worker request and response frames.
    WorkerFrames,
    /// Validated synthesis-cache lookup and publication.
    CachePublication,
    /// Master-first package creation and immutable selection.
    PackageWriter,
    /// Minimal durable job ownership and selected-package state.
    JobState,
}

/// A three-part semantic version for a provisional contract.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ContractVersion {
    /// Major version, incremented for a breaking change.
    pub major: u16,
    /// Minor version, incremented for a compatible extension.
    pub minor: u16,
    /// Reserved patch component; E0 diagnostic-only edits retain the version.
    pub patch: u16,
}

/// The declared meaning of a contract descriptor relative to its predecessor.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractChange {
    /// The initial descriptor for a contract.
    Baseline,
    /// Diagnostics changed without changing durable bytes or behavior.
    DiagnosticPatch,
    /// An optional field or capability was added with an explicit default.
    CompatibleExtension,
    /// A required field, semantic rule, or frame shape changed.
    Breaking,
    /// An authority or architecture boundary change that requires an accepted
    /// ADR.
    Architectural,
}

/// A checked-in descriptor for one provisional public seam.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ContractDescriptor {
    /// Contract this descriptor versions.
    pub contract_id: ContractId,
    /// Semantic contract version.
    pub version: ContractVersion,
    /// Compatibility claim relative to the prior descriptor.
    pub change: ContractChange,
    /// Declared default for an optional compatible extension.
    pub extension_default: Option<String>,
    /// Whether fields outside the declared representation are rejected.
    pub rejects_unknown_fields: bool,
}

impl ContractDescriptor {
    /// Checks `successor` against its declared change class.
    ///
    /// # Errors
    ///
    /// [`ContractVersionError::ContractChanged`] when the descriptors name
    /// different seams, [`ContractVersionError::MissingExtensionDefault`]
    /// when an extension omits its default,
    /// [`ContractVersionError::UnknownFieldsMustBeRejected`] when a project
    /// format becomes lenient, or
    /// [`ContractVersionError::VersionClassMismatch`] when the declared change
    /// and semantic increment disagree.
    pub fn assess_successor(
        &self,
        successor: &Self,
    ) -> Result<SuccessorCompatibility, ContractVersionError> {
        if self.contract_id != successor.contract_id {
            return Err(ContractVersionError::ContractChanged {
                previous: self.contract_id,
                successor: successor.contract_id,
            });
        }
        if !successor.rejects_unknown_fields {
            return Err(ContractVersionError::UnknownFieldsMustBeRejected);
        }
        if successor.change == ContractChange::CompatibleExtension
            && successor.extension_default.is_none()
        {
            return Err(ContractVersionError::MissingExtensionDefault);
        }
        if successor == self {
            return Ok(SuccessorCompatibility::Unchanged);
        }

        let represented_semantics_unchanged = successor.extension_default == self.extension_default
            && successor.rejects_unknown_fields == self.rejects_unknown_fields;
        let compatibility = match successor.change {
            ContractChange::Baseline => None,
            ContractChange::DiagnosticPatch => (successor.version == self.version
                && represented_semantics_unchanged)
                .then_some(SuccessorCompatibility::DiagnosticPatch),
            ContractChange::CompatibleExtension => (successor.version.major == self.version.major
                && successor.version.minor > self.version.minor
                && successor.version.patch == 0)
                .then_some(SuccessorCompatibility::CompatibleExtension),
            ContractChange::Breaking | ContractChange::Architectural => (successor.version.major
                > self.version.major
                && successor.version.minor == 0
                && successor.version.patch == 0)
                .then_some(SuccessorCompatibility::Breaking),
        };

        compatibility.ok_or(ContractVersionError::VersionClassMismatch {
            previous: self.version,
            successor: successor.version,
            change: successor.change,
        })
    }
}

/// Compatibility established for a valid successor descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuccessorCompatibility {
    /// The successor descriptor is identical to its predecessor.
    Unchanged,
    /// Only diagnostics changed while the represented contract stayed fixed.
    DiagnosticPatch,
    /// An optional extension changed under a minor increment.
    CompatibleExtension,
    /// A major increment explicitly breaks compatibility.
    Breaking,
}

/// Why a provisional contract successor is not valid.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ContractVersionError {
    /// A descriptor attempted to become a different contract.
    #[error(
        "contract changed from {previous:?} to {successor:?}; the engineering owner must amend \
         each contract separately"
    )]
    ContractChanged {
        /// Contract governed by the previous descriptor.
        previous: ContractId,
        /// Contract named by the successor.
        successor: ContractId,
    },
    /// A compatible extension did not state its default.
    #[error(
        "a compatible extension omits the default used by older consumers; the engineering owner \
         must declare it in the contract descriptor"
    )]
    MissingExtensionDefault,
    /// A project-owned boundary attempted to ignore unknown fields.
    #[error(
        "the contract descriptor permits unknown fields in a project-owned format; the \
         engineering owner must restore strict deserialization"
    )]
    UnknownFieldsMustBeRejected,
    /// The semantic increment does not match the declared change class.
    #[error(
        "contract version {previous:?} cannot become {successor:?} as {change:?}; the engineering \
         owner must use the change-control version assigned to that class"
    )]
    VersionClassMismatch {
        /// Version being amended.
        previous: ContractVersion,
        /// Proposed successor version.
        successor: ContractVersion,
        /// Compatibility class the successor claims.
        change: ContractChange,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t1_e0_a_compatible_extension_always_declares_its_default() {
        let extension = ContractDescriptor {
            contract_id: ContractId::WorkerFrames,
            version: ContractVersion {
                major: 0,
                minor: 1,
                patch: 0,
            },
            change: ContractChange::CompatibleExtension,
            extension_default: None,
            rejects_unknown_fields: true,
        };

        assert_eq!(
            extension.assess_successor(&extension),
            Err(ContractVersionError::MissingExtensionDefault)
        );
    }

    #[test]
    fn t1_e0_contract_refusals_name_the_engineering_owner() {
        let version = ContractVersion {
            major: 0,
            minor: 1,
            patch: 0,
        };
        let errors = [
            ContractVersionError::ContractChanged {
                previous: ContractId::WorkerFrames,
                successor: ContractId::CachePublication,
            },
            ContractVersionError::MissingExtensionDefault,
            ContractVersionError::UnknownFieldsMustBeRejected,
            ContractVersionError::VersionClassMismatch {
                previous: version,
                successor: version,
                change: ContractChange::Breaking,
            },
        ];

        for error in errors {
            assert!(
                error.to_string().contains("engineering owner"),
                "`{error}` omits its remedy owner"
            );
        }
    }
}
