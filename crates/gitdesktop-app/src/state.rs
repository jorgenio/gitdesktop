//! Estado de la aplicación (patrón Elm) y tareas asíncronas.

use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use gitdesktop_core::{
    BlameLine, BranchInfo, CommitSummary, DiffLine, FileEntry, MergeOutcome, NetOpKind, Progress,
    RepoEntry, RepoStatus, RepoStore, StashEntry, StoreData, TomlStore,
};
use iced::widget::text_editor;
use time::OffsetDateTime;

use crate::messages::{BranchOpResult, MenuAction, Message, MergeMode, Op, Tab};
use crate::widgets::commit_graph::GraphRow;
use crate::widgets::conflict_editor::{ConflictEditor, EditAction, EDITOR_VISIBLE_LINES};

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
    /// Carriles del grafo (uno por commit, misma longitud que `history`).
    pub graph_rows: Vec<GraphRow>,
    /// Modal de fusión en modo merge o rebase.
    pub merge_mode: MergeMode,
    pub new_branch_name: String,
    pub pending_delete_branch: Option<String>,
    pub merge_modal: bool,
    pub merge_target: Option<String>,

    // Clone (M4)
    pub clone_modal: bool,
    pub repo_menu_open: bool,
    pub clone_url: String,
    pub clone_folder: Option<PathBuf>,

    // Editor de conflictos embebido (fase 2)
    pub conflict_editor: Option<ConflictEditor>,

    // GitHub / PRs (M7)
    pub prs: Vec<crate::github::PullRequest>,
    pub prs_loaded: bool,
    pub selected_pr: Option<i64>,
    pub pr_modal: bool,
    pub pr_title: String,
    pub pr_body: iced::widget::text_editor::Content,

    // Stash / revert / cherry-pick / blame (M6)
    pub stashes: Vec<StashEntry>,
    pub stashes_visible: bool,
    pub pending_drop_stash: Option<String>,
    pub blame: Option<(String, Vec<BlameLine>)>,

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
                graph_rows: Vec::new(),
                merge_mode: MergeMode::Merge,
                branch_modal: false,
                new_branch_name: String::new(),
                pending_delete_branch: None,
                merge_modal: false,
                merge_target: None,
                clone_modal: false,
                repo_menu_open: false,
                clone_url: String::new(),
                clone_folder: None,
                conflict_editor: None,
                prs: Vec::new(),
                prs_loaded: false,
                selected_pr: None,
                pr_modal: false,
                pr_title: String::new(),
                pr_body: iced::widget::text_editor::Content::new(),
                stashes: Vec::new(),
                stashes_visible: false,
                pending_drop_stash: None,
                blame: None,
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

        if self.conflict_editor.is_some() {
            subs.push(self.editor_keys());
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
                        self.merge_mode = MergeMode::Merge;
                        self.merge_target = None;
                        self.load_branches()
                    }
                    MenuAction::RebaseBranches => {
                        self.merge_modal = true;
                        self.merge_mode = MergeMode::Rebase;
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
                        Tab::PullRequests => "pull-requests",
                    }
                    .to_owned());
                }
                self.persist();
                match tab {
                    Tab::History => self.ensure_log(),
                    Tab::PullRequests => {
                        if !self.prs_loaded {
                            self.load_prs()
                        } else {
                            iced::Task::none()
                        }
                    }
                    Tab::Changes => iced::Task::none(),
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
                    self.graph_rows =
                        crate::widgets::commit_graph::compute_lanes(&commits);
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

            Message::RepoMenuToggled(open) => {
                self.repo_menu_open = open;
                iced::Task::none()
            }
            Message::EditConflictPressed { path } => {
                let Some(open) = self.repo.as_ref() else {
                    return iced::Task::none();
                };
                let abs = open.entry.path.join(&path);
                match ConflictEditor::open(&path, abs) {
                    Ok(editor) => {
                        self.notice = Some(format!(
                            "Edita los marcadores <<<<<<< ======= >>>>>>> y usa                              «Guardar y marcar resuelto»."
                        ));
                        self.conflict_editor = Some(editor);
                    }
                    Err(err) => self.error = Some(err),
                }
                iced::Task::none()
            }
            Message::EditorMsg(action) => self.on_editor_msg(action),
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
            Message::RebasePressed => {
                let Some(target) = self.merge_target.clone() else {
                    return iced::Task::none();
                };
                let Some(repo_path) = self.repo_path() else {
                    return iced::Task::none();
                };
                self.loading = true;
                iced::Task::perform(
                    async move {
                        let engine =
                            gitdesktop_core::open(&repo_path).map_err(|e| e.to_string())?;
                        engine.rebase(&target).map_err(|e| e.to_string())
                    },
                    Message::RebaseDone,
                )
            }
            Message::RebaseDone(result) => self.on_rebase_done(result),
            Message::RebaseContinuePressed => {
                self.loading = true;
                match self.repo_path() {
                    Some(repo_path) => op_task(
                        repo_path,
                        Op::RebaseContinue,
                        |engine| engine.continue_rebase(),
                    ),
                    None => {
                        self.loading = false;
                        iced::Task::none()
                    }
                }
            }
            Message::RebaseAbortPressed => {
                self.loading = true;
                match self.repo_path() {
                    Some(repo_path) => iced::Task::perform(
                        async move {
                            let engine = gitdesktop_core::open(&repo_path)
                                .map_err(|e| e.to_string())?;
                            engine.abort_rebase().map_err(|e| e.to_string())?;
                            Ok(MergeOutcome::Aborted)
                        },
                        Message::RebaseDone,
                    ),
                    None => {
                        self.loading = false;
                        iced::Task::none()
                    }
                }
            }

            // ——— Stash ———
            Message::StashesToggled(open) => {
                self.stashes_visible = open;
                if open && self.stashes.is_empty() {
                    self.load_stashes()
                } else {
                    iced::Task::none()
                }
            }
            Message::StashesLoaded(result) => match result {
                Ok(stashes) => {
                    self.stashes = stashes;
                    iced::Task::none()
                }
                Err(err) => {
                    self.error = Some(err);
                    iced::Task::none()
                }
            },
            Message::StashCreatePressed => {
                if self.running_op.is_some() {
                    return iced::Task::none();
                }
                self.loading = true;
                match self.repo_path() {
                    Some(repo_path) => op_task(repo_path, Op::Stash, |engine| {
                        engine.stash_push(Some("WIP desde Git-Desktop Jorgenio"))
                    }),
                    None => {
                        self.loading = false;
                        iced::Task::none()
                    }
                }
            }
            Message::StashApply(reference) => self.stash_op("apply", move |engine| {
                let index = stash_index(&reference);
                engine.stash_apply(index)
            }),
            Message::StashPop(reference) => self.stash_op("pop", move |engine| {
                let index = stash_index(&reference);
                engine.stash_pop(index)
            }),
            Message::StashDropRequested(reference) => {
                self.pending_drop_stash = Some(reference);
                iced::Task::none()
            }
            Message::StashDropConfirmed => {
                match (self.repo_path(), self.pending_drop_stash.take()) {
                    (Some(_repo_path), Some(reference)) => {
                        self.loading = true;
                        self.stash_op("drop", move |engine| {
                            let index = stash_index(&reference);
                            engine.stash_drop(index)
                        })
                    }
                    _ => iced::Task::none(),
                }
            }
            Message::StashCancelDrop => {
                self.pending_drop_stash = None;
                iced::Task::none()
            }
            Message::StashOpDone(result) => {
                self.loading = false;
                match result {
                    Ok(()) => {
                        self.error = None;
                        self.notice = Some("Stash aplicado.".into());
                        self.last_refresh = None;
                        let refresh = self.refresh();
                        match self.repo_path() {
                            Some(path) => {
                                iced::Task::batch([refresh, stashes_task(path)])
                            }
                            None => refresh,
                        }
                    }
                    Err(err) => {
                        self.error = Some(err);
                        iced::Task::none()
                    }
                }
            }

            // ——— Revert / cherry-pick ———
            Message::RevertPressed { id } => {
                self.loading = true;
                match self.repo_path() {
                    Some(repo_path) => iced::Task::perform(
                        async move {
                            let engine = gitdesktop_core::open(&repo_path)
                                .map_err(|e| e.to_string())?;
                            engine.revert_commit(&id).map_err(|e| e.to_string())
                        },
                        Message::RevertDone,
                    ),
                    None => {
                        self.loading = false;
                        iced::Task::none()
                    }
                }
            }
            Message::CherryPressed { id } => {
                self.loading = true;
                match self.repo_path() {
                    Some(repo_path) => iced::Task::perform(
                        async move {
                            let engine = gitdesktop_core::open(&repo_path)
                                .map_err(|e| e.to_string())?;
                            engine.cherry_pick(&id).map_err(|e| e.to_string())
                        },
                        Message::CherryDone,
                    ),
                    None => {
                        self.loading = false;
                        iced::Task::none()
                    }
                }
            }
            Message::RevertDone(result) => self.on_undo_done("Revert", result),
            Message::CherryDone(result) => self.on_undo_done("Cherry-pick", result),
            Message::ContinueOpPressed => {
                let Some(open) = self.repo.as_ref() else {
                    return iced::Task::none();
                };
                let repo_path = open.entry.path.clone();
                self.loading = true;
                if open.status.reverting {
                    op_task(repo_path, Op::RevertContinue, |engine| {
                        engine.continue_revert()
                    })
                } else if open.status.cherry_picking {
                    op_task(repo_path, Op::CherryContinue, |engine| {
                        engine.continue_cherry_pick()
                    })
                } else {
                    op_task(repo_path, Op::RebaseContinue, |engine| {
                        engine.continue_rebase()
                    })
                }
            }

            // ——— Blame ———
            Message::BlameRequested { path } => {
                self.blame = None;
                match self.repo_path() {
                    Some(repo_path) => blame_task(repo_path, path),
                    None => iced::Task::none(),
                }
            }
            Message::BlameLoaded(result) => match result {
                Ok((path, lines)) => {
                    self.blame = Some((path, lines));
                    iced::Task::none()
                }
                Err(err) => {
                    self.error = Some(err);
                    iced::Task::none()
                }
            },
            Message::BlameClosed => {
                self.blame = None;
                iced::Task::none()
            }

            // ——— GitHub: PRs ———
            Message::PrsLoaded(result) => match result {
                Ok(prs) => {
                    self.prs = prs;
                    self.prs_loaded = true;
                    if let Some(selected) = self.selected_pr {
                        if !self.prs.iter().any(|pr| pr.number == selected) {
                            self.selected_pr = None;
                        }
                    }
                    iced::Task::none()
                }
                Err(err) => {
                    self.prs_loaded = true;
                    self.error = Some(err);
                    iced::Task::none()
                }
            },
            Message::PrSelected(number) => {
                self.selected_pr = number;
                iced::Task::none()
            }
            Message::CreatePrModalToggled(open) => {
                self.pr_modal = open;
                if open {
                    self.pr_title.clear();
                    self.pr_body = iced::widget::text_editor::Content::new();
                }
                iced::Task::none()
            }
            Message::PrTitleChanged(title) => {
                self.pr_title = title;
                iced::Task::none()
            }
            Message::PrBodyEdited(action) => {
                self.pr_body.perform(action);
                iced::Task::none()
            }
            Message::CreatePrPressed => {
                let title = self.pr_title.trim().to_owned();
                let body = self.pr_body.text();
                if title.is_empty() || self.repo.is_none() {
                    return iced::Task::none();
                }
                let Some(open) = self.repo.as_ref() else {
                    return iced::Task::none();
                };
                if !open.status.has_upstream {
                    self.error =
                        Some("Publica la rama primero (sin upstream no puede crearse el PR).".into());
                    return iced::Task::none();
                }
                self.loading = true;
                let repo_path = open.entry.path.clone();
                iced::Task::perform(
                    async move { crate::github::create_pr(repo_path, title, body).await },
                    Message::PrCreated,
                )
            }
            Message::PrCreated(result) => match result {
                Ok(url) => {
                    self.loading = false;
                    self.error = None;
                    self.notice = Some(format!("PR creado: {url}"));
                    self.pr_modal = false;
                    self.prs_loaded = false;
                    self.load_prs()
                }
                Err(err) => {
                    self.loading = false;
                    self.error = Some(err);
                    iced::Task::none()
                }
            },
            Message::CheckoutPr { number } => {
                self.loading = true;
                let Some(repo_path) = self.repo_path() else {
                    self.loading = false;
                    return iced::Task::none();
                };
                iced::Task::perform(
                    async move { crate::github::checkout_pr(repo_path, number).await },
                    Message::PrCheckoutDone,
                )
            }
            Message::PrCheckoutDone(result) => {
                self.loading = false;
                match result {
                    Ok(()) => {
                        self.error = None;
                        self.notice = Some("Rama del PR activada.".into());
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
            Message::OpenUrl(url) => iced::Task::perform(
                async move { crate::github::open_url(url).await },
                |_| Message::ClearNotice,
            ),

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
            self.conflict_editor = None;
            self.stashes.clear();
            self.stashes_visible = false;
            self.blame = None;
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
                let refresh = self.refresh();
                match op {
                    Op::Stash => match self.repo_path() {
                        Some(path) => iced::Task::batch([refresh, stashes_task(path)]),
                        None => refresh,
                    },
                    _ => refresh,
                }
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

    /// Tras un rebase: banners según el resultado y refresco completo.
    fn on_rebase_done(
        &mut self,
        result: Result<MergeOutcome, String>,
    ) -> iced::Task<Message> {
        self.loading = false;
        match result {
            Ok(MergeOutcome::Conflicts(files)) => {
                self.error = Some(format!(
                    "El rebase quedó pausado con conflictos en {} archivo(s): {}. \
                     Edítalos, márcalos como resueltos y pulsa «Continuar rebase».",
                    files.len(),
                    files.join(", ")
                ));
                self.after_merge_refresh()
            }
            Ok(outcome) => {
                self.error = None;
                self.notice = Some(match outcome {
                    MergeOutcome::Merged => "Rebase completado.".into(),
                    MergeOutcome::UpToDate => "La rama ya estaba sobre el objetivo.".into(),
                    MergeOutcome::Aborted => "Rebase abortado; rama restaurada.".into(),
                    MergeOutcome::FastForward | MergeOutcome::Conflicts(_) => {
                        unreachable!()
                    }
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

    /// ¿Hay un rebase pausado? (para el modal)
    pub fn status_rebasing(&self) -> bool {
        self.repo.as_ref().map(|open| open.status.rebasing).unwrap_or(false)
    }

    /// Recarga la lista de ramas.
    fn load_branches(&mut self) -> iced::Task<Message> {
        match self.repo_path() {
            Some(path) => branches_task(path),
            None => iced::Task::none(),
        }
    }

    /// Acciones del editor de conflictos embebido.
    fn on_editor_msg(&mut self, action: EditAction) -> iced::Task<Message> {
        // Guardar y cerrar requieren el motor; el resto edita el buffer.
        match action {
            EditAction::Save => {
                let Some(editor) = self.conflict_editor.as_mut() else {
                    return iced::Task::none();
                };
                match editor.save() {
                    Ok(()) => {
                        self.notice = Some("Cambios guardados en el archivo.".into());
                    }
                    Err(err) => self.error = Some(err),
                }
                iced::Task::none()
            }
            EditAction::SaveAndResolve => {
                let Some(editor) = self.conflict_editor.as_mut() else {
                    return iced::Task::none();
                };
                if let Err(err) = editor.save() {
                    self.error = Some(err);
                    return iced::Task::none();
                }
                let path = editor.path.clone();
                self.conflict_editor = None;
                self.loading = true;
                match self.repo_path() {
                    Some(repo_path) => op_task(repo_path, Op::Stage, move |engine| {
                        engine.stage(&path)
                    }),
                    None => {
                        self.loading = false;
                        iced::Task::none()
                    }
                }
            }
            EditAction::Close => {
                self.conflict_editor = None;
                iced::Task::none()
            }
            other => {
                let dirty = self.conflict_editor.as_mut().map(|editor| {
                    editor.apply(&other, EDITOR_VISIBLE_LINES)
                });
                let _ = dirty;
                iced::Task::none()
            }
        }
    }

    /// Teclado exclusivo del editor embebido cuando está abierto.
    fn editor_keys(&self) -> iced::Subscription<Message> {
        use iced::keyboard::{key, Event, Key};

        iced::keyboard::listen().filter_map(|event| {
            let (key, modifiers) = match event {
                Event::KeyPressed { key, modifiers, .. } => (key, modifiers),
                _ => return None,
            };
            let ctrl = modifiers.control() || modifiers.logo();
            match key {
                Key::Named(named) => match named {
                    key::Named::ArrowLeft => Some(Message::EditorMsg(EditAction::Left)),
                    key::Named::ArrowRight => Some(Message::EditorMsg(EditAction::Right)),
                    key::Named::ArrowUp => Some(Message::EditorMsg(EditAction::Up)),
                    key::Named::ArrowDown => Some(Message::EditorMsg(EditAction::Down)),
                    key::Named::Home => Some(Message::EditorMsg(EditAction::Home)),
                    key::Named::End => Some(Message::EditorMsg(EditAction::End)),
                    key::Named::Backspace => Some(Message::EditorMsg(EditAction::Backspace)),
                    key::Named::Delete => Some(Message::EditorMsg(EditAction::Delete)),
                    key::Named::PageUp => Some(Message::EditorMsg(EditAction::PageUp)),
                    key::Named::PageDown => Some(Message::EditorMsg(EditAction::PageDown)),
                    key::Named::Enter => {
                        if ctrl {
                            Some(Message::EditorMsg(EditAction::SaveAndResolve))
                        } else {
                            Some(Message::EditorMsg(EditAction::Newline))
                        }
                    }
                    key::Named::Escape => Some(Message::EditorMsg(EditAction::Close)),
                    _ => None,
                },
                Key::Character(text) => {
                    let text = text.to_lowercase();
                    if ctrl && text == "z" {
                        Some(Message::EditorMsg(if modifiers.shift() {
                            EditAction::Redo
                        } else {
                            EditAction::Undo
                        }))
                    } else if ctrl && text == "s" {
                        Some(Message::EditorMsg(EditAction::SaveAndResolve))
                    } else if ctrl {
                        None
                    } else {
                        Some(Message::EditorMsg(EditAction::Char(text)))
                    }
                }
                _ => None,
            }
        })
    }

    /// Operación de stash genérica (apply/pop/drop) con recarga posterior.
    fn stash_op<F>(&mut self, label: &str, f: F) -> iced::Task<Message>
    where
        F: FnOnce(&dyn gitdesktop_core::GitEngine) -> gitdesktop_core::Result<()>
            + Send
            + 'static,
    {
        let _ = label;
        self.loading = true;
        match self.repo_path() {
            Some(repo_path) => iced::Task::perform(
                async move {
                    let engine = gitdesktop_core::open(&repo_path)
                        .map_err(|err| err.to_string())?;
                    f(engine.as_ref()).map_err(|err| err.to_string())
                },
                Message::StashOpDone,
            ),
            None => {
                self.loading = false;
                iced::Task::none()
            }
        }
    }

    /// Carga los PRs abiertos vía gh (si el repo es de GitHub).
    fn load_prs(&mut self) -> iced::Task<Message> {
        match self.repo_path() {
            Some(path) => iced::Task::perform(
                async move { crate::github::pull_requests(path).await },
                Message::PrsLoaded,
            ),
            None => iced::Task::none(),
        }
    }

    /// Recarga la lista de stashes.
    fn load_stashes(&mut self) -> iced::Task<Message> {
        match self.repo_path() {
            Some(path) => stashes_task(path),
            None => iced::Task::none(),
        }
    }

    /// Resultado común de revert/cherry-pick (con o sin conflictos).
    fn on_undo_done(
        &mut self,
        label: &str,
        result: Result<MergeOutcome, String>,
    ) -> iced::Task<Message> {
        self.loading = false;
        match result {
            Ok(MergeOutcome::Conflicts(_)) => {
                self.error = Some(format!(
                    "{label} dejó conflictos. Edítalos, márcalos como resueltos                      y pulsa «Continuar»."
                ));
                self.last_refresh = None;
                self.refresh()
            }
            Ok(_) => {
                self.error = None;
                self.notice = Some(format!("{label} aplicado correctamente."));
                self.last_refresh = None;
                self.refresh()
            }
            Err(err) => {
                self.error = Some(err);
                iced::Task::none()
            }
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
    pub fn which_exists(binary: &str) -> bool {
        std::env::var_os("PATH")
            .map(|paths| {
                std::env::split_paths(&paths).any(|dir| {
                    dir.join(format!("{binary}{BINARY_SUFFIX}")).is_file()
                })
            })
            .unwrap_or(false)
    }

    pub fn repo_path(&self) -> Option<PathBuf> {
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

/// Recarga la lista de stashes.
fn stashes_task(path: PathBuf) -> iced::Task<Message> {
    iced::Task::perform(
        async move {
            let engine = gitdesktop_core::open(&path).map_err(|err| err.to_string())?;
            engine.stashes().map_err(|err| err.to_string())
        },
        Message::StashesLoaded,
    )
}

/// Calcula el blame de un archivo (respecto a HEAD).
fn blame_task(path: PathBuf, file: String) -> iced::Task<Message> {
    iced::Task::perform(
        async move {
            let engine = gitdesktop_core::open(&path).map_err(|err| err.to_string())?;
            let lines = engine.blame(&file).map_err(|err| err.to_string())?;
            Ok((file, lines))
        },
        Message::BlameLoaded,
    )
}

/// Índice numérico de una referencia `stash@{n}`.
fn stash_index(reference: &str) -> usize {
    reference
        .split("stash@{")
        .nth(1)
        .and_then(|rest| rest.trim_end_matches('}').parse().ok())
        .unwrap_or(0)
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
