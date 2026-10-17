# PLAN — Cliente Git de escritorio en Rust (clon de GitHub Desktop)

> **Nombre de trabajo:** `gito` (provisional — alternativas: GitNova, RustDesk-Git, TurGit)
> **Plataformas objetivo:** Ubuntu Linux (primario) + Windows
> **Decisiones tomadas:** UI 100% Rust (iced) · MVP = gestor Git simple (sin API de GitHub en fase 1)

---

## 1. Resumen ejecutivo

GitHub Desktop **no tiene versión oficial para Linux** (solo Windows/macOS) y los forks comunitarios
para Linux son ports de Electron (pesados, ~300 MB, alto consumo de RAM). Este proyecto construye un
cliente Git de escritorio **nativo y 100% en Rust**, con la misma filosofía de GitHub Desktop:
interfaz simple de 3 paneles para gestionar el día a día — cambios, commits, ramas, pull, push y merge —
sin tocar la terminal.

**Fase 1 (este plan):** gestor Git puro, sin dependencia de la API de GitHub.
**Fase futura:** integración con GitHub (OAuth, PRs, issues) usando la base ya construida.

---

## 2. Contexto y estado del arte

| App | Stack | Linux | Problema |
|---|---|---|---|
| GitHub Desktop (oficial) | Electron + TypeScript | ❌ | No existe para Linux |
| Forks comunitarios (shiftkey/desktop → forks 2025-26) | Electron | ✅ .deb/AppImage/rpm | Mismo peso de Electron; mantenimiento irregular (shiftkey inactivo desde mediados de 2025) |
| GitButler | Tauri + Rust + Svelte | ✅ | Licencia Fair Source (prohíbe competir), enfoque distinto (virtual branches) |
| GitKraken | Electron (propietario) | ✅ | Cerrado, de pago para repos privados |
| Gittyup / GitCola | C++/Qt, Python/Qt | ✅ | UI anticuada, no estilo GitHub Desktop |

**Conclusión:** hay un nicho real para un cliente Git **libre (MIT/Apache), nativo en Rust y ligero**,
con la UX de GitHub Desktop. GitButler ya demostró que la pila Rust funciona bien en escritorio.

---

## 3. Alcance

### 3.1 MVP (v0.1.0) — "un gestor de git, rápido y sin fricción"

| # | Funcionalidad | Detalle |
|---|---|---|
| 1 | Abrir repositorio | Selector de carpeta + **lista de repos persistida** en disco (ver §8): al reabrir la app están todos sin buscar nada |
| 2 | Clonar | URL HTTPS/SSH con barra de progreso |
| 3 | Cambios (Changes) | Lista de archivos modificados/nuevos/borrados con **checkbox para stage/unstage** (como GitHub Desktop), icono de estado por archivo |
| 4 | Diff | Visor **unificado** con números de línea y resaltado de sintaxis; líneas añadidas/eliminadas en verde/rojo |
| 5 | Commit | Summary + description, botón contextual **"Commit to `<rama>`"**, amend opcional |
| 6 | Descartar cambios | Por archivo (con confirmación) |
| 7 | History | Lista de commits (autor, fecha, mensaje) + detalle de commit con su diff |
| 8 | Ramas | Selector en cabecera, crear, cambiar, eliminar |
| 9 | Fetch/Pull/Push | Fetch automático en segundo plano (indicador "última sincronización hace X"), pull con fast-forward **o merge**, push, "Publish branch" si no hay upstream |
| 10 | Merge | Elegir rama a fusionar, detección de conflictos, marcar archivo resuelto, commit de fusión o **abortar merge** |
| 11 | Refresco en vivo | Vigilancia del sistema de archivos: la lista de cambios se actualiza sola |

### 3.2 Fuera del MVP (fase 2+)

- Stash (sección "Stashed Changes"), rebase, cherry-pick, revert, blame
- Diff **side-by-side** (el MVP será unificado; el side-by-side del screenshot es fase 2)
- Grafo visual de commits (canvas)
- Editor integrado para resolver conflictos
- Submodules, Git LFS (detectar y avisar que no están soportados)
- Tema claro, i18n completo, auto-actualizador

### 3.3 Fase 3 — Integración GitHub (lo que mencionas para después)

- Login OAuth (device flow), avatar y datos de usuario
- Lista/creación/comentario de Pull Requests, estado de CI (checks)
- Notificaciones, apertura de issues → la arquitectura del núcleo (crates separados) lo permite sin reescritura

---

## 4. Stack tecnológico

### 4.1 Interfaz — `iced` 0.14 (elegido)

| Criterio | iced 0.14 | egui 0.37 |
|---|---|---|
| Paradigma | Elm (estado explícito, eventos tipados) | Immediate mode |
| Widgets de escritorio (listas, scroll, checkboxes, dropdowns) | ✅ sólidos | ✅ pero menos "app de escritorio" |
| Resaltado de sintaxis | ✅ feature `highlighter` (syntect integrado) | Manual |
| Texto multilínea editable (descripción del commit) | ✅ `text_editor` | Básico |
| Look & feel | Tipo app nativa, tema oscuro fácil | Look propio de egui |
| Riesgo | Widgets custom requieren API `advanced` | Rendimiento en diffs grandes |

**Veredicto:** iced para el clon de GitHub Desktop (arquitectura predecible, widgets apropiados,
resaltado integrado). Requiere Rust ≥ 1.88. Render wgpu + fallback tiny-skia (funciona en VMs sin GPU).

### 4.2 Motor Git — **híbrido: `git2` 0.21 + CLI `git` para red**

Este es el punto de diseño más importante. GitHub Desktop no usa libgit2: **ejecuta el CLI `git`**
(vía dugite) para aprovechar credenciales, hooks y progreso. Recomendamos replicar esa idea:

| Operación | Implementación | Por qué |
|---|---|---|
| Status, stage, discard, commit, diff, log, ramas, merge, conflictos | **git2 (libgit2)** en el proceso | Rápido, sin spawns, control fino de datos |
| Clone, fetch, pull, push | **CLI `git`** en subproceso con `--progress` | Hereda **credential helpers del sistema** (libsecret en Ubuntu, Git Credential Manager en Windows), ssh-agent, proxies, y da progreso real sin pelear con libssh2 |

Todo el acceso a Git se esconde detrás de un trait `GitEngine` (ver §5) para que, en el futuro,
se pueda migrar a **gix 0.89 (gitoxide, 100% Rust)** sin tocar la UI.

### 4.3 Resto de crates

| Crate | Uso |
|---|---|
| `tokio` | Runtime para subprocesos del CLI y temporizadores |
| `notify` | Vigilancia del filesystem (auto-refresh de cambios) |
| `rfd` | Diálogos nativos de archivos/carpetas (Linux + Windows) |
| `syntect` | Resaltado de sintaxis en diffs (vía feature de iced o directo) |
| `serde`, `toml` | Persistencia de la lista de repos y configuración (`gito.toml`) |
| `rusqlite` | SQLite embebido — **fase 2** (historial de operaciones, caché de PRs) |
| `dirs` | Rutas de configuración por plataforma |
| `open` | Abrir archivo/carpeta/repo en el navegador o editor |
| `thiserror` + `tracing` | Errores y logging |
| `time` | "última sincronización hace 3 min" |

---

## 5. Arquitectura

```
┌─────────────────────────────────────────────────────────┐
│                    gito-app (iced)                       │
│  views: changes · history · diff · dialogs · progreso    │
└───────────────▲─────────────────────────▲───────────────┘
      Message (Elm)              Subscription (eventos)
┌───────────────┴─────────────────────────┴───────────────┐
│                  gito-core (biblioteca)                  │
│  ┌────────────┐  ┌──────────────────┐  ┌──────────────┐  │
│  │ RepoWorker │  │  CliBridge (git) │  │ FileWatcher  │  │
│  │ 1 hilo     │  │ clone/fetch/pull │  │ notify →     │  │
│  │ dueño del  │  │ /push + progreso │  │ debounced    │  │
│  │ git2::Repo │  └──────────────────┘  └──────────────┘  │
│  └────────────┘                                           │
│  trait GitEngine { status(); commit(); push(); merge()… } │
└──────────────────────────────────────────────────────────┘
```

**Reglas clave:**

1. **Un solo hilo worker por repositorio** es dueño exclusivo de `git2::Repository` (no es `Sync`).
   La UI envía comandos por canal `mpsc` y recibe resultados como `Message`s de iced → cero race conditions.
2. **Operaciones largas (clone/fetch/push/merge) siempre en background** con `Progress { fase, %, bytes }`
   hacia la barra de estado; la UI nunca se bloquea.
3. **Estado de la app** = struct único `App` (patrón Elm de iced): repos recientes, repo activo,
   `RepoStatus` cacheado, diffs cacheados por archivo seleccionado, operación en curso.
4. **Auto-fetch cada 5 min** (temporizador tipo suscripción) + refresco por `notify` al tocar archivos.
5. **Modelos propios** en gito-core (`FileEntry { path, status, staged }`, `DiffLine { old_no, new_no, kind, content }`,
   `CommitSummary`, `BranchInfo`, `MergeConflict { path, ours, theirs }`) — la UI nunca ve tipos de git2.

---

## 6. Diseño de UI (mapeo del screenshot de GitHub Desktop)

```
┌────────────────────────────────────────────────────────────────────────────┐
│ ▤ Current Repository   │ ⑂ Current Branch      │ ⇩ Pull origin              │
│   gito            ▾    │   main            ▾    │   Última sync: hace 3 min  │
├──────────────┬─────────────────────────────────────────────────────────────┤
│ Changes  (3) │  app/src/ui/diff/seamless_diff_switcher.rs        ⚙         │
│ History      ├─────────────────────────────────────────────────────────────┤
│──────────────│  @@ -19,6 +19,7 @@  import {                                 │
│ ☑ 3 archivos │  - prevDiff.text === newDiff.text)          [rojo]           │
│ ☑ src/app.rs │  + textDiffEquals(prevDiff, newDiff))       [verde]          │
│ ☑ README.md  │                                                              │
│              │         (visor unificado; side-by-side en fase 2)            │
│              │                                                             │
│ [fase 2:     │                                                             │
│  Stashed]    │                                                             │
│──────────────│                                                             │
│ Summary      │                                                             │
│ [__________] │                                                             │
│ Description  │                                                             │
│ [__________] │                                                             │
│ [ Commit to  │                                                             │
│    main ]    │                                                             │
└──────────────┴─────────────────────────────────────────────────────────────┘
   panel izq. ~360px          panel diff (resto)
```

- **Tema oscuro** tipo GitHub Desktop (#0d1117/#161b22, acentos azules #2f81f7; verdes #238636, rojos #da3633).
- Cabecera con los 3 controles clave siempre visibles (repo, rama, pull) — exactamente como el screenshot.
- Botón de commit contextual: `Commit to main` / `Commit to feature/x`.
- Barra de progreso en el propio botón Pull/Push durante la operación.
- Atajos: `Ctrl+1/2` changes/history, `Ctrl+Enter` commit, `Ctrl+Shift+P` push, `Ctrl+Shift+L` pull.

---

## 7. Credenciales (el mayor riesgo resuelto por diseño)

| Escenario | Solución |
|---|---|
| HTTPS en Ubuntu | CLI git → helper `libsecret` del sistema (sin guardar nada en la app) |
| HTTPS en Windows | CLI git → Git Credential Manager |
| SSH | CLI git → ssh-agent / claves por defecto |
| Usuario sin helpers | Fallback: petición de usuario + token (PAT) en diálogo, pasada por `GIT_ASKPASS`/env al subproceso |

La app **nunca almacena credenciales propias** en el MVP → menos superficie de seguridad.

---

## 8. Persistencia — lista de repositorios

**Sí**: la lista de repositorios se guarda en disco desde el hito M0 — abres la app y tus repos ya
están listados, sin volver a buscarlos. Para el MVP se usa un **archivo de configuración TOML**
(la misma estrategia de GitHub Desktop, que guarda su lista en un JSON del directorio de datos del
usuario). Una base de datos formal (SQLite) entra en fase 2, solo cuando el volumen de datos lo
justifique — no necesitamos SQL para guardar 10-50 rutas.

### 8.1 MVP — `gito.toml` (`dirs` + `serde` + `toml`)

- Rutas estándar de cada plataforma: Linux `~/.config/gito/gito.toml` · Windows `%APPDATA%\gito\gito.toml`
- Escritura **atómica** (archivo temporal + rename) y copia de seguridad `gito.toml.bak`;
  si el archivo se corrompe se restaura el backup y se recrea limpio
- La app es el único proceso que escribe ese archivo → sin bloqueos, sin daemon
- Código aislado en `gito-core/store.rs` tras un mini-trait `RepoStore` (igual que `GitEngine`),
  para poder cambiar a SQLite sin tocar la UI

```toml
[ui]
theme = "dark"
last_repo = "a1b2c3"
window = { width = 1400, height = 900, maximized = false }

[[repo]]
id = "a1b2c3"
path = "/home/jorgenio/Projects/gito"
name = "gito"
added_at = 2026-10-17T10:00:00Z
last_opened_at = 2026-10-17T18:30:00Z
pinned = false
last_branch = "main"
last_tab = "changes"

[[repo]]
id = "9f8e7d"
path = "C:\\dev\\mi-app"
name = "mi-app"
added_at = 2026-09-02T09:00:00Z
last_opened_at = 2026-10-15T22:10:00Z
pinned = true
```

**Qué se guarda:** lista de repos (ruta, nombre, fecha de alta y de último uso, fijados "pinned"),
orden manual, última rama y pestaña por repositorio, tema, tamaño/posición de la ventana.

### 8.2 Fase 2 — SQLite (`rusqlite`, embebido)

Se migra cuando aparezcan datos con volumen o consulta compleja:

- Historial de operaciones para *undo timeline* (estilo GitButler)
- Caché de PRs, checks de CI y notificaciones (fase de integración GitHub)
- Búsqueda global de commits

La migración es automática: al primer arranque se lee el TOML y se inserta en la BD.

---

## 9. Estructura del repositorio

```
gito/
├── Cargo.toml                  # workspace
├── crates/
│   ├── gito-core/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── engine.rs       # trait GitEngine + factory
│   │       ├── models.rs       # FileEntry, DiffLine, CommitSummary, BranchInfo…
│   │       ├── worker.rs       # RepoWorker (hilo + canales)
│   │       ├── native/         # implementación con git2
│   │       │   ├── status.rs, diff.rs, commits.rs, branches.rs, merge.rs
│   │       ├── cli.rs          # CliBridge: spawn git --progress, parse de progreso
│   │       ├── watcher.rs
│   │       ├── store.rs       # persistencia: repos + estado UI → gito.toml (atómico)
│   │       └── error.rs
│   └── gito-app/
│       └── src/
│           ├── main.rs, state.rs, messages.rs, theme.rs
│           ├── views/          # changes.rs, history.rs, diff_view.rs, dialogs.rs
│           └── widgets/        # file_list.rs, dropdown.rs, progress_button.rs
├── assets/                     # iconos (SVG), fuente, .desktop
├── packaging/                  # cargo-deb, AppImage, cargo-wix
├── .github/workflows/          # ci.yml (matriz ubuntu/windows), release.yml
└── docs/
```

Licencia propuesta: **MIT** (o Apache-2.0 para cobertura de patentes).

---

## 10. Roadmap

| Hito | Duración | Entregable | Criterio de aceptación |
|---|---|---|---|
| **M0 — Esqueleto** | semana 1 | Workspace + CI (Ubuntu/Windows) + ventana iced con tema oscuro + abrir repo y listar estado + **persistencia de la lista de repos (`store.rs`)** | `cargo build` verde en ambas plataformas; abre cualquier repo y **la lista de repos sobrevive al cierre y reapertura** |
| **M1 — Changes + Commit** | sem. 2-3 | Lista de archivos con checkboxes, stage/unstage/discard, diff unificado con resaltado, commit, botón "Commit to X" | Flujo completo edit→stage→commit sin tocar terminal |
| **M2 — History + Diff** | semana 4 | Historial con detalle de commit y su diff; watcher en vivo | Al editar un archivo fuera de la app, la lista se refresca sola |
| **M3 — Red** | sem. 5-6 | Fetch automático + indicador "hace X", pull (ff/merge), push, publish, progreso, credenciales | Push/pull a un repo real de GitHub con HTTPS y SSH en Ubuntu y Windows |
| **M4 — Ramas + Merge** | semana 7 | Selector de rama, crear/cambiar/eliminar, merge, conflictos (marcar resuelto / abortar), clone con progreso | Merge con conflicto detectado, resuelto y commiteado desde la app |
| **M5 — Release 0.1.0** | semana 8 | Pulido, atajos, errores amigables, .deb + AppImage + MSI firmado (o sin firmar) | Instalable en Ubuntu 22.04/24.04 y Windows 10/11 |

**Esfuerzo estimado:** ~8 semanas a tiempo parcial (1 desarrollador). El núcleo `gito-core`
es reutilizable después para CLI (fase 2) sin cambios en la UI.

### Post-MVP (orden sugerido)
1. Stash, side-by-side diff, grafo de commits (canvas de iced), rebase/cherry-pick/revert
2. Editor de conflictos integrado, tema claro, i18n (es/en), auto-actualizador
3. **Integración GitHub**: OAuth device flow → PRs, checks de CI, notificaciones (tu idea de "luego se integra")
4. Snap/Flatpak, winget, empaquetado rpm

---

## 11. Riesgos y mitigaciones

| Riesgo | Impacto | Mitigación |
|---|---|---|
| libgit2 se comporta distinto al CLI en merges raros | Confusiones de usuario | Operaciones de red en CLI real; tests de integración contra repos con historiales complejos |
| Credenciales multiplataforma frágiles | Bloqueo del flujo principal | Usar helpers del sistema vía CLI (§7); nunca reinventar auth |
| Widgets custom en iced (diff con números de línea, dropdowns) | Tiempo M1 | Prototipar el widget diff **primero** (M1, no al final); API `advanced` documentada |
| Repos grandes (status lento) | UI congelada percibida | Todo en worker en background + debounce del watcher + caché de diffs |
| Repos con LFS/submodules | Fallos confusos | Detección temprana + aviso "no soportado en MVP" |
| Windows: CRLF, rutas largas | Commits sorpresa | Respetar `core.autocrlf`, mostrar configuración de saltos de línea |
| Migración futura a gix | Reescritura | Trait `GitEngine` aísla el motor desde el día 1 |

---

## 12. Distribución

- **Ubuntu:** `.deb` (cargo-deb) para 22.04/24.04 + **AppImage** universal; Flatpak en fase 2
- **Windows:** MSI (cargo-wix) o NSIS; winget en fase 2
- **CI:** GitHub Actions — matriz `ubuntu-22.04`/`windows-latest`, tests + builds de release en cada tag
- Binario esperado: **~15-25 MB** (vs ~300 MB de un port Electron de GitHub Desktop)

---

## 13. Próximos pasos inmediatos

1. Crear el workspace y el esqueleto de `gito-core`/`gito-app` (M0), incluida la capa de persistencia `store.rs` → `gito.toml`
2. Configurar CI de las dos plataformas desde el día 1
3. Prototipo del **widget de diff** (el componente más crítico de la UI)
4. Validar M3 con un repo real temprano (la red es la parte con más sorpresas)

¿Empezamos? Dime "anda con el M0" y creo el scaffold completo del proyecto.
