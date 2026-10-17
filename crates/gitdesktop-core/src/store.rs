//! Persistencia de la lista de repositorios y del estado de la UI.
//!
//! MVP (ver `PLAN.md` §8): un único archivo `gitdesktop.toml` con escritura
//! atómica y copia de seguridad. Todo vive tras [`RepoStore`] para poder
//! migrar a SQLite en fase 2 sin tocar la UI.
//!
//! Rutas estándar por plataforma (vía `dirs`):
//! - Linux:  `~/.config/gitdesktop/gitdesktop.toml`
//! - Windows: `%APPDATA%\gitdesktop\gitdesktop.toml`

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::error::{GitoError, Result, io_err};

/// Almacén persistente de repositorios.
pub trait RepoStore: Send + Sync {
    /// Carga los datos. Devuelve [`StoreData::default`] si el archivo aún
    /// no existe o está irrecuperable (nunca bloquea el arranque de la app).
    fn load(&self) -> Result<StoreData>;

    /// Guarda los datos de forma atómica (tmp + rename) con backup.
    fn save(&self, data: &StoreData) -> Result<()>;

    /// Ruta del archivo físico (informativo, para mostrarla en la UI).
    fn path(&self) -> &Path;
}

/// Datos completos que Git-Desktop Jorgenio persiste en disco.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StoreData {
    /// Estado global de la interfaz.
    #[serde(default)]
    pub ui: UiState,

    /// Lista de repositorios conocidos (se serializa como `[[repo]]`).
    #[serde(default, rename = "repo")]
    pub repos: Vec<RepoEntry>,
}

impl StoreData {
    /// Registra (o actualiza) un repositorio por su ruta y devuelve la
    /// entrada resultante. Actualiza `last_opened_at`.
    pub fn touch_repo(&mut self, path: &Path, now: OffsetDateTime) -> RepoEntry {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

        if let Some(entry) = self
            .repos
            .iter_mut()
            .find(|r| r.path == canonical)
        {
            entry.last_opened_at = now;
            entry.clone()
        } else {
            let entry = RepoEntry::new(&canonical, now);
            self.repos.push(entry.clone());
            entry
        }
    }

    /// Ordena: fijados primero, luego por último uso (más reciente arriba).
    pub fn sort_recent(&mut self) {
        self.repos.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| b.last_opened_at.cmp(&a.last_opened_at))
        });
    }

    /// Busca un repositorio por su `id`.
    pub fn find_by_id(&self, id: &str) -> Option<&RepoEntry> {
        self.repos.iter().find(|r| r.id == id)
    }
}

/// Estado global de la interfaz.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiState {
    /// Tema de la app (`"dark"` en el MVP; fase 2: claro/seguir sistema).
    #[serde(default = "default_theme")]
    pub theme: String,

    /// `id` del último repositorio abierto (se reabre al arrancar).
    #[serde(default)]
    pub last_repo: Option<String>,

    /// Tamaño y estado de la ventana.
    #[serde(default)]
    pub window: WindowState,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            last_repo: None,
            window: WindowState::default(),
        }
    }
}

fn default_theme() -> String {
    "dark".to_owned()
}

/// Tamaño y estado de la ventana principal.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    #[serde(default = "default_width")]
    pub width: f32,
    #[serde(default = "default_height")]
    pub height: f32,
    #[serde(default)]
    pub maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: default_width(),
            height: default_height(),
            maximized: false,
        }
    }
}

fn default_width() -> f32 {
    1280.0
}

fn default_height() -> f32 {
    800.0
}

/// Un repositorio conocido por la app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepoEntry {
    /// Identificador estable derivado de la ruta.
    pub id: String,
    /// Ruta canónica del repositorio.
    pub path: PathBuf,
    /// Nombre corto (última componente de la ruta).
    pub name: String,

    #[serde(with = "time::serde::rfc3339")]
    pub added_at: OffsetDateTime,

    #[serde(with = "time::serde::rfc3339")]
    pub last_opened_at: OffsetDateTime,

    /// Fijado en la parte superior de la lista.
    #[serde(default)]
    pub pinned: bool,

    /// Última rama activa (para restaurar la vista).
    #[serde(default)]
    pub last_branch: Option<String>,

    /// Última pestaña abierta (`"changes"` | `"history"`).
    #[serde(default)]
    pub last_tab: Option<String>,
}

impl RepoEntry {
    /// Crea una entrada para la ruta dada (la canonicaliza si es posible).
    pub fn new(path: &Path, now: OffsetDateTime) -> Self {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());

        Self {
            id: id_for(&path),
            path,
            name,
            added_at: now,
            last_opened_at: now,
            pinned: false,
            last_branch: None,
            last_tab: None,
        }
    }
}

/// Identificador estable a partir de la ruta (hash determinista).
///
/// Los duplicados por ruta son imposibles porque [`StoreData::touch_repo`]
/// compara rutas canónicas; el `id` es solo una clave cómoda para la UI.
pub fn id_for(path: &Path) -> String {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// Implementación TOML de [`RepoStore`] (MVP).
#[derive(Debug, Clone)]
pub struct TomlStore {
    path: PathBuf,
}

impl TomlStore {
    /// Ruta por defecto según la plataforma.
    pub fn default_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("git-desktop-jorgenio").join("gitdesktop.toml"))
    }

    /// Abre el almacén en la ruta por defecto.
    pub fn open_default() -> Result<Self> {
        let path = Self::default_path().ok_or(GitoError::ConfigDir)?;
        Ok(Self::new(path))
    }

    /// Abre el almacén en una ruta arbitraria (útil para tests).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn backup_path(&self) -> PathBuf {
        PathBuf::from(format!("{}.bak", self.path.display()))
    }
}

impl RepoStore for TomlStore {
    fn load(&self) -> Result<StoreData> {
        let raw = match fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            // Primera ejecución: todavía no hay configuración.
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(StoreData::default());
            }
            Err(err) => return Err(io_err(&self.path, err)),
        };

        match toml::from_str::<StoreData>(&raw) {
            Ok(data) => Ok(data),
            Err(_primary) => {
                // El archivo está corrupto: intenta el backup de la última
                // versión buena. Si tampoco funciona, se arranca limpio
                // (la próxima llamada a `save` lo regenera).
                if let Ok(backup) = fs::read_to_string(self.backup_path()) {
                    if let Ok(data) = toml::from_str::<StoreData>(&backup) {
                        return Ok(data);
                    }
                }
                Ok(StoreData::default())
            }
        }
    }

    fn save(&self, data: &StoreData) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
        }

        let raw = toml::to_string_pretty(data)?;

        // 1) Escribe a un archivo temporal…
        let tmp = PathBuf::from(format!("{}.tmp", self.path.display()));
        fs::write(&tmp, raw.as_bytes()).map_err(|e| io_err(&tmp, e))?;

        // 2) …copia la versión previa como backup (best effort)…
        if self.path.exists() {
            let _ = fs::copy(&self.path, self.backup_path());
        }

        // 3) …y reemplaza atómicamente.
        fs::rename(&tmp, &self.path).map_err(|e| io_err(&self.path, e))?;

        Ok(())
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(now: OffsetDateTime) -> StoreData {
        let mut data = StoreData::default();
        data.ui.last_repo = Some("abc".into());
        data.touch_repo(Path::new("/tmp/proyecto-a"), now);
        data.touch_repo(Path::new("/tmp/proyecto-b"), now);
        data.repos[0].pinned = true;
        data.repos[0].last_branch = Some("main".into());
        data
    }

    #[test]
    fn roundtrip_preserves_data() {
        let dir = tempfile::tempdir().unwrap();
        let store = TomlStore::new(dir.path().join("gitdesktop.toml"));
        let now = OffsetDateTime::now_utc();

        let data = sample(now);
        store.save(&data).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded, data);
        assert_eq!(loaded.repos.len(), 2);
        assert_eq!(loaded.repos[0].last_branch.as_deref(), Some("main"));
    }

    #[test]
    fn missing_file_loads_default() {
        let dir = tempfile::tempdir().unwrap();
        let store = TomlStore::new(dir.path().join("gitdesktop.toml"));

        let loaded = store.load().unwrap();
        assert!(loaded.repos.is_empty());
        assert_eq!(loaded.ui.theme, "dark");
    }

    #[test]
    fn corrupt_file_falls_back_to_backup() {
        let dir = tempfile::tempdir().unwrap();
        let store = TomlStore::new(dir.path().join("gitdesktop.toml"));
        let now = OffsetDateTime::now_utc();

        // Dos guardados: el .bak contiene la primera versión.
        store.save(&sample(now)).unwrap();
        let mut v2 = sample(now);
        v2.ui.last_repo = Some("version-2".into());
        store.save(&v2).unwrap();

        // Corrompe el archivo principal.
        fs::write(store.path(), "esto no es toml [[[").unwrap();

        // Carga desde el backup sin bloquear la app.
        let loaded = store.load().unwrap();
        assert_eq!(loaded.repos.len(), 2);
    }

    #[test]
    fn touch_repo_dedupes_by_path() {
        let mut data = StoreData::default();
        let now = OffsetDateTime::now_utc();

        data.touch_repo(Path::new("/tmp/x"), now);
        data.touch_repo(Path::new("/tmp/x"), now);

        assert_eq!(data.repos.len(), 1);
    }

    #[test]
    fn toml_uses_repo_tables() {
        let dir = tempfile::tempdir().unwrap();
        let store = TomlStore::new(dir.path().join("gitdesktop.toml"));
        store.save(&sample(OffsetDateTime::now_utc())).unwrap();

        let raw = fs::read_to_string(store.path()).unwrap();
        assert!(raw.contains("[[repo]]"));
        assert!(raw.contains("last_opened_at"));
        assert!(raw.contains("theme = \"dark\""));
    }
}
