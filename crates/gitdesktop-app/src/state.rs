//! Estado de la aplicación (patrón Elm) y tareas asíncronas.

use std::path::PathBuf;
use std::time::Instant;

use gitdesktop_core::{
    CommitSummary, DiffLine, FileEntry, RepoEntry, RepoStatus, RepoStore, StoreData, TomlStore,
};
use iced::widget::text_editor;
use time::OffsetDateTime;

use crate::messages::{MenuAction, Message, Op, Tab};

/// Límite de commits cargados en el historial.
const LOG_LIMIT: usize = 200;

/// Intervalo mínimo entre refrescos automáticos (anti-rebote del watcher).
const REFRESH_MIN_INTERVAL: std::time::Duration = std::time::Duration::from_millis(800);

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
    pub store: TomlStore,
    pub data: StoreData,
    pub repo: Option<OpenRepo>,

    pub error: Option<String>,
    pub loading: bool,
    pub tab: Tab,

    // Cromo personalizado (barra de título + menús)
    pub open_menu: Option<usize>,
    pub about: bool,
    pub maximized: bool,
    pub window_id: Option<iced::window::Id>,
    pub last_refresh: Option<Instant>,

    // Pestaña Cambios
    pub selected: Option<(String, bool)>,
    pub diff: Option<Vec<DiffLine>>,
    pub summary: String,
    pub description: text_editor::Content,
    pub pending_discard: Option<String>,

    // Pestaña Historial
    pub history: Vec<CommitSummary>,
    pub selected_commit: Option<String>,
    pub commit_files: Vec<FileEntry>,
    pub selected_commit_file: Option<String>,
    pub commit_diff: Option<Vec<DiffLine>>,
}

impl App {
    /// Carga la configuración persistida y agenda reabrir el último repo.
    pub fn new() -> (Self, iced::Task<Message>) {
        let (store, mut data, warning) = match TomlStore::open_default() {
            Ok(store) => match store.load() {
                Ok(data) => (store, data, None),
                Err(err) => (store, StoreData::default(), Some(err.to_string())),
            },
            Err(err) => {
                let fallback = TomlStore::new(
                    std::env::temp_dir().join("git-desktop-jorgenio").join("gitdesktop.toml"),
                );
                let data = fallback.load().unwrap_or_default();
                (fallback, data, Some(err.to_string()))
            }
        };

        data.sort_recent();

        // Captura el Id de la ventana principal y reabre el último repo.
        let mut tasks = vec![iced::window::latest().map(Message::MainWindowId)];
        if let Some(entry) = data
            .ui
            .last_repo
            .as_deref()
            .and_then(|id| data.find_by_id(id))
        {
            tasks.push(open_task(entry.path.clone()));
        }

        (
            Self {
                store,
                data,
                repo: None,
                error: warning,
                loading: false,
                tab: Tab::Changes,
                open_menu: None,
                about: false,
                maximized: false,
                window_id: None,
                last_refresh: None,
                selected: None,
                diff: None,
                summary: String::new(),
                description: text_editor::Content::new(),
                pending_discard: None,
                history: Vec::new(),
                selected_commit: None,
                commit_files: Vec::new(),
                selected_commit_file: None,
                commit_diff: None,
            },
            iced::Task::batch(tasks),
        )
    }

    /// Título dinámico de la ventana.
    pub fn title(&self) -> String {
        match &self.repo {
            Some(open) => format!("Git-Desktop Jorgenio — {}", open.entry.name),
            None => "Git-Desktop Jorgenio".to_owned(),
        }
    }

    /// Vista raíz (delegada al módulo `views`).
    pub fn view(&self) -> iced::Element<'_, Message> {
        crate::views::view(self)
    }

    /// Suscripciones: refresco automático periódico mientras haya un repo
    /// abierto (MVP: polling; el watcher con `notify` llega en M3).
    pub fn subscription(&self) -> iced::Subscription<Message> {
        if self.repo.is_some() {
            iced::time::every(std::time::Duration::from_millis(2000))
                .map(|_| Message::AutoRefresh)
        } else {
            iced::Subscription::none()
        }
    }

    /// Lógica de actualización (arquitectura Elm).
    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            // ——— Menú y ventana ———
            Message::MenuToggled(index) => {
                self.open_menu = match (self.open_menu, index) {
                    (Some(current), Some(requested)) if current == requested => None,
                    (_, requested) => requested,
                };
                iced::Task::none()
            }
            Message::MenuSelected(action) => {
                self.open_menu = None;
                match action {
                    MenuAction::OpenRepository => {
                        self.loading = true;
                        iced::Task::perform(pick_folder(), Message::FolderPicked)
                    }
                    MenuAction::Refresh => {
                        self.last_refresh = None;
                        self.refresh()
                    }
                    MenuAction::ShowChanges => {
                        self.tab = Tab::Changes;
                        iced::Task::none()
                    }
                    MenuAction::ShowHistory => {
                        self.tab = Tab::History;
                        self.ensure_log()
                    }
                    MenuAction::About => {
                        self.about = true;
                        iced::Task::none()
                    }
                    MenuAction::Quit => iced::exit(),
                }
            }
            Message::TitleBarPressed => match self.window_id {
                Some(id) => iced::window::drag(id),
                None => iced::Task::none(),
            },
            Message::MinimizeWindow => match self.window_id {
                Some(id) => iced::window::minimize(id, true),
                None => iced::Task::none(),
            },
            Message::ToggleMaximize => {
                self.maximized = !self.maximized;
                match self.window_id {
                    Some(id) => iced::window::maximize(id, self.maximized),
                    None => iced::Task::none(),
                }
            }
            Message::CloseWindow => match self.window_id {
                Some(id) => iced::window::close(id),
                None => iced::Task::none(),
            },
            Message::MainWindowId(id) => {
                self.window_id = id;
                iced::Task::none()
            }
            Message::AboutToggled(open) => {
                self.about = open;
                iced::Task::none()
            }

            // ——— Repositorios ———
            Message::OpenFolderDialog => {
                self.loading = true;
                iced::Task::perform(pick_folder(), Message::FolderPicked)
            }
            Message::FolderPicked(Some(path)) => open_task(path),
            Message::FolderPicked(None) => {
                self.loading = false;
                iced::Task::none()
            }
            Message::OpenRecent(id) => match self.data.find_by_id(&id) {
                Some(entry) => open_task(entry.path.clone()),
                None => iced::Task::none(),
            },
            Message::RepoOpened(result) => match result {
                Ok((path, status)) => self.on_repo_opened(path, status),
                Err(err) => {
                    self.loading = false;
                    self.error = Some(err);
                    iced::Task::none()
                }
            },
            Message::AutoRefresh => self.refresh(),
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
                if tab == Tab::History {
                    self.ensure_log()
                } else {
                    iced::Task::none()
                }
            }

            // ——— Pestaña Cambios ———
            Message::FileSelected { path, staged } => {
                self.selected = Some((path.clone(), staged));
                self.diff = None;
                match self.repo_path() {
                    Some(repo_path) => diff_task(repo_path, path, staged),
                    None => iced::Task::none(),
                }
            }
            Message::ToggleStaged { path, staged } => {
                self.loading = true;
                match self.repo_path() {
                    Some(repo_path) => {
                        if staged {
                            stage_task(repo_path, path)
                        } else {
                            unstage_task(repo_path, path)
                        }
                    }
                    None => {
                        self.loading = false;
                        iced::Task::none()
                    }
                }
            }
            Message::RequestDiscard { path } => {
                self.pending_discard = Some(path);
                iced::Task::none()
            }
            Message::ConfirmDiscard => match (self.repo_path(), self.pending_discard.take())
            {
                (Some(repo_path), Some(file)) => {
                    self.loading = true;
                    discard_task(repo_path, file)
                }
                _ => iced::Task::none(),
            },
            Message::CancelDiscard => {
                self.pending_discard = None;
                iced::Task::none()
            }
            Message::SummaryChanged(summary) => {
                self.summary = summary;
                iced::Task::none()
            }
            Message::DescriptionEdited(action) => {
                self.description.perform(action);
                iced::Task::none()
            }
            Message::CommitPressed => {
                let summary = self.summary.trim().to_owned();
                let body = self.description.text();
                if summary.is_empty() || self.repo.is_none() {
                    return iced::Task::none();
                }
                self.loading = true;
                match self.repo_path() {
                    Some(repo_path) => commit_task(repo_path, summary, body),
                    None => {
                        self.loading = false;
                        iced::Task::none()
                    }
                }
            }
            Message::OpDone(op, result) => self.on_op_done(op, result),
            Message::DiffLoaded(result) => match result {
                Ok((path, lines)) => {
                    if self.selected.as_ref().map(|(p, _)| p.as_str()) == Some(path.as_str()) {
                        self.diff = Some(lines);
                    }
                    iced::Task::none()
                }
                Err(err) => {
                    self.error = Some(err);
                    iced::Task::none()
                }
            },

            // ——— Pestaña Historial ———
            Message::LogLoaded(result) => match result {
                Ok(commits) => {
                    self.history = commits;
                    if self.selected_commit.is_none() {
                        if let Some(first) = self.history.first().cloned() {
                            self.selected_commit = Some(first.id.clone());
                            return match self.repo_path() {
                                Some(repo_path) => commit_files_task(repo_path, first.id),
                                None => iced::Task::none(),
                            };
                        }
                    }
                    iced::Task::none()
                }
                Err(err) => {
                    self.error = Some(err);
                    iced::Task::none()
                }
            },
            Message::CommitSelected(id) => {
                self.selected_commit = Some(id.clone());
                self.commit_files.clear();
                self.selected_commit_file = None;
                self.commit_diff = None;
                match self.repo_path() {
                    Some(repo_path) => commit_files_task(repo_path, id),
                    None => iced::Task::none(),
                }
            }
            Message::CommitFilesLoaded(result) => match result {
                Ok((commit_id, files)) => {
                    if self.selected_commit.as_deref() == Some(commit_id.as_str()) {
                        self.commit_files = files;
                    }
                    iced::Task::none()
                }
                Err(err) => {
                    self.error = Some(err);
                    iced::Task::none()
                }
            },
            Message::CommitFileSelected(file) => {
                self.selected_commit_file = Some(file.clone());
                self.commit_diff = None;
                match (self.repo_path(), self.selected_commit.clone()) {
                    (Some(repo_path), Some(commit_id)) => {
                        commit_diff_task(repo_path, commit_id, file)
                    }
                    _ => iced::Task::none(),
                }
            }
            Message::CommitDiffLoaded(result) => match result {
                Ok((commit_id, file, lines)) => {
                    if self.selected_commit.as_deref() == Some(commit_id.as_str())
                        && self.selected_commit_file.as_deref() == Some(file.as_str())
                    {
                        self.commit_diff = Some(lines);
                    }
                    iced::Task::none()
                }
                Err(err) => {
                    self.error = Some(err);
                    iced::Task::none()
                }
            },

            Message::ClearError => {
                self.error = None;
                iced::Task::none()
            }
        }
    }

    // ———————————————————————— auxiliares de update ————————————————————————

    fn on_repo_opened(
        &mut self,
        path: PathBuf,
        status: gitdesktop_core::RepoStatus,
    ) -> iced::Task<Message> {
        self.loading = false;
        self.error = None;

        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        let is_switching = self
            .repo
            .as_ref()
            .map(|open| open.entry.path.clone())
            != Some(canonical.clone());

        if is_switching {
            // Limpia el estado dependiente del repositorio anterior.
            self.selected = None;
            self.diff = None;
            self.summary.clear();
            self.description = text_editor::Content::new();
            self.pending_discard = None;
            self.history.clear();
            self.selected_commit = None;
            self.commit_files.clear();
            self.selected_commit_file = None;
            self.commit_diff = None;
        }

        self.data.touch_repo(&path, OffsetDateTime::now_utc());
        if let Some(entry) = self.data.repos.iter_mut().find(|r| r.path == canonical) {
            entry.last_branch = Some(status.branch.clone());
            let entry = entry.clone();

            self.data.ui.last_repo = Some(entry.id.clone());
            self.data.sort_recent();
            self.persist();
            self.repo = Some(OpenRepo { entry, status });
        }

        // Recarga lo que está visible.
        let mut tasks: Vec<iced::Task<Message>> = Vec::new();
        if let Some((file, staged)) = self.selected.clone() {
            tasks.push(diff_task(path.clone(), file, staged));
        }
        if self.tab == Tab::History {
            tasks.push(log_task(path));
        }
        iced::Task::batch(tasks)
    }

    fn on_op_done(&mut self, op: Op, result: Result<(), String>) -> iced::Task<Message> {
        self.loading = false;
        match result {
            Ok(()) => {
                self.error = None;
                if op == Op::Commit {
                    self.summary.clear();
                    self.description = text_editor::Content::new();
                    self.selected = None;
                    self.diff = None;
                }
                self.last_refresh = None;
                self.refresh()
            }
            Err(err) => {
                self.error = Some(err);
                iced::Task::none()
            }
        }
    }

    fn repo_path(&self) -> Option<PathBuf> {
        self.repo.as_ref().map(|open| open.entry.path.clone())
    }

    /// Refresca el estado respetando el anti-rebote.
    fn refresh(&mut self) -> iced::Task<Message> {
        let Some(open) = self.repo.as_ref() else {
            return iced::Task::none();
        };
        let now = Instant::now();
        if let Some(last) = self.last_refresh {
            if now.duration_since(last) < REFRESH_MIN_INTERVAL {
                return iced::Task::none();
            }
        }
        self.last_refresh = Some(now);
        self.loading = true;

        let path = open.entry.path.clone();
        iced::Task::batch(vec![open_task(path)])
    }

    /// Carga el historial si aún no está en memoria.
    fn ensure_log(&mut self) -> iced::Task<Message> {
        if self.history.is_empty() {
            match self.repo_path() {
                Some(repo_path) => log_task(repo_path),
                None => iced::Task::none(),
            }
        } else {
            iced::Task::none()
        }
    }

    /// Persiste los datos (escritura atómica; archivo minúsculo).
    fn persist(&mut self) {
        if let Err(err) = self.store.save(&self.data) {
            self.error = Some(err.to_string());
        }
    }
}

// ————————————————————————————— tareas ————————————————————————————————

/// Tarea asíncrona: abrir un repositorio y calcular su estado.
pub fn open_task(path: PathBuf) -> iced::Task<Message> {
    iced::Task::perform(open_repo(path), Message::RepoOpened)
}

async fn open_repo(path: PathBuf) -> Result<(PathBuf, gitdesktop_core::RepoStatus), String> {
    let engine = gitdesktop_core::open(&path).map_err(|err| err.to_string())?;
    let status = engine.status().map_err(|err| err.to_string())?;
    Ok((engine.path().to_path_buf(), status))
}

/// Ejecuta una operación genérica del motor y reporta con [`Op`].
fn op_task<F>(path: PathBuf, op: Op, f: F) -> iced::Task<Message>
where
    F: FnOnce(&dyn gitdesktop_core::GitEngine) -> gitdesktop_core::Result<()> + Send + 'static,
{
    iced::Task::perform(
        async move {
            let engine = gitdesktop_core::open(&path).map_err(|err| err.to_string())?;
            f(engine.as_ref()).map_err(|err| err.to_string())
        },
        move |result| Message::OpDone(op, result),
    )
}

fn stage_task(path: PathBuf, file: String) -> iced::Task<Message> {
    op_task(path, Op::Stage, move |engine| engine.stage(&file))
}

fn unstage_task(path: PathBuf, file: String) -> iced::Task<Message> {
    op_task(path, Op::Unstage, move |engine| engine.unstage(&file))
}

fn discard_task(path: PathBuf, file: String) -> iced::Task<Message> {
    op_task(path, Op::Discard, move |engine| engine.discard(&file))
}

fn commit_task(path: PathBuf, summary: String, body: String) -> iced::Task<Message> {
    op_task(path, Op::Commit, move |engine| {
        engine.commit(&summary, &body)
    })
}

fn diff_task(path: PathBuf, file: String, staged: bool) -> iced::Task<Message> {
    iced::Task::perform(
        async move {
            let engine = gitdesktop_core::open(&path).map_err(|err| err.to_string())?;
            let lines = engine.file_diff(&file, staged).map_err(|err| err.to_string())?;
            Ok((file, lines))
        },
        Message::DiffLoaded,
    )
}

fn log_task(path: PathBuf) -> iced::Task<Message> {
    iced::Task::perform(
        async move {
            let engine = gitdesktop_core::open(&path).map_err(|err| err.to_string())?;
            engine.log(LOG_LIMIT).map_err(|err| err.to_string())
        },
        Message::LogLoaded,
    )
}

fn commit_files_task(path: PathBuf, commit_id: String) -> iced::Task<Message> {
    iced::Task::perform(
        async move {
            let engine = gitdesktop_core::open(&path).map_err(|err| err.to_string())?;
            let files = engine
                .commit_files(&commit_id)
                .map_err(|err| err.to_string())?;
            Ok((commit_id, files))
        },
        Message::CommitFilesLoaded,
    )
}

fn commit_diff_task(path: PathBuf, commit_id: String, file: String) -> iced::Task<Message> {
    iced::Task::perform(
        async move {
            let engine = gitdesktop_core::open(&path).map_err(|err| err.to_string())?;
            let lines = engine
                .commit_diff(&commit_id, &file)
                .map_err(|err| err.to_string())?;
            Ok((commit_id, file, lines))
        },
        Message::CommitDiffLoaded,
    )
}

/// Diálogo nativo de selección de carpeta (rfd asíncrono).
async fn pick_folder() -> Option<PathBuf> {
    rfd::AsyncFileDialog::new()
        .set_title("Abrir repositorio de Git")
        .pick_folder()
        .await
        .map(|handle| handle.path().to_path_buf())
}
