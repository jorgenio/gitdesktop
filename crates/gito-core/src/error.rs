//! Errores unificados del núcleo de Gito.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Error de alto nivel que la UI puede mostrar tal cual (`Display`).
#[derive(Debug, Error)]
pub enum GitoError {
    #[error("no es un repositorio git: {0}")]
    NotARepository(PathBuf),

    #[error("error de git: {0}")]
    Git(#[from] git2::Error),

    #[error("no se pudo determinar el directorio de configuración del usuario")]
    ConfigDir,

    #[error("error de E/S en {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("error al leer la configuración: {0}")]
    StoreFormat(#[from] toml::de::Error),

    #[error("error al escribir la configuración: {0}")]
    StoreWrite(#[from] toml::ser::Error),
}

impl GitoError {
    /// Atajo para construir [`GitoError::Io`] con la ruta implicada.
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io { path: path.into(), source }
    }
}

/// `Result` por defecto del núcleo.
pub type Result<T> = std::result::Result<T, GitoError>;

/// Envuelve un `std::io::Error` con la ruta en la que ocurrió.
pub(crate) fn io_err(path: &Path, err: std::io::Error) -> GitoError {
    GitoError::io(path.to_path_buf(), err)
}
