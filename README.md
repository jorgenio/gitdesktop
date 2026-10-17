# Gito

Cliente Git de escritorio **100% en Rust** con la filosofía de GitHub Desktop:
una interfaz simple de 3 paneles para gestionar cambios, commits, ramas,
pull, push y merge sin tocar la terminal.

- **Objetivo:** Ubuntu Linux (primario) + Windows
- **UI:** [iced](https://iced.rs) 0.14 · **Motor Git:** libgit2 (`git2`) + CLI `git`
- **Plan completo:** [`PLAN.md`](./PLAN.md)

## Estado (M0)

- [x] Workspace de Cargo (`gito-core` + `gito-app`)
- [x] Persistencia de la lista de repositorios (`gito.toml`, escritura atómica + backup)
- [x] Abrir repositorio y mostrar su estado (rama, ahead/behind, archivos modificados)
- [ ] Stage/unstage, diffs, commits (M1)

## Desarrollo

```sh
cargo run -p gito-app
```

Requiere Rust estable (≥ 1.88): [rustup.rs](https://rustup.rs).

La configuración se guarda en `~/.config/gito/gito.toml` (Linux) o
`%APPDATA%\gito\gito.toml` (Windows).

## Licencia

MIT
