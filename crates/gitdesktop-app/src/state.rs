//! Estado de la aplicación (patrón Elm) y tareas asíncronas.

use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use gitdesktop_core::{
    BranchInfo, CommitSummary, DiffLine, FileEntry, MergeOutcome, NetOpKind, Progress, RepoEntry,
    RepoStatus, RepoStore, StoreData, TomlStore,
};
use iced::widget::text_editor;
use time::OffsetDateTime;

use crate::messages::{BranchOpResult, MenuAction, Message, Op, Tab};

/// Límite de commits cargados en el historial.
const LOG_LIMIT: usize = 200;

/// Intervalo mínimo entre refrescos automáticos (anti-rebote).
const REFRESH_MIN_INTERVAL: Duration = Duration::from_millis(800);

/// Frecuencia del fetch automático en segundo plano.
const AUTO_FETCH_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Sufijo de ejecutables según la plataforma.
#[cfg(target_os = "windows")]
const BINARY_SUFFIX: &str = ".exe";
#[cfg(not(target_os = "windows"))]
const BINARY_SUFFIX: &str = "";

/// Operación de red en curso (identidad de la suscripción + guardas).
#[derive(Debug, Clone)]
pub struct NetRun {
    pub path: PathBuf,
    pub branch: String,
    pub kind: NetOpKind,
}

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
    pub notice: Option<String>,
    pub loading: bool,
    pub tab: Tab,

    // Red (fetch/pull/push/clone)
    pub running_op: Option<NetRun>,
    pub net_progress: Option<Progress>,
    pub last_fetch: Option<Instant>,

    // Ramas (M4)
    pub branches: Vec<BranchInfo>,
    pub branch_modal: bool,
    pub new_branch_name: String,
    pub pending_delete_branch: Option<String>,
    pub merge_modal: bool,
    pub merge_target: Option<String>,

    // Clone (M4)
    pub clone_modal: bool,
    pub clone_url: String,
    pub clone_folder: Option<PathBuf>,

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
                notice: None,
                loading: false,
                tab: Tab::Changes,
                running_op: None,
                net_progress: None,
                last_fetch: None,
                branches: Vec::new(),
                branch_modal: false,
                new_branch_name: String::new(),
                pending_delete_branch: None,
                merge_modal: false,
                merge_target: None,
                clone_modal: false,
                clone_url: String::new(),
                clone_folder: None,
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

    /// Suscripciones: refresco periódico, fetch automático y operación
    /// de red en curso.
    pub fn subscription(&self) -> iced::Subscription<Message> {
        let mut subs: Vec<iced::Subscription<Message>> = Vec::new();

        if let Some(open) = &self.repo {
            // Watcher notify (guardados) + temporizador de auto-fetch.
            subs.push(crate::subscriptions::repository_activities(
                open.entry.path.clone(),
            ));
        }

        if let Some(run) = &self.running_op {
            subs.push(crate::subscriptions::network_op(
                run.path.clone(),
                run.branch.clone(),
                run.kind.clone(),
            ));
        }

        match subs.is_empty() {
            true => iced::Subscription::none(),
            false => iced::Subscription::batch(subs),
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
                    MenuAction::Clone => {
                        self.clone_modal = true;
                        iced::Task::none()
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
                    MenuAction::NewBranch => {
                        self.new_branch_name.clear();
                        self.branch_modal = true;
                        self.load_branches()
                    }
                    MenuAction::MergeBranches => {
                        self.merge_modal = true;
                        self.merge_target = None;
                        self.load_branches()
                    }
                    MenuAction::ToggleTheme => self.toggle_theme(),
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
                Ok((path, status, auto_resolved)) => {
                    self.on_repo_opened(path, status, auto_resolved)
                }
                Err(err) => {
                    self.loading = false;
                    self.error = Some(err);
                    iced::Task::none()
                }
            },
            Message::AutoRefresh => {
                let refresh_task = self.refresh();
                let fetch_task = self.maybe_auto_fetch();
                iced::Task::batch([refresh_task, fetch_task])
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
            Message::ConfirmDiscard => match (self.repo_path(), self.pending_discard.take()) {
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

            Message::ResolveInEditor { path } => {
                let Some(open) = self.repo.as_ref() else {
                    return iced::Task::none();
                };
                let full = open.entry.path.join(&path);
                if Self::which_exists("jorgenio") {
                    match std::process::Command::new("jorgenio").arg(&full).spawn() {
                        Ok(_) => {
                            self.notice =
                                Some(format!("Abierto en Jorgenio Text: {path}"));
                        }
                        Err(err) => {
                            self.error = Some(format!("No se pudo abrir jorgenio: {err}"));
                        }
                    }
                } else {
                    self.error = Some(
                        "Jorgenio Text no está en PATH. Instálalo (cargo install --path                          Projects/text-editor/crates/jorgenio) o abre el archivo con tu editor."
                            .into(),
                    );
                    let _ = open::that(&full);
                }
                iced::Task::none()
            }
            Message::ClearError => {
                self.error = None;
                iced::Task::none()
            }
            Message::ClearNotice => {
                self.notice = None;
                iced::Task::none()
            }

            // ——— Red (fetch/pull/push) ———
            Message::NetStart(kind) => self.start_net(kind),
            Message::NetProgress(progress) => {
                self.net_progress = Some(progress);
                iced::Task::none()
            }
            Message::NetDone(kind, result) => {
                self.running_op = None;
                self.net_progress = None;
                if matches!(kind, NetOpKind::Fetch | NetOpKind::Pull) {
                    self.last_fetch = Some(Instant::now());
                }
                match result {
                    Ok(_summary) => self.error = None,
                    Err(err) => self.error = Some(err),
                }
                // Un clone exitoso abre directamente el repo recién clonado.
                if let NetOpKind::Clone { dest, .. } = &kind {
                    return open_task(dest.clone());
                }
                self.last_refresh = None;
                self.refresh()
            }

            // ——— Ramas ———
            Message::BranchModalToggled(open) => {
                self.branch_modal = open;
                if open {
                    self.load_branches()
                } else {
                    iced::Task::none()
                }
            }
            Message::BranchesLoaded(result) => match result {
                Ok(branches) => {
                    self.branches = branches;
                    iced::Task::none()
                }
                Err(err) => {
                    self.error = Some(err);
                    iced::Task::none()
                }
            },
            Message::NewBranchNameChanged(name) => {
                self.new_branch_name = name;
                iced::Task::none()
            }
            Message::BranchCreatePressed => {
                let name = self.new_branch_name.trim().to_owned();
                if name.is_empty() {
                    return iced::Task::none();
                }
                self.loading = true;
                match self.repo_path() {
                    Some(repo_path) => branch_task(
                        repo_path,
                        BranchOpResult::Created,
                        move |engine| engine.create_branch(&name),
                    ),
                    None => {
                        self.loading = false;
                        iced::Task::none()
                    }
                }
            }
            Message::BranchSwitch(name) => {
                self.loading = true;
                match self.repo_path() {
                    Some(repo_path) => branch_task(
                        repo_path,
                        BranchOpResult::Switched,
                        move |engine| engine.switch_branch(&name),
                    ),
                    None => {
                        self.loading = false;
                        iced::Task::none()
                    }
                }
            }
            Message::BranchDeleteRequested(name) => {
                self.pending_delete_branch = Some(name);
                iced::Task::none()
            }
            Message::BranchDeleteConfirmed => {
                match (self.repo_path(), self.pending_delete_branch.take()) {
                    (Some(repo_path), Some(name)) => {
                        self.loading = true;
                        branch_task(
                            repo_path,
                            BranchOpResult::Deleted,
                            move |engine| engine.delete_branch(&name),
                        )
                    }
                    _ => iced::Task::none(),
                }
            }
            Message::BranchCancelDelete => {
                self.pending_delete_branch = None;
                iced::Task::none()
            }
            Message::BranchDone(_op, result) => self.on_branch_done(result),

            // ——— Merge ———
            Message::MergeModalToggled(open) => {
                self.merge_modal = open;
                if open {
                    self.merge_target = None;
                    self.load_branches()
                } else {
                    iced::Task::none()
                }
            }
            Message::MergeTargetPicked(target) => {
                self.merge_target = target;
                iced::Task::none()
            }
            Message::MergePressed => {
                let Some(target) = self.merge_target.clone() else {
                    return iced::Task::none();
                };
                let Some(repo_path) = self.repo_path() else {
                    return iced::Task::none();
                };
                self.loading = true;
                merge_task(repo_path, target)
            }
            Message::MergeDone(result) => self.on_merge_done(result),
            Message::MergeCommitPressed => {
                self.loading = true;
                match self.repo_path() {
                    Some(repo_path) => {
                        op_task(repo_path, Op::MergeCommit, |engine| engine.commit_merge())
                    }
                    None => {
                        self.loading = false;
                        iced::Task::none()
                    }
                }
            }
            Message::MergeAborted => {
                self.loading = true;
                match self.repo_path() {
                    Some(repo_path) => abort_task(repo_path),
                    None => {
                        self.loading = false;
                        iced::Task::none()
                    }
                }
            }

            // ——— Clone ———
            Message::CloneModalToggled(open) => {
                self.clone_modal = open;
                iced::Task::none()
            }
            Message::CloneUrlChanged(url) => {
                self.clone_url = url;
                iced::Task::none()
            }
            Message::CloneDestPicked(folder) => {
                self.clone_folder = folder;
                iced::Task::none()
            }
            Message::ClonePickFolder => {
                iced::Task::perform(pick_folder(), Message::CloneDestPicked)
            }
            Message::ClonePressed => {
                if self.running_op.is_some() {
                    return iced::Task::none();
                }
                let url = self.clone_url.trim().to_owned();
                let Some(folder) = self.clone_folder.clone() else {
                    self.error = Some("Elige la carpeta de destino para el clon.".into());
                    return iced::Task::none();
                };
                if url.is_empty() {
                    self.error = Some("Escribe la URL del repositorio a clonar.".into());
                    return iced::Task::none();
                }
                // Destino final = carpeta elegida + nombre derivado de la URL.
                let name = url
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or("repo")
                    .trim_end_matches(".git")
                    .to_owned();
                let name = if name.is_empty() { "repo".into() } else { name };
                let dest = folder.join(name);

                self.net_progress = None;
                self.running_op = Some(NetRun {
                    path: dest.clone(),
                    branch: String::new(),
                    kind: NetOpKind::Clone { url, dest },
                });
                iced::Task::none()
            }
        }
    }

    // ———————————————————————— auxiliares de update ————————————————————————

    fn on_repo_opened(
        &mut self,
        path: PathBuf,
        status: RepoStatus,
        auto_resolved: Vec<String>,
    ) -> iced::Task<Message> {
        self.loading = false;
        self.error = None;
        if !auto_resolved.is_empty() {
            self.notice = Some(format!(
                "Resueltos automáticamente al guardar (sin marcadores): {}",
                auto_resolved.join(", ")
            ));
        }

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
            self.branches.clear();
            self.notice = None;
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
            tasks.push(log_task(path.clone()));
        }
        tasks.push(branches_task(path));
        iced::Task::batch(tasks)
    }

    fn on_op_done(&mut self, op: Op, result: Result<(), String>) -> iced::Task<Message> {
        self.loading = false;
        match result {
            Ok(()) => {
                self.error = None;
                if matches!(op, Op::Commit | Op::MergeCommit) {
                    if op == Op::Commit {
                        self.summary.clear();
                        self.description = text_editor::Content::new();
                    }
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

    /// Tras crear/cambiar/eliminar una rama: cierra modal y refresca todo.
    fn on_branch_done(&mut self, result: Result<(), String>) -> iced::Task<Message> {
        self.loading = false;
        match result {
            Ok(()) => {
                self.error = None;
                self.notice = None;
                self.branch_modal = false;
                self.pending_delete_branch = None;
                self.selected = None;
                self.diff = None;
                self.last_refresh = None;
                let refresh = self.refresh();
                match self.repo_path() {
                    Some(path) => iced::Task::batch([refresh, branches_task(path)]),
                    None => refresh,
                }
            }
            Err(err) => {
                self.error = Some(err);
                iced::Task::none()
            }
        }
    }

    /// Tras un merge: banners según el resultado y refresco completo.
    fn on_merge_done(&mut self, result: Result<MergeOutcome, String>) -> iced::Task<Message> {
        self.loading = false;
        match result {
            Ok(MergeOutcome::Conflicts(files)) => {
                self.error = Some(format!(
                    "El merge dejó conflictos en {} archivo(s): {}. Edítalos, \
                     márcalos como resueltos con el checkbox y pulsa «Confirmar merge».",
                    files.len(),
                    files.join(", ")
                ));
                self.after_merge_refresh()
            }
            Ok(outcome) => {
                self.error = None;
                self.notice = Some(match outcome {
                    MergeOutcome::Merged => "Merge realizado con commit de fusión.".into(),
                    MergeOutcome::FastForward => "Fusión fast-forward aplicada.".into(),
                    MergeOutcome::UpToDate => "Todo ya estaba integrado; nada que fusionar.".into(),
                    MergeOutcome::Aborted => {
                        "Merge abortado: el repositorio volvió a su estado anterior.".into()
                    }
                    MergeOutcome::Conflicts(_) => unreachable!("manejado arriba"),
                });
                self.after_merge_refresh()
            }
            Err(err) => {
                self.error = Some(err);
                iced::Task::none()
            }
        }
    }

    fn after_merge_refresh(&mut self) -> iced::Task<Message> {
        self.selected = None;
        self.diff = None;
        self.last_refresh = None;
        let refresh = self.refresh();
        match self.repo_path() {
            Some(path) => iced::Task::batch([refresh, branches_task(path)]),
            None => refresh,
        }
    }

    /// Recarga la lista de ramas.
    fn load_branches(&mut self) -> iced::Task<Message> {
        match self.repo_path() {
            Some(path) => branches_task(path),
            None => iced::Task::none(),
        }
    }

    /// Alterna claro/oscuro y persiste la preferencia.
    fn toggle_theme(&mut self) -> iced::Task<Message> {
        self.data.ui.theme = match self.data.ui.theme.as_str() {
            "light" => "dark".to_owned(),
            _ => "light".to_owned(),
        };
        self.persist();
        iced::Task::none()
    }

    /// ¿Existe un ejecutable en PATH?
    fn which_exists(binary: &str) -> bool {
        std::env::var_os("PATH")
            .map(|paths| {
                std::env::split_paths(&paths).any(|dir| {
                    dir.join(format!("{binary}{BINARY_SUFFIX}")).is_file()
                })
            })
            .unwrap_or(false)
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

    /// Inicia una operación de red (si no hay otra en curso).
    fn start_net(&mut self, kind: NetOpKind) -> iced::Task<Message> {
        if self.running_op.is_some() {
            return iced::Task::none();
        }
        let Some(open) = self.repo.as_ref() else {
            return iced::Task::none();
        };
        if open.status.branch.is_empty() {
            self.error = Some("El repositorio aún no tiene commits; haz el primero.".into());
            return iced::Task::none();
        }

        self.net_progress = None;
        if kind == NetOpKind::Fetch {
            // Marca ya la hora para no re-disparar el auto-fetch en bucle.
            self.last_fetch = Some(Instant::now());
        }
        self.running_op = Some(NetRun {
            path: open.entry.path.clone(),
            branch: open.status.branch.clone(),
            kind,
        });
        iced::Task::none()
    }

    /// Fetch automático cada `AUTO_FETCH_INTERVAL` si hay remoto y la app
    /// está ociosa (mantiene vivos el indicador "hace X" y el ↓/↑).
    fn maybe_auto_fetch(&mut self) -> iced::Task<Message> {
        if self.running_op.is_some() {
            return iced::Task::none();
        }
        let Some(open) = self.repo.as_ref() else {
            return iced::Task::none();
        };
        if !open.status.has_remote {
            return iced::Task::none();
        }

        let due = match self.last_fetch {
            None => true,
            Some(last) => last.elapsed() >= AUTO_FETCH_INTERVAL,
        };

        if due {
            self.start_net(NetOpKind::Fetch)
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

async fn open_repo(
    path: PathBuf,
) -> Result<(PathBuf, RepoStatus, Vec<String>), String> {
    let engine = gitdesktop_core::open(&path).map_err(|err| err.to_string())?;
    let mut status = engine.status().map_err(|err| err.to_string())?;

    // Auto-resolución: si un archivo en conflicto ya no tiene marcadores
    // (el usuario lo editó fuera, p. ej. en Jorgenio Text), se prepara
    // como resuelto y se avisa en la próxima actualización.
    let mut auto_resolved: Vec<String> = Vec::new();
    for entry in &status.entries {
        if entry.status == gitdesktop_core::FileStatus::Conflicted
            && matches!(engine.conflict_markers_remaining(&entry.path), Ok(false))
        {
            if engine.stage(&entry.path).is_ok() {
                auto_resolved.push(entry.path.clone());
            }
        }
    }
    if !auto_resolved.is_empty() {
        status = engine.status().map_err(|err| err.to_string())?;
    }

    Ok((engine.path().to_path_buf(), status, auto_resolved))
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

/// Recarga la lista de ramas del repositorio.
fn branches_task(path: PathBuf) -> iced::Task<Message> {
    iced::Task::perform(
        async move {
            let engine = gitdesktop_core::open(&path).map_err(|err| err.to_string())?;
            engine.branches().map_err(|err| err.to_string())
        },
        Message::BranchesLoaded,
    )
}

/// Ejecuta una operación de rama (crear/cambiar/eliminar).
fn branch_task<F>(
    path: PathBuf,
    result: BranchOpResult,
    f: F,
) -> iced::Task<Message>
where
    F: FnOnce(&dyn gitdesktop_core::GitEngine) -> gitdesktop_core::Result<()> + Send + 'static,
{
    iced::Task::perform(
        async move {
            let engine = gitdesktop_core::open(&path).map_err(|err| err.to_string())?;
            f(engine.as_ref()).map_err(|err| err.to_string())
        },
        move |outcome| Message::BranchDone(result, outcome),
    )
}

/// Fusiona otra rama en la actual.
fn merge_task(path: PathBuf, name: String) -> iced::Task<Message> {
    iced::Task::perform(
        async move {
            let engine = gitdesktop_core::open(&path).map_err(|err| err.to_string())?;
            engine.merge(&name).map_err(|err| err.to_string())
        },
        Message::MergeDone,
    )
}

/// Abandona el merge en curso.
fn abort_task(path: PathBuf) -> iced::Task<Message> {
    iced::Task::perform(
        async move {
            let engine = gitdesktop_core::open(&path).map_err(|err| err.to_string())?;
            engine.abort_merge().map_err(|err| err.to_string())?;
            Ok(MergeOutcome::Aborted)
        },
        Message::MergeDone,
    )
}
