//! Estado de la aplicación (patrón Elm) y tareas asíncronas.

use std::path::PathBuf;

use gito_core::{GitEngine, RepoEntry, RepoStatus, RepoStore, StoreData, TomlStore};
use time::OffsetDateTime;

use crate::messages::{Message, Tab};

/// Repositorio actualmente abierto.
#[derive(Debug, Clone)]
pub struct OpenRepo {
    /// Entrada de la lista persistida (id, nombre, ruta…).
    pub entry: RepoEntry,
    /// Última instantánea de estado calculada.
    pub status: RepoStatus,
}

/// Estado raíz de la app. `iced` lo posee y llama a `update`/`view`.
pub struct App {
    /// Almacén persistente (`gito.toml`).
    pub store: TomlStore,
    /// Datos cargados del almacén.
    pub data: StoreData,

    /// Repositorio abierto (`None` → pantalla de bienvenida).
    pub repo: Option<OpenRepo>,
    /// Error visible en el banner superior.
    pub error: Option<String>,
    /// Hay una operación de apertura en curso.
    pub loading: bool,
    /// Pestaña activa del panel lateral.
    pub tab: Tab,
}

impl App {
    /// Carga la configuración persistida y agenda reabrir el último repo.
    pub fn new() -> (Self, iced::Task<Message>) {
        // Carga síncrona: el archivo es minúsculo y ocurre una sola vez.
        let (store, mut data, warning) = match TomlStore::open_default() {
            Ok(store) => match store.load() {
                Ok(data) => (store, data, None),
                Err(err) => (store, StoreData::default(), Some(err.to_string())),
            },
            Err(err) => {
                // Sin directorio de configuración (caso raro): modo efímero.
                let fallback =
                    TomlStore::new(std::env::temp_dir().join("gito").join("gito.toml"));
                let data = fallback.load().unwrap_or_default();
                (fallback, data, Some(err.to_string()))
            }
        };

        data.sort_recent();

        // Reabre automáticamente el último repositorio (como GitHub Desktop).
        let task = data
            .ui
            .last_repo
            .as_deref()
            .and_then(|id| data.find_by_id(id))
            .map(|entry| open_task(entry.path.clone()))
            .unwrap_or_else(iced::Task::none);

        (
            Self {
                store,
                data,
                repo: None,
                error: warning,
                loading: false,
                tab: Tab::Changes,
            },
            task,
        )
    }

    /// Título dinámico de la ventana.
    pub fn title(&self) -> String {
        match &self.repo {
            Some(open) => format!("Gito — {}", open.entry.name),
            None => "Gito".to_owned(),
        }
    }

    /// Lógica de actualización (arquitectura Elm).
    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::OpenFolderDialog => {
                self.loading = true;
                iced::Task::perform(pick_folder(), Message::FolderPicked)
            }
            Message::FolderPicked(Some(path)) => {
                self.loading = true;
                open_task(path)
            }
            Message::FolderPicked(None) => {
                self.loading = false;
                iced::Task::none()
            }
            Message::RepoOpened(Ok((path, status))) => {
                self.loading = false;
                self.error = None;

                let now = OffsetDateTime::now_utc();
                self.data.touch_repo(&path, now);

                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                if let Some(entry) = self
                    .data
                    .repos
                    .iter_mut()
                    .find(|r| r.path == canonical)
                {
                    entry.last_branch = Some(status.branch.clone());
                    let entry = entry.clone();

                    self.data.ui.last_repo = Some(entry.id.clone());
                    self.data.sort_recent();
                    self.persist();

                    // Restaura la última pestaña usada en este repo.
                    if let Some(tab) = &entry.last_tab {
                        self.tab = match tab.as_str() {
                            "history" => Tab::History,
                            _ => Tab::Changes,
                        };
                    }

                    self.repo = Some(OpenRepo { entry, status });
                }
                iced::Task::none()
            }
            Message::RepoOpened(Err(err)) => {
                self.loading = false;
                self.error = Some(err);
                iced::Task::none()
            }
            Message::OpenRecent(id) => {
                self.loading = true;
                match self.data.find_by_id(&id) {
                    Some(entry) => open_task(entry.path.clone()),
                    None => iced::Task::none(),
                }
            }
            Message::Refresh => {
                self.loading = true;
                match self.repo.as_ref() {
                    Some(open) => open_task(open.entry.path.clone()),
                    None => iced::Task::none(),
                }
            }
            Message::TabSelected(tab) => {
                self.tab = tab;
                if let Some(open) = self.repo.as_mut() {
                    open.entry.last_tab = Some(match tab {
                        Tab::Changes => "changes",
                        Tab::History => "history",
                    }
                    .to_owned());
                }
                self.persist();
                iced::Task::none()
            }
            Message::ClearError => {
                self.error = None;
                iced::Task::none()
            }
        }
    }

    /// Persiste los datos (escritura atómica; archivo minúsculo).
    fn persist(&mut self) {
        if let Err(err) = self.store.save(&self.data) {
            self.error = Some(err.to_string());
        }
    }
}

/// Tarea asíncrona: abrir un repositorio y calcular su estado.
pub fn open_task(path: PathBuf) -> iced::Task<Message> {
    iced::Task::perform(open_repo(path), Message::RepoOpened)
}

/// Abre el repo y lee su estado.
///
/// M0: se ejecuta en el executor de iced; las lecturas de libgit2 son
/// rápidas. Desde M3 esto correrá en el hilo worker del núcleo (PLAN.md §5).
async fn open_repo(path: PathBuf) -> Result<(PathBuf, RepoStatus), String> {
    let engine = gito_core::open(&path).map_err(|err| err.to_string())?;
    let status = engine.status().map_err(|err| err.to_string())?;
    Ok((engine.path().to_path_buf(), status))
}

/// Diálogo nativo de selección de carpeta (rfd asíncrono).
async fn pick_folder() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Abrir repositorio de Git")
        .pick_folder()
        .await
        .map(|handle| handle.path().to_path_buf())
}
