//! # gitdesktop-core
//!
//! Núcleo de Git-Desktop Jorgenio: motor Git (libgit2, con puente al CLI planificado para
//! las operaciones de red) y persistencia de la lista de repositorios.
//!
//! Reglas de la casa:
//! - La UI (`gitdesktop-app`) **nunca** depende de `git2`; solo consume los
//!   modelos de [`models`] y el [`engine::GitEngine`].
//! - El acceso a Git se esconde tras [`engine::GitEngine`] para poder
//!   migrar el motor (p. ej. a gitoxide) sin tocar la UI.
//! - La persistencia vive tras [`store::RepoStore`] (TOML en el MVP,
//!   SQLite en fase 2). Ver `PLAN.md` §8.

pub mod engine;
pub mod error;
pub mod models;
pub mod store;

mod native;

pub use engine::{GitEngine, open};
pub use error::{GitoError, Result};
pub use models::{BranchInfo, CommitSummary, DiffLine, DiffLineKind, FileEntry, FileStatus, RepoStatus};
pub use store::{RepoEntry, RepoStore, StoreData, TomlStore, UiState, WindowState};