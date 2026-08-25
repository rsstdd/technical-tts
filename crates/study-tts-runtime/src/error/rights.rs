//! Publication refusals owned by source and voice rights declarations.

use thiserror::Error;

use super::{RemedyAdvice, RemedyOwner};

/// Why rights declarations refused production publication.
#[derive(Debug, Error)]
pub enum RightsError {
    /// A source reached publication without a resolved rights classification.
    #[error(
        "production release is refused: source `{source_id}` has unresolved rights \
         classification `{classification}`; the project owner must resolve the classification in \
         its rights record before publication"
    )]
    UnresolvedContentRights {
        /// The source whose classification is unresolved.
        source_id: String,
        /// The classification it currently carries.
        classification: String,
    },

    /// A manifest rights section is not a valid declaration.
    #[error(
        "production release is refused: the `{section}` manifest section is not a valid rights \
         declaration ({source}); the project owner must correct the manifest before publication"
    )]
    InvalidRightsDeclaration {
        /// Which manifest section failed to parse.
        section: &'static str,
        /// What the parser reported.
        source: serde_json::Error,
    },

    /// The manifest names no classified source.
    #[error(
        "production release is refused: the manifest declares no `content_rights` \
         classification for its sources; the project owner must classify every source in its \
         rights record before publication"
    )]
    MissingContentRightsDeclaration,

    /// The manifest names no voice profile.
    #[error(
        "production release is refused: the manifest declares no `voice_profiles` for the \
         voices it was rendered with; the project owner must declare each voice profile and \
         its rights record before publication"
    )]
    MissingVoiceProfileDeclaration,

    /// A declaration names an identifier but leaves it blank.
    #[error(
        "production release is refused: the `{section}` manifest section declares an empty \
         `{field}`; the project owner must name it before publication"
    )]
    EmptyManifestIdentifier {
        /// The manifest section holding the declaration.
        section: &'static str,
        /// The identifier field left blank.
        field: &'static str,
    },
}

impl RightsError {
    /// Returns the governed recovery advice for this rights refusal.
    pub(super) fn remedy(&self) -> Option<RemedyAdvice> {
        match self {
            Self::UnresolvedContentRights { .. }
            | Self::InvalidRightsDeclaration { .. }
            | Self::MissingContentRightsDeclaration
            | Self::MissingVoiceProfileDeclaration
            | Self::EmptyManifestIdentifier { .. } => Some(RemedyAdvice::new(
                RemedyOwner::ProjectOwner,
                "correct or resolve the rights declaration before publication",
                Some("Missing rights classification"),
            )),
        }
    }
}
